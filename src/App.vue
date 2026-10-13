<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import { ElMessage } from "element-plus";
import { Calendar, DocumentCopy, Moon, Setting, Sunny } from "@element-plus/icons-vue";
import type { ClipboardItem, DateRangeType, DateRange } from "@/types";
import {
  fetchHistoryByDate,
  getClipboardImage,
  setClipboard,
  getHotkey,
  setHotkey,
  isAutostartEnabled,
  setAutostart,
  clearHistory,
  subscribeClipboardUpdates
} from "@/services/api";
import { setThemeMode } from "@/services/theme";
import { openImageViewer } from "@/services/imageViewer";
import {
  checkForUpdate,
  initAutoUpdate,
  installUpdate,
  showUpdateDialog,
  updateState
} from "@/services/update";
import { defaultHotkey, isMacOS } from "@/services/platform";
import HistoryManager from "@/components/HistoryManager.vue";
import LazyClipboardImage from "@/components/LazyClipboardImage.vue";
import ImageViewer from "@/components/ImageViewer.vue";
import SettingsShell from "@/components/SettingsShell.vue";
import AmbientLayer from "@/components/AmbientLayer.vue";
import TitleBarControls from "@/components/TitleBarControls.vue";
import UpdateDialog from "@/components/UpdateDialog.vue";

const query = ref("");
const items = ref<ClipboardItem[]>([]);
const loading = ref(false);
const selectedId = ref<number | null>(null);
const activeDate = ref<DateRangeType>("today");
const customDate = ref<Date>(new Date());
const currentHotkey = ref(defaultHotkey);
const newHotkey = ref("");
const autostart = ref(false);
const activeTab = ref("clipboard");

/**
 * 左侧主导航：固定三项，不再随功能增减而变动。
 * 后续要加模块时在这里补一项，再在模板里加对应的 v-else-if 内容分支。
 */
const mainTabs: Array<{ id: string; label: string; icon: unknown }> = [
  { id: "clipboard", label: "剪贴板", icon: DocumentCopy },
  { id: "history", label: "历史管理", icon: Calendar },
  { id: "settings", label: "设置", icon: Setting }
];

/** 侧边栏选中项对应的右侧标题 */
const activeTabLabel = computed(() => mainTabs.find((tab) => tab.id === activeTab.value)?.label ?? "");

// 当前生效的深浅色（含"跟随系统"解析后的结果）。
// 通过监听 html 的 data-theme 属性变化感知所有来源的切换：
// 快捷按钮、设置面板、以及来自快捷窗口的同步事件。
const resolvedDark = ref(document.documentElement.dataset.theme === "dark");
let themeObserver: MutationObserver | null = null;

