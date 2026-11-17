<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { ArrowDown, CopyDocument, Refresh, Search, Warning } from "@element-plus/icons-vue";
import {
  addManualPeer,
  disableSync,
  enableSync,
  getStatus,
  lanDiagnose,
  lanLocalAddrs,
  loadConfig,
  normalizeCode,
  removeManualPeer,
  rotateCode,
  setLanOnly,
  setSyncImages,
  setSyncMode,
  subscribeConfig,
  subscribeStatus,
  syncNow,
  type SyncConfig,
  type SyncMode,
  type SyncStatus
} from "@/services/sync";

/**
 * 多设备同步面板。
 *
 * 交互只做三件事：开关、看/抄同步码、手动同步。同步码本身就是账号+密钥，
 * 所以界面上不出现"账号""密码"这类词——用户只需要知道抄走这 6 位数字，
 * 在另一台设备输入进去，两边就开始共享同一份历史。
 */

const config = ref<SyncConfig>(loadConfig());
const status = ref<SyncStatus>(getStatus());
const joinInput = ref("");
const busy = ref(false);

/** 手填对端地址 */
const peerInput = ref("");
const peerBusy = ref(false);
const diagBusy = ref(false);
const localAddrs = ref<string[]>([]);
const diagnosisText = ref("");
/** 高级设置默认收起：第一屏只保留开关、状态、同步码与加入 */
const advancedOpen = ref(false);

/** 输入时只留数字并自动按 3+3 分组，粘贴带空格的长码也不会被拒 */
watch(joinInput, (value) => {
  const digits = value.replace(/\D/g, "").slice(0, 6);
  const formatted = digits.length > 3 ? `${digits.slice(0, 3)} ${digits.slice(3)}` : digits;
  if (formatted !== value) joinInput.value = formatted;
});

let unsubscribeStatus: (() => void) | null = null;
let unsubscribeConfig: (() => void) | null = null;

onMounted(() => {
  unsubscribeStatus = subscribeStatus((value) => {
    status.value = value;
  });
  unsubscribeConfig = subscribeConfig((value) => {
    config.value = value;
  });
  void lanLocalAddrs().then((list) => {
    localAddrs.value = list;
  });
});

onBeforeUnmount(() => {
  unsubscribeStatus?.();
  unsubscribeConfig?.();
});

/** 6 位数字拆成 3+3 展示，口述和核对都更轻松 */
const formattedCode = computed(() => {
  const code = config.value.code;
  if (!code) return "";
  return `${code.slice(0, 3)} ${code.slice(3)}`;
});

/** 中继地址只在构建时注入，缺了它就是截图里那条"同步异常" */
const relayMissing = computed(() => !config.value.relay);

/** 当前实际走的是哪条通道：手动开了局域网，或者压根没有中继可用 */
const lanMode = computed(() => config.value.lanOnly || !config.value.relay);

/** 高级项被动过就挂个提示，避免折叠后忘了里面改过什么 */
const advancedDirty = computed(
  () =>
    config.value.lanOnly ||
    config.value.syncImages ||
    config.value.mode !== "both" ||
    (config.value.manualPeers?.length ?? 0) > 0
);

/** 方向前缀：单向模式一定要在状态里写明，否则用户会以为双向都在跑 */
const modeLabel = computed(() => {
  if (config.value.mode === "send") return "仅发送";
  if (config.value.mode === "receive") return "仅接收";
  return "";
});

/** 邻居的可读名字：设备名，用来判断"连上的到底是不是那台" */
const peerLabel = computed(() => {
  const names = status.value.lanPeerNames;
  if (names && names.length > 0) return names.join("、");
  return `${status.value.lanPeers} 台设备`;
});

/** 发现得到但连不上的设备：和"连上了"必须分开说，否则用户会以为同步在跑 */
const unreachableText = computed(() => status.value.lanUnreachable?.[0] ?? "");

/** 收到却解不开的条数：通常意味着两台设备的同步码其实不是同一个 */
const skippedText = computed(() => {
  const skipped = status.value.skipped ?? 0;
  if (!skipped) return "";
  return ` · 有 ${skipped} 条解不开（两台的同步码可能不一致）`;
});

