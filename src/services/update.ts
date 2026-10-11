/**
 * 应用自动更新。
 *
 * 流程（配合发版流程自动生成的 latest.json，见 .github/workflows/release.yml）：
 *
 *   启动 → 检查远端清单 → 有新版本就弹出提示并自动开始下载
 *        → 下载完成 → 用户点"立即重启安装" → 交给安装程序 → 应用自动重启
 *
 * 关键点：
 * - 清单地址写在 tauri.conf.json 的 plugins.updater.endpoints，指向
 *   https://github.com/<owner>/<repo>/releases/latest/download/latest.json；
 * - 清单里带版本号、下载地址、更新日志（notes）和签名；更新包必须通过
 *   minisign 签名校验（plugins.updater.pubkey）才会被安装，私钥只存在 CI 的 Secret 里；
 * - 插件只在 Tauri 里可用，浏览器预览（vite dev）时全部静默降级；
 * - 开发模式（pnpm tauri dev）不做启动检查，免得拿本地版本和已发布版本比较。
 */

import { reactive, readonly, ref } from "vue";
import type { Update } from "@tauri-apps/plugin-updater";

export type UpdatePhase =
  /** 还没检查过 */
  | "idle"
  /** 正在检查 */
  | "checking"
  /** 有新版本，准备/正在下载 */
  | "available"
  /** 正在下载 */
  | "downloading"
  /** 下载完成，等用户确认安装 */
  | "ready"
  /** 正在启动安装程序 */
  | "installing"
  /** 已是最新版本 */
  | "latest"
  /** 出错（离线、清单缺失、公钥未配置等） */
  | "error";

export interface UpdateState {
  phase: UpdatePhase;
  /** 远端最新版本号 */
  version: string;
  /** 当前运行版本号 */
  currentVersion: string;
  /** 更新日志，来自清单的 notes（也就是 GitHub 自动生成的 Release 说明） */
  notes: string;
  /** 下载进度 0~100；服务端没给总大小时保持 0 */
  percent: number;
  /** 出错信息，仅用于手动检查时的提示 */
  error: string;
}

const state = reactive<UpdateState>({
  phase: "idle",
  version: "",
  currentVersion: "",
  notes: "",
  percent: 0,
  error: ""
});

const dialogVisible = ref(false);

/** 已下载（或正在下载）的更新句柄：保留它，"稍后再说"之后仍可直接安装 */
let pendingUpdate: Update | null = null;

export const updateState = readonly(state);
export const updateDialogVisible = readonly(dialogVisible);

/** 是否运行在 Tauri 里（浏览器预览时为 false） */
function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function describeError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (/public key|pubkey|signature/i.test(message)) {
    return "更新签名校验未通过（公钥没配置或与发布用的私钥不匹配）";
  }
  if (/network|request|dns|connect|timed?\s?out|error sending/i.test(message)) {
    return "网络请求失败，请检查网络后重试";
  }
  if (/404|not found/i.test(message)) {
    return "远端还没有更新清单 latest.json（需要先发布一个带更新包的版本）";
  }
  return message || "未知错误";
}

async function readCurrentVersion(): Promise<string> {
  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    return await getVersion();
  } catch {
    return "";
  }
}

/**
 * 检查更新。返回 true 表示发现新版本并已开始下载。
 * silent=true（启动自动检查）时失败不打扰用户，只记一条控制台日志。
 */
export async function checkForUpdate(silent = false): Promise<boolean> {
  if (!isTauriRuntime()) {
    state.phase = silent ? "idle" : "error";
    state.error = silent ? "" : "当前环境不支持自动更新";
    return false;
  }

  state.phase = "checking";
  state.error = "";
  state.percent = 0;

  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();

    if (!update) {
      const current = await readCurrentVersion();
      state.phase = "latest";
      state.currentVersion = current;
      state.version = current;
      return false;
    }

    // 之前留下的更新先释放，避免同时持有两份
    await pendingUpdate?.close().catch(() => undefined);
    pendingUpdate = update;

    state.version = update.version;
    state.currentVersion = update.currentVersion;
    state.notes = update.body ?? "";
    state.phase = "available";

    // "有新版本就自动下载"：弹出提示的同时就开始下载
    void startDownload();
    return true;
  } catch (error) {
    state.phase = silent ? "idle" : "error";
    state.error = silent ? "" : describeError(error);
    if (silent) console.warn("[update] 启动检查更新失败（已忽略）：", error);
    return false;
  }
}

/** 下载更新包并刷新进度；下载完成后进入 ready，等用户确认安装 */
export async function startDownload(): Promise<boolean> {
  const update = pendingUpdate;
  if (!update) return false;
  if (state.phase === "downloading" || state.phase === "ready" || state.phase === "installing") {
    return true;
  }

  state.phase = "downloading";
  state.percent = 0;

  let total = 0;
  let received = 0;

  try {
    await update.download((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
      } else if (event.event === "Progress") {
        received += event.data.chunkLength;
        if (total > 0) state.percent = Math.min(99, Math.round((received / total) * 100));
      } else {
        state.percent = 100;
      }
    });
    state.percent = 100;
    state.phase = "ready";
    return true;
  } catch (error) {
    state.phase = "error";
    state.error = `下载更新失败：${describeError(error)}`;
    return false;
  }
}

/**
 * 安装已下载的更新。
 * Windows 下插件启动安装程序后会退出当前进程（安装程序装完自动重启应用），
 * 所以这里之后的代码在 Windows 上不一定会执行。
 */
export async function installUpdate(): Promise<void> {
  const update = pendingUpdate;
  if (!update || state.phase !== "ready") return;

  state.phase = "installing";
  try {
    await update.install();
  } catch (error) {
    state.phase = "ready";
    state.error = `安装失败：${describeError(error)}`;
  }
}

/** 点"稍后再说"：只收起提示层，下载结果保留，随时可以再装 */
export function hideUpdateDialog(): void {
  dialogVisible.value = false;
}

export function showUpdateDialog(): void {
  dialogVisible.value = true;
}

/** 启动时检查一次：开发模式跳过，失败静默 */
export async function initAutoUpdate(): Promise<void> {
  if (import.meta.env.DEV || !isTauriRuntime()) return;
  window.setTimeout(() => {
    void checkForUpdate(true).then((hasUpdate) => {
      if (hasUpdate) dialogVisible.value = true;
    });
  }, 3000);
}
