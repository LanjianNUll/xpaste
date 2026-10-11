<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import { ElMessage } from "element-plus";
import { Moon, Sunny } from "@element-plus/icons-vue";
import hljs from "highlight.js/lib/common";
import type { ClipboardItem, DateRangeType, DateRange } from "@/types";
import {
  fetchHistoryByDate,
  setClipboard,
  getHotkey,
  setHotkey,
  isAutostartEnabled,
  setAutostart,
  clearHistory,
  subscribeClipboardUpdates
} from "@/services/api";
import { setThemeMode } from "@/services/theme";
import {
  checkForUpdate,
  initAutoUpdate,
  installUpdate,
  showUpdateDialog,
  updateState
} from "@/services/update";
import HistoryManager from "@/components/HistoryManager.vue";
import LazyClipboardImage from "@/components/LazyClipboardImage.vue";
import ThemeSettings from "@/components/ThemeSettings.vue";
import AmbientLayer from "@/components/AmbientLayer.vue";
import UpdateDialog from "@/components/UpdateDialog.vue";

const query = ref("");
const items = ref<ClipboardItem[]>([]);
const loading = ref(false);
const selectedId = ref<number | null>(null);
const activeDate = ref<DateRangeType>("today");
const customDate = ref<Date>(new Date());
const currentHotkey = ref("Win+V");
const showSettingsDialog = ref(false);
const settingsTab = ref<"appearance" | "general">("appearance");
const newHotkey = ref("");
const autostart = ref(false);
const activeTab = ref("clipboard");

// 当前生效的深浅色（含"跟随系统"解析后的结果）。
// 通过监听 html 的 data-theme 属性变化感知所有来源的切换：
// 快捷按钮、设置面板、以及来自快捷窗口的同步事件。
const resolvedDark = ref(document.documentElement.dataset.theme === "dark");
let themeObserver: MutationObserver | null = null;

function toggleTheme() {
  void setThemeMode(resolvedDark.value ? "light" : "dark");
}

/**
 * 无边框窗口的自定义标题栏按钮。
 * 先用 try 兜住 import：浏览器里预览页面时没有 Tauri 运行时，
 * 三个按钮静默失效，不影响其它功能。
 */
const isMaximized = ref(false);
let unlistenWindowResize: (() => void) | null = null;

async function currentWindow() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  } catch {
    return null;
  }
}

async function minimizeWindow() {
  const window = await currentWindow();
  if (!window) return;
  await window.minimize().catch(() => undefined);
}

async function toggleMaximizeWindow() {
  const window = await currentWindow();
  if (!window) return;
  await window.toggleMaximize().catch(() => undefined);
  isMaximized.value = await window.isMaximized().catch(() => false);
}

/** ✕ 与原来点系统关闭按钮的行为保持一致：隐藏到托盘，而不是退出应用 */
async function hideMainWindow() {
  const window = await currentWindow();
  if (!window) return;
  await window.hide().catch(() => undefined);
}

const selectedItem = computed(() => {
  if (selectedId.value != null) {
    return items.value.find((item) => item.id === selectedId.value) ?? null;
  }
  return items.value[0] ?? null;
});

const categoryLabel: Record<ClipboardItem["category"], string> = {
  link: "链接",
  image: "图片",
  text: "文本",
  file: "文件",
  folder: "文件夹"
};

const formatLabel: Record<ClipboardItem["format"], string> = {
  text: "文本",
  image: "图片",
  html: "HTML",
  file: "文件",
  color: "颜色"
};

const debounceHandle = ref<number | null>(null);
const unlistenHandle = ref<(() => void) | null>(null);