function toggleTheme() {
  void setThemeMode(resolvedDark.value ? "light" : "dark");
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

/**
 * 点击历史记录里的图片缩略图：打开全局全屏预览。
 * 缩略图本身是懒加载的，这里按需再读一次原始 Base64，避免拿到的是压缩过的缩略图。
 */
async function handleImageClick(item: ClipboardItem) {
  try {
    const data = await getClipboardImage(item.id, true);
    if (!data) {
      ElMessage.warning("图片已失效或读取失败。");
      return;
    }
    openImageViewer(`data:image/png;base64,${data}`, `剪贴板图片_${formatTime(item.createdAt).replace(":", "")}`);
  } catch (err) {
    ElMessage.error("读取图片失败。");
  }
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

/**
 * 切换左侧导航：切到设置页时额外读一份外部状态，
 * 保证快捷键输入框与开机启动开关显示的是实际生效值。
 */
async function selectTab(id: string) {
  if (id === "settings") {
    newHotkey.value = currentHotkey.value;
    autostart.value = await isAutostartEnabled();
  }
  activeTab.value = id;
}

/**
 * 开机启动开关：与外观设置一致，改动即时生效并写入系统启动项。
 */
async function handleAutostartChange(value: boolean) {
  autostart.value = value;
  try {
    await setAutostart(value);
    ElMessage.success(value ? "已开启开机启动" : "已关闭开机启动");
  } catch (err) {
    // 失败时回滚开关，避免界面显示与系统实际状态不一致
    autostart.value = !value;
    ElMessage.error("设置开机启动失败");
  }
}

/** 快捷键防抖句柄：输入过程中不反复调后端 */
let hotkeyDebounce: number | null = null;

/**
 * 快捷键输入即时生效：停止输入 600ms 后写入，避免每敲一个字符就注册一次全局热键。
 */
function handleHotkeyChange(value: string) {
  newHotkey.value = value;
  if (hotkeyDebounce != null) {
    window.clearTimeout(hotkeyDebounce);
  }
  const next = value.trim();
  if (!next || next === currentHotkey.value) return;
  hotkeyDebounce = window.setTimeout(async () => {
    try {
      await setHotkey(next);
      currentHotkey.value = next;
      ElMessage.success(`快捷键已更新为 ${next}`);
    } catch (err) {
      newHotkey.value = currentHotkey.value;
      ElMessage.error("设置快捷键失败");
    }
  }, 600);
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
  if (hotkeyDebounce != null) {
    window.clearTimeout(hotkeyDebounce);
  }
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
    <ImageViewer />

    <!-- 左侧固定侧边栏：品牌区 + 主导航 + 底部用户区 -->
    <aside class="sidebar">
      <!-- 品牌区同时充当标题栏拖拽区；macOS 红绿灯必须在这里才贴近原生位置 -->
      <div class="sidebar-brand" data-tauri-drag-region>
        <TitleBarControls v-if="isMacOS" />
        <span class="brand-logo" aria-hidden="true">X</span>
        <span class="brand-name">Xpaste</span>
      </div>

      <nav class="sidebar-nav">
        <button
          v-for="tab in mainTabs"
          :key="tab.id"
          type="button"
          class="sidebar-item"
          :class="{ 'is-active': activeTab === tab.id }"
          :title="tab.label"
          @click="selectTab(tab.id)"
        >
          <el-icon :size="16"><component :is="tab.icon" /></el-icon>
          <span class="sidebar-item-label">{{ tab.label }}</span>
        </button>
      </nav>

      <!-- 底部用户区：登录/账户入口的预留位，功能上线前只占位不渲染内容 -->
      <div class="sidebar-footer">
        <div class="user-slot" title="用户中心即将上线" aria-hidden="true">
          <span class="user-avatar"></span>
          <span class="user-text">
            <i class="user-name">未登录</i>
            <i class="user-sub">即将上线</i>
          </span>
        </div>
      </div>
    </aside>

    <!-- 右侧主列：标题栏 + 内容区 -->
    <div class="main-column">
      <header class="main-header" data-tauri-drag-region>
        <h1 class="page-title">{{ activeTabLabel }}</h1>
        <!-- 标题栏中部的空白拖拽区：窗口主要靠它拖动，双击可最大化 -->
        <div class="header-drag" data-tauri-drag-region></div>
        <div class="header-actions">
          <button
            type="button"
            class="toolbar-icon-btn"
            :title="resolvedDark ? '切换到浅色模式' : '切换到深色模式'"
            @click="toggleTheme"
          >
            <el-icon :size="16">
              <Sunny v-if="resolvedDark" />
              <Moon v-else />
            </el-icon>
          </button>
          <TitleBarControls v-if="!isMacOS" />
        </div>
      </header>

      <main class="main-content">
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
            <div
              v-if="item.format === 'image'"
              class="history-image-preview"
              title="点击查看大图"
              @click.stop="handleImageClick(item)"
            >
              <LazyClipboardImage :item-id="item.id" class="thumbnail" alt="预览" />
              <span class="image-expand-hint">点击查看大图</span>
            </div>
            <div v-else class="history-preview" v-html="highlightText(shortPreview(item), query)" />
          </div>
          <el-empty v-if="!loading && items.length === 0" description="暂无记录" />
        </div>
      </section>
    </div>

    <!-- 历史管理页面 -->
      <div v-else-if="activeTab === 'history'" class="history-manager-container">
        <HistoryManager />
      </div>

      <!-- 设置页：与其它页面同级的右侧内容，不再是弹窗 -->
      <div v-else-if="activeTab === 'settings'" class="settings-page">
        <SettingsShell
          :active="activeTab === 'settings'"
          :current-hotkey="newHotkey"
          :autostart="autostart"
          :update-checking="updateChecking"
          :update-status-text="updateStatusText"
          :update-action-text="updateState.phase === 'ready' ? '重启并安装' : '检查更新'"
          @update:current-hotkey="handleHotkeyChange"
          @update:autostart="handleAutostartChange"
          @clear-data="clearDataInSettings"
          @update-action="handleUpdateAction"
        />
      </div>
    </main>
    </div>
  </div>
</template>