const statusText = computed(() => {
  if (!config.value.enabled) return "未启用";
  if (status.value.busy) return "同步中…";
  if (status.value.lastError) return status.value.lastError + skippedText.value;

  const prefix = modeLabel.value ? `${modeLabel.value} · ` : "";

  if (lanMode.value) {
    if (status.value.lanPeers === 0) {
      return "还没发现同房间设备：确认两台连的是同一个 Wi-Fi、同步码一致；自动发现不生效时，到「高级设置」里手动填对方 IP";
    }
    const base = `已发现 ${peerLabel.value}`;
    // 探测通了才算真连上；只看到广播、TCP 连不上时把原因摆出来
    if (unreachableText.value) {
      return `${prefix}${base}，但连不上：${unreachableText.value}`;
    }
    if (!status.value.lastSyncAt) return `${prefix}${base} · 等待首次同步`;
    const time = new Date(status.value.lastSyncAt).toLocaleTimeString();
    return `${prefix}${base} · 上次同步 ${time} · 收到 ${status.value.lastPulled} 条 · 发出 ${status.value.lastPushed} 条${skippedText.value}`;
  }

  if (!status.value.lastSyncAt) return `${prefix}等待首次同步`;
  const time = new Date(status.value.lastSyncAt).toLocaleTimeString();
  return `${prefix}上次同步 ${time} · 收到 ${status.value.lastPulled} 条 · 发出 ${status.value.lastPushed} 条${skippedText.value}`;
});

/** 状态行是不是"看起来连上、其实不通"：用来换成警示色 */
const statusWarning = computed(
  () =>
    Boolean(status.value.lastError) ||
    Boolean(unreachableText.value) ||
    Boolean(status.value.skipped)
);

/** 手填过的对端地址 */
const manualPeers = computed(() => config.value.manualPeers ?? []);

/** 本机地址：两台互相填的时候，对方要的就是这一串 */
const localAddrsText = computed(() =>
  localAddrs.value.length > 0 ? localAddrs.value.join("、") : "未获取到（检查网络连接）"
);

/** 手动添加一台设备：广播发现不到时的保底路径 */
async function handleAddPeer() {
  const addr = peerInput.value.trim();
  if (!addr) {
    ElMessage.error("请填写对方的 IP，例如 192.168.1.8");
    return;
  }
  peerBusy.value = true;
  try {
    const result = await addManualPeer(addr);
    config.value = loadConfig();
    peerInput.value = "";
    if (result.ok) {
      ElMessage.success(result.message);
    } else {
      ElMessage.warning(result.message);
    }
    void syncNow();
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : "添加失败");
  } finally {
    peerBusy.value = false;
  }
}

function handleRemovePeer(addr: string) {
  config.value = removeManualPeer(addr);
  ElMessage.success(`已移除 ${addr}`);
}

/** 网络体检：把"到底卡在哪一步"摆出来，别让用户对着 0 条猜 */
async function handleDiagnose() {
  diagBusy.value = true;
  try {
    localAddrs.value = await lanLocalAddrs();
    const result = await lanDiagnose();
    if (!result) {
      diagnosisText.value = "诊断不可用（桌面端才支持）";
      return;
    }
    if (!result.roomConfigured) {
      diagnosisText.value = "还没开启同步：先把上面的多设备同步开关打开";
      return;
    }
    if (!result.started) {
      diagnosisText.value =
        result.lastError ||
        `端口 ${result.servicePort} 没能监听，通常是另一个 Xpaste 还在运行，退出旧版本后重试`;
      return;
    }
    if (result.peers.length === 0) {
      diagnosisText.value = `端口正常（TCP ${result.servicePort} / UDP ${result.discoveryPort}），但没收到任何设备的广播：两台可能不在同一网段，或被路由器隔离。到「高级设置」里手动添加对方 IP（本机 ${localAddrsText.value}）；macOS 还要在 系统设置 → 隐私与安全性 → 本地网络 里允许 Xpaste，Windows 首次运行要允许防火墙访问`;
      return;
    }
    const failed = result.peers.filter((peer) => !peer.ok);
    if (failed.length === 0) {
      diagnosisText.value = `一切正常：和 ${result.peers.length} 台设备都能握手，端口 ${result.servicePort}`;
      return;
    }
    diagnosisText.value = failed
      .map((peer) => `${peer.device || peer.addr}：${explainReason(peer.error)}`)
      .join("；");
  } finally {
    diagBusy.value = false;
  }
}

/** 把 Rust 抛上来的连接错误翻成能照着做的提示 */
function explainReason(raw: string): string {
  const text = raw || "";
  if (/refused|拒绝/i.test(text)) return "对方端口拒绝连接：那台设备可能没在运行、没开启同步，或被防火墙拦截";
  if (/timed? ?out|超时/i.test(text)) return "连接超时，多半是防火墙或路由器隔离";
  if (/unreachable|No route|网络不可达/i.test(text)) return "网络不可达，两台不在同一网段";
  if (/拒绝接收/.test(text)) return "被对方拒绝，两台的同步码不一致";
  return text || "连接失败";
}