function getDateRange(type: DateRangeType): DateRange {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  
  switch(type) {
    case "today":
      return {
        startTs: today.getTime(),
        endTs: today.getTime() + 86400000 - 1
      };
    case "yesterday":
      const yesterday = new Date(today.getTime() - 86400000);
      return {
        startTs: yesterday.getTime(),
        endTs: yesterday.getTime() + 86400000 - 1
      };
    case "beforeYesterday":
      const beforeYesterday = new Date(today.getTime() - 172800000);
      return {
        startTs: beforeYesterday.getTime(),
        endTs: beforeYesterday.getTime() + 86400000 - 1
      };
    case "custom":
      const custom = new Date(customDate.value.getFullYear(), customDate.value.getMonth(), customDate.value.getDate());
      return {
        startTs: custom.getTime(),
        endTs: custom.getTime() + 86400000 - 1
      };
  }
}

async function loadHistory() {
  loading.value = true;
  try {
    const range = getDateRange(activeDate.value);
    const data = await fetchHistoryByDate(range.startTs, range.endTs, query.value);
    items.value = data;
    if (data.length === 0) {
      selectedId.value = null;
      return;
    }
    const exists = selectedId.value != null && data.some((item) => item.id === selectedId.value);
    if (!exists) {
      selectedId.value = data[0].id;
    }
  } catch (err) {
    ElMessage.error("加载历史失败，请稍后重试。");
  } finally {
    loading.value = false;
  }
}

function scheduleLoad() {
  if (debounceHandle.value != null) {
    window.clearTimeout(debounceHandle.value);
  }
  debounceHandle.value = window.setTimeout(() => {
    loadHistory();
  }, 300);
}

function selectItem(item: ClipboardItem) {
  selectedId.value = item.id;
}

async function handleItemClick(item: ClipboardItem) {
  selectItem(item);
  try {
    await setClipboard(item.id);
    ElMessage.success("已写入剪贴板，可直接粘贴。");
  } catch (err) {
    ElMessage.error("写入剪贴板失败。");
  }
}

function handleDateChange() {
  loadHistory();
}

function formatTime(ts: number) {
  const date = new Date(ts);
  return `${date.getHours().toString().padStart(2, "0")}:${date
    .getMinutes()
    .toString()
    .padStart(2, "0")}`;
}

function escapeHtml(input: string) {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function highlightText(text: string, keyword: string) {
  const safe = escapeHtml(text);
  if (!keyword.trim()) return safe;
  const escaped = keyword.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return safe.replace(new RegExp(escaped, "gi"), (match) => `<mark>${match}</mark>`);
}

function isCodeLike(text: string) {
  if (text.length < 40) return false;
  return /[;{}<>]|\b(function|const|let|var|class|def|fn|impl|use|import)\b/i.test(text);
}

function codeHtml(text: string) {
  return hljs.highlightAuto(text).value;
}

function shortPreview(item: ClipboardItem) {
  if (item.format === "image") {
    return "[图片]";
  }
  if (item.format === "file") {
    return filePreview(item);
  }
  if (item.format === "color") {
    return item.color ?? item.text ?? "[颜色]";
  }
  return item.text ?? item.html ?? "";
}

/**
 * 文件/文件夹记录的内容预览：始终展示完整路径；
 * 多选时展示首个路径并标注数量（粘贴时会写入全部路径）。
 */
function filePreview(item: ClipboardItem) {
  const paths = (item.text ?? item.filePath ?? "")
    .split(/\r?\n/)
    .map((path) => path.trim())
    .filter(Boolean);
  if (paths.length === 0) {
    return "[文件]";
  }
  if (paths.length === 1) {
    return paths[0];
  }
  return `${paths[0]} 等 ${paths.length} 个路径`;
}

/** 类型文案：文件夹单独展示，其余沿用「分类 / 格式」 */
function itemTypeLabel(item: ClipboardItem) {
  if (item.category === "folder") {
    return "文件夹";
  }
  return `${categoryLabel[item.category]} / ${formatLabel[item.format]}`;
}

async function clearData() {
  try {
    await clearHistory();
    ElMessage.success("数据已清空");
    await loadHistory();
  } catch (err) {
    ElMessage.error("清空数据失败");
  }
}

async function openSettingsDialog() {
  newHotkey.value = currentHotkey.value;
  autostart.value = await isAutostartEnabled();
  settingsTab.value = "appearance";
  showSettingsDialog.value = true;
}

async function saveSettings() {
  let success = true;
  
  // 保存快捷键
  if (newHotkey.value.trim() && newHotkey.value !== currentHotkey.value) {
    try {
      await setHotkey(newHotkey.value);
      currentHotkey.value = newHotkey.value;
    } catch (err) {
      ElMessage.error("设置快捷键失败");
      success = false;
    }
  }
  
  // 保存开机启动设置
  try {
    await setAutostart(autostart.value);
  } catch (err) {
    ElMessage.error("设置开机启动失败");
    success = false;
  }
  
  if (success) {
    showSettingsDialog.value = false;
    ElMessage.success("设置已保存");
  }
}

async function clearDataInSettings() {
  try {
    await clearHistory();
    ElMessage.success("数据已清空");
    await loadHistory();
  } catch (err) {
    ElMessage.error("清空数据失败");
  }
}

/* ===== 自动更新 ===== */

/** 设置面板里的"检查更新"：手动触发，结果直接反馈在按钮旁边 */
const updateChecking = ref(false);

const updateStatusText = computed(() => {
  switch (updateState.phase) {
    case "checking":
      return "正在检查…";
    case "available":
    case "downloading":
      return `发现新版本 v${updateState.version}，正在下载 ${updateState.percent}%`;
    case "ready":
      return `新版本 v${updateState.version} 已下载完成`;
    case "installing":
      return "正在启动安装程序…";
    case "latest":
      return `已是最新版本${updateState.currentVersion ? `（v${updateState.currentVersion}）` : ""}`;
    case "error":
      return updateState.error || "检查更新失败";
    default:
      return "每次启动会自动检查一次更新";
  }
});

async function handleCheckUpdate() {
  if (updateChecking.value) return;
  updateChecking.value = true;
  try {
    const hasUpdate = await checkForUpdate(false);
    if (hasUpdate) {
      showUpdateDialog();
    } else if (updateState.phase === "latest") {
      ElMessage.success("当前已是最新版本");
    } else if (updateState.phase === "error") {
      ElMessage.warning(updateState.error || "检查更新失败");
    }
  } finally {
    updateChecking.value = false;
  }
}

/** 已下载完成时，"检查更新"按钮变成"重启安装" */
async function handleUpdateAction() {
  if (updateState.phase === "ready") {
    await installUpdate();
    return;
  }
  await handleCheckUpdate();
}

onMounted(async () => {
  await loadHistory();

  // 启动时自动检查更新（开发模式与浏览器预览会自动跳过）
  void initAutoUpdate();

  themeObserver = new MutationObserver(() => {
    resolvedDark.value = document.documentElement.dataset.theme === "dark";
  });
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"]
  });

  // 加载当前快捷键
  currentHotkey.value = await getHotkey();
  // 加载自启动状态
  autostart.value = await isAutostartEnabled();

  // 窗口最大化状态：决定标题栏按钮显示"最大化"还是"向下还原"图标
  const window = await currentWindow();
  if (window) {
    isMaximized.value = await window.isMaximized().catch(() => false);
    unlistenWindowResize = await window
      .onResized(() => {
        void window
          .isMaximized()
          .then((value) => {
            isMaximized.value = value;
          })
          .catch(() => undefined);
      })
      .catch(() => null);
  }
  
  try {
    const unlisten = await subscribeClipboardUpdates(() => {
      console.log("clipboard://updated event received");
      loadHistory();
    });
    unlistenHandle.value = unlisten;
    console.log("clipboard://updated listener registered");
  } catch (err) {
    console.error("clipboard://updated listener failed", err);
  }
});
onBeforeUnmount(() => {
  themeObserver?.disconnect();
  unlistenWindowResize?.();
  if (unlistenHandle.value) {
    unlistenHandle.value();
  }
});
watch(query, scheduleLoad);
watch(customDate, () => {
  if (activeDate.value === "custom") {
    loadHistory();
  }
});
</script>