async function handleToggle(value: boolean) {
  try {
    if (value) {
      await enableSync();
      config.value = loadConfig();
      ElMessage.success("已开启同步，正在和另一端对齐");
    } else {
      disableSync();
      config.value = loadConfig();
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : "开启失败");
  }
}

async function handleJoin() {
  const normalized = normalizeCode(joinInput.value);
  if (!normalized) {
    ElMessage.error("同步码应为 6 位数字，例如 123 456");
    return;
  }
  busy.value = true;
  try {
    await enableSync(normalized);
    config.value = loadConfig();
    joinInput.value = "";
    ElMessage.success("已加入，另一端的历史会陆续同步过来");
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : "加入失败");
  } finally {
    busy.value = false;
  }
}

async function handleCopy() {
  if (!config.value.code) return;
  try {
    await navigator.clipboard.writeText(config.value.code);
    ElMessage.success("同步码已复制，别发到公开的地方");
  } catch {
    ElMessage.error("复制失败，请手动选中复制");
  }
}

function handleRotate() {
  const code = rotateCode();
  config.value = loadConfig();
  ElMessage.success(`已换成新同步码 ${code}，旧设备需要用新码重新加入`);
}

/** 局域网开关：打开后不再依赖公网，同 Wi-Fi 设备直连 */
function handleLanToggle(value: boolean) {
  config.value = setLanOnly(value);
  ElMessage.success(
    value ? "已切换为局域网直连，同一 Wi-Fi 下的设备会互相发现" : "已恢复中继同步，局域网通道继续并行可用"
  );
}

/** 同步方向：只发 / 只收 / 双向，切换后立刻跑一轮 */
function handleModeChange(value: SyncMode) {
  config.value = setSyncMode(value);
  ElMessage.success(
    value === "both"
      ? "已切回双向同步，期间积压的内容会一并补发和补收"
      : value === "send"
        ? "只发送：本机内容会传给对方，但不接收对方的内容"
        : "只接收：只拉取对方的内容，本机内容不会传出去"
  );
}

/** 图片同步开关：默认关闭，打开后回溯 7 天并立刻推一轮 */
function handleImagesToggle(value: boolean) {
  config.value = setSyncImages(value);
  ElMessage.success(
    value ? "已开启图片同步，会补推最近 7 天的图片" : "已关闭图片同步，只同步文字"
  );
}