<template>
  <div class="app-shell">
    <AmbientLayer />
    <UpdateDialog />
    <!-- 窗口无系统边框（decorations:false），这条工具栏同时充当标题栏：
         整条可拖动，右侧是自绘的最小化 / 最大化 / 关闭按钮 -->
    <div class="toolbar" data-tauri-drag-region>
      <el-tabs v-model="activeTab" class="main-tabs">
        <el-tab-pane label="剪贴板" name="clipboard" />
        <el-tab-pane label="历史管理" name="history" />
      </el-tabs>
      <!-- 中间的空白拖拽区：窗口靠它拖动，双击可最大化 -->
      <div class="toolbar-drag" data-tauri-drag-region></div>
      <el-button
        :title="resolvedDark ? '切换到浅色模式' : '切换到深色模式'"
        @click="toggleTheme"
      >
        <el-icon>
          <Sunny v-if="resolvedDark" />
          <Moon v-else />
        </el-icon>
      </el-button>
      <el-button @click="openSettingsDialog">设置</el-button>
      <div class="window-controls">
        <button type="button" class="window-btn" title="最小化" @click="minimizeWindow">
          <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" /></svg>
        </button>
        <button
          type="button"
          class="window-btn"
          :title="isMaximized ? '向下还原' : '最大化'"
          @click="toggleMaximizeWindow"
        >
          <svg v-if="isMaximized" viewBox="0 0 10 10" aria-hidden="true">
            <rect x="0.5" y="2.5" width="7" height="7" />
            <path d="M2.5 2.5V0.5h7v7h-2" />
          </svg>
          <svg v-else viewBox="0 0 10 10" aria-hidden="true">
            <rect x="0.5" y="0.5" width="9" height="9" />
          </svg>
        </button>
        <button
          type="button"
          class="window-btn window-btn-close"
          title="隐藏到托盘"
          @click="hideMainWindow"
        >
          <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" /></svg>
        </button>
      </div>
    </div>

    <!-- 剪贴板页面 -->
    <div v-if="activeTab === 'clipboard'" class="content">
      <section class="panel">
        <div class="panel-header">
          <span>历史记录</span>
          <div style="display: flex; gap: 12px; align-items: center;">
            <el-input
              v-model="query"
              placeholder="搜索剪贴板历史..."
              clearable
              style="max-width: 240px"
            />
            <span style="font-size: 12px; color: var(--muted)">{{ items.length }} 条</span>
          </div>
        </div>
        <div class="date-tabs">
          <el-tabs v-model="activeDate" @tab-change="handleDateChange">
            <el-tab-pane label="今天" name="today" />
            <el-tab-pane label="昨天" name="yesterday" />
            <el-tab-pane label="前天" name="beforeYesterday" />
            <el-tab-pane label="自定义" name="custom">
              <el-date-picker
                v-model="customDate"
                type="date"
                placeholder="选择日期"
                style="width: 100%; margin-top: 8px"
              />
            </el-tab-pane>
          </el-tabs>
        </div>
        <div class="panel-body">
          <div
            v-for="item in items"
            :key="item.id"
            class="history-item"
            :class="{ active: item.id === selectedItem?.id }"
            @click="handleItemClick(item)"
          >
            <div class="history-meta">
              <span>{{ itemTypeLabel(item) }}</span>
              <span>{{ formatTime(item.createdAt) }}</span>
            </div>
            <div v-if="item.format === 'image'" class="history-image-preview">
              <LazyClipboardImage :item-id="item.id" class="thumbnail" alt="预览" />
            </div>
            <div v-else class="history-preview" v-html="highlightText(shortPreview(item), query)" />
          </div>
          <el-empty v-if="!loading && items.length === 0" description="暂无记录" />
        </div>
      </section>

      <section class="panel">
        <div class="panel-header">
          <span>预览</span>
        </div>
        <div class="panel-body" v-if="selectedItem">
          <div class="preview-title">{{ formatLabel[selectedItem.format] }}</div>

          <template v-if="selectedItem.format === 'image'">
            <LazyClipboardImage class="preview-image" :item-id="selectedItem.id" alt="clipboard" />
          </template>

          <template v-else-if="selectedItem.format === 'color'">
            <div style="display: flex; align-items: center; gap: 12px;">
              <div
                style="width: 42px; height: 42px; border-radius: 2px; border: 1px solid var(--border);"
                :style="{ background: selectedItem.color || selectedItem.text || '#fff' }"
              />
              <div class="preview-text">{{ selectedItem.color || selectedItem.text }}</div>
            </div>
          </template>

          <template v-else-if="selectedItem.format === 'file'">
            <div class="preview-text">{{ selectedItem.filePath }}</div>
          </template>

          <template v-else>
            <div
              v-if="selectedItem.text && isCodeLike(selectedItem.text)"
              class="preview-code"
              v-html="codeHtml(selectedItem.text)"
            />
            <div
              v-else
              class="preview-text"
              v-html="highlightText(selectedItem.text || selectedItem.html || '', query)"
            />
          </template>
        </div>
        <div class="panel-body" v-else>
          <el-empty description="请选择一条记录" />
        </div>
      </section>
    </div>

    <!-- 历史管理页面 -->
    <div v-else-if="activeTab === 'history'" class="history-manager-container">
      <HistoryManager />
    </div>
    
    <!-- 设置对话框：外观即时生效；通用项通过"保存"提交 -->
    <el-dialog
      v-model="showSettingsDialog"
      title="设置"
      width="min(620px, 94vw)"
      top="6vh"
      class="settings-dialog"
    >
      <el-tabs v-model="settingsTab">
        <el-tab-pane label="外观" name="appearance">
          <ThemeSettings :active="showSettingsDialog" />
        </el-tab-pane>

        <el-tab-pane label="通用" name="general">
          <el-form label-width="92px" class="general-form">
            <el-form-item label="全局快捷键">
              <el-input
                v-model="newHotkey"
                placeholder="例如：Win+V, Ctrl+Shift+V"
              />
              <div class="settings-tip">
                当前快捷键：<span class="hotkey-value">{{ currentHotkey }}</span><br>
                支持的修饰键：Ctrl、Alt、Shift、Win；按键：A-Z。修改后可能需要重启应用生效。
              </div>
            </el-form-item>

            <el-form-item label="开机启动">
              <el-switch v-model="autostart" />
              <span class="settings-tip" style="margin-left: 8px;">应用随系统自动启动</span>
            </el-form-item>

            <el-form-item label="数据管理">
              <el-button type="danger" @click="clearDataInSettings">清理存储</el-button>
              <span class="settings-tip" style="margin-left: 8px;">清空所有剪贴板历史记录</span>
            </el-form-item>

            <el-form-item label="软件更新">
              <el-button
                :loading="updateChecking"
                @click="handleUpdateAction"
              >
                {{ updateState.phase === "ready" ? "重启并安装" : "检查更新" }}
              </el-button>
              <span class="settings-tip" style="margin-left: 8px;">{{ updateStatusText }}</span>
            </el-form-item>
          </el-form>
        </el-tab-pane>
      </el-tabs>
      <template #footer>
        <template v-if="settingsTab === 'general'">
          <el-button @click="showSettingsDialog = false">取消</el-button>
          <el-button type="primary" @click="saveSettings">保存</el-button>
        </template>
        <el-button v-else type="primary" @click="showSettingsDialog = false">完成</el-button>
      </template>
    </el-dialog>
  </div>
</template>