async function handleSyncNow() {
  busy.value = true;
  try {
    await syncNow();
    config.value = loadConfig();
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="settings-group">
    <div class="settings-row">
      <div class="settings-row-main">
        <div class="settings-row-label">多设备同步</div>
        <div class="settings-row-desc">
          用同一个 6 位同步码在几台设备间共享剪贴板历史。内容在本机加密后才上传，
          中继服务看到的只有密文
        </div>
      </div>
      <div class="settings-row-control">
        <el-switch :model-value="config.enabled" @update:model-value="handleToggle(Boolean($event))" />
      </div>
    </div>

    <template v-if="config.enabled">
      <!-- 第一屏只保留主线：状态、同步码；低频选项收进下方「高级设置」 -->
      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">同步状态</div>
          <div class="settings-row-desc">
            {{ statusText }}
            <template v-if="statusWarning">
              <el-icon class="sync-warn"><Warning /></el-icon>
            </template>
          </div>
        </div>
        <div class="settings-row-control">
          <el-button :loading="busy || status.busy" @click="handleSyncNow">立即同步</el-button>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">同步码</div>
          <div class="settings-row-desc">
            6 位数字就是配对密码：在另一台设备输入同一串即可配对，泄露后点「换新」重新生成
          </div>
          <div class="sync-self">本机：{{ config.deviceName }}</div>
        </div>
        <div class="settings-row-control">
          <code class="sync-code sync-code--short">{{ formattedCode }}</code>
          <el-button :icon="CopyDocument" size="small" @click="handleCopy">复制</el-button>
          <el-button :icon="Refresh" size="small" @click="handleRotate">换新</el-button>
        </div>
      </div>
    </template>

    <div v-if="relayMissing && !config.lanOnly" class="settings-row">
      <div class="settings-row-main">
        <div class="settings-row-label">同步服务未配置</div>
        <div class="settings-row-desc">
          当前安装包没有注入中继地址，同步无法连接。请使用官方最新安装包，或自建中继后重新构建
        </div>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row-main">
        <div class="settings-row-label">在另一台设备加入</div>
        <div class="settings-row-desc">
          在新设备上输入这 6 位数字，它就会和历史合并到同一个房间
        </div>
      </div>
      <div class="settings-row-control">
        <el-input
          v-model="joinInput"
          placeholder="6 位数字，例如 123 456"
          inputmode="numeric"
          style="width: 180px"
          @keyup.enter="handleJoin"
        />
        <el-button type="primary" :loading="busy" @click="handleJoin">加入</el-button>
      </div>
    </div>

    <!-- 低频选项收进折叠区：默认视图只保留开关、状态、同步码与加入 -->
    <div
      v-if="config.enabled"
      class="settings-row sync-advanced-row"
      @click="advancedOpen = !advancedOpen"
    >
      <div class="settings-row-main">
        <div class="settings-row-label">
          高级设置
          <el-tag
            v-if="advancedDirty"
            size="small"
            type="warning"
            effect="plain"
            class="sync-advanced-tag"
          >
            已自定义
          </el-tag>
        </div>
        <div class="settings-row-desc">局域网直连、手动添加设备、网络诊断、同步方向与图片同步</div>
      </div>
      <div class="settings-row-control">
        <el-button size="small" text>
          {{ advancedOpen ? "收起" : "展开" }}
          <el-icon class="sync-advanced-arrow" :class="{ 'is-open': advancedOpen }">
            <ArrowDown />
          </el-icon>
        </el-button>
      </div>
    </div>

    <template v-if="config.enabled && advancedOpen">
      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">局域网直连</div>
          <div class="settings-row-desc">
            同一 Wi-Fi 下直接互联，不经过公网中继；关闭时中继与局域网并行，哪条通都能同步
          </div>
        </div>
        <div class="settings-row-control">
          <el-switch
            :model-value="config.lanOnly"
            @update:model-value="handleLanToggle(Boolean($event))"
          />
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">手动添加设备</div>
          <div class="settings-row-desc">
            自动发现收不到时，填对方 IP 走纯 TCP 直连；两台互填最稳
            <div class="sync-self">本机地址：{{ localAddrsText }}</div>
            <div v-if="manualPeers.length" class="sync-peers">
              <el-tag
                v-for="peer in manualPeers"
                :key="peer"
                size="small"
                closable
                @close="handleRemovePeer(peer)"
                >{{ peer }}</el-tag
              >
            </div>
          </div>
        </div>
        <div class="settings-row-control">
          <el-input
            v-model="peerInput"
            placeholder="192.168.1.8"
            style="width: 160px"
            @keyup.enter="handleAddPeer"
          />
          <el-button :loading="peerBusy" @click="handleAddPeer">添加</el-button>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">网络诊断</div>
          <div class="settings-row-desc">
            {{ diagnosisText || "点「检查」逐项确认端口监听、房间配置与邻居握手" }}
          </div>
        </div>
        <div class="settings-row-control">
          <el-button :icon="Search" :loading="diagBusy" @click="handleDiagnose">检查</el-button>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">同步方向</div>
          <div class="settings-row-desc">
            只发送 / 只接收时，反方向的内容不会流动；切回双向会把攒下的内容补齐
          </div>
        </div>
        <div class="settings-row-control">
          <el-radio-group
            :model-value="config.mode"
            size="small"
            @update:model-value="handleModeChange($event as SyncMode)"
          >
            <el-radio-button value="both">双向</el-radio-button>
            <el-radio-button value="send">只发送</el-radio-button>
            <el-radio-button value="receive">只接收</el-radio-button>
          </el-radio-group>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">同步图片</div>
          <div class="settings-row-desc">
            图片压缩到 1280px 后加密上传，会明显增加流量和等待时间
          </div>
        </div>
        <div class="settings-row-control">
          <el-switch
            :model-value="config.syncImages"
            @update:model-value="handleImagesToggle(Boolean($event))"
          />
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
/* 本机标识：弱化处理，只在用户想核对“哪台是哪台”时才需要 */
.sync-self {
  margin-top: 2px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.sync-code {
  display: inline-block;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  padding: 4px 10px;
  border-radius: 6px;
  background: var(--el-fill-color-light);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  letter-spacing: 0.4px;
  vertical-align: middle;
}

.sync-code--short {
  max-width: none;
  padding: 6px 14px;
  font-size: 16px;
  font-weight: 600;
  letter-spacing: 2px;
}

.sync-warn {
  margin-left: 6px;
  color: var(--el-color-warning);
  vertical-align: -2px;
}

/* 手填的设备：一行小标签，一眼看清都连了哪些地址 */
.sync-peers {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 6px;
}

/* 高级设置折叠行：整行可点，箭头指示展开状态 */
.sync-advanced-row {
  cursor: pointer;
  user-select: none;
}

.sync-advanced-tag {
  margin-left: 8px;
  vertical-align: 1px;
}

.sync-advanced-arrow {
  margin-left: 4px;
  transition: transform 0.15s ease;
}

.sync-advanced-arrow.is-open {
  transform: rotate(180deg);
}
</style>
