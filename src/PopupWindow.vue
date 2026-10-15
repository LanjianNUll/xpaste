<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from "vue";
import { ElMessage } from "element-plus";
import { Search, Filter, ArrowDown, Star, StarFilled } from "@element-plus/icons-vue";
import type { ClipboardItem, DateRangeType, DateRange, ItemSource } from "@/types";
import {
  fetchHistoryPage,
  fetchFavorites,
  getClipboardImage,
  hidePopup,
  pasteHistoryItem,
  toggleFavorite,
  removeFavorite,
  subscribeClipboardUpdates,
  subscribePopupShown
} from "@/services/api";
import { closeImageViewer, imageViewerVisible, openImageViewer } from "@/services/imageViewer";
import LazyClipboardImage from "@/components/LazyClipboardImage.vue";
import ImageViewer from "@/components/ImageViewer.vue";
import AmbientLayer from "@/components/AmbientLayer.vue";

const items = ref<ClipboardItem[]>([]);
const loading = ref(false);
const activeDate = ref<DateRangeType>("today");
const customDate = ref<Date>(new Date());
const keyword = ref("");
const selectedType = ref("all");
const favoritesOnly = ref(false);
const activeIndex = ref(-1);
const pasting = ref(false);
let loadRequestId = 0;

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
const unlistenFocusHandle = ref<(() => void) | null>(null);
const unlistenShownHandle = ref<(() => void) | null>(null);
const popupBodyRef = ref<HTMLElement | null>(null);
const searchInputRef = ref<{ focus: () => void } | null>(null);

// 历史搜索：只在"用某关键词搜到结果并真的粘贴了该结果"之后才入库，
// 所以历史里都是真正用过的词，不会堆满无效搜索。
const SEARCH_HISTORY_KEY = "xpaste.searchHistory";
const SEARCH_HISTORY_LIMIT = 20;
const SEARCH_HISTORY_COLLAPSED_LIMIT = 3;

const searchHistory = ref<string[]>([]);
const historyExpanded = ref(false);

const visibleSearchHistory = computed(() =>
  historyExpanded.value
    ? searchHistory.value
    : searchHistory.value.slice(0, SEARCH_HISTORY_COLLAPSED_LIMIT)
);

// 筛选面板：默认收起，避免类型/日期/历史搜索长期占用列表高度。
// 只有搜索框常驻，点击右侧"筛选"按钮才展开其余筛选条件。
const filterExpanded = ref(false);

// 日期分段按钮（替代原来的 el-tabs，高度从 46px 压到 28px）
const dateOptions: { label: string; value: DateRangeType }[] = [
  { label: "今天", value: "today" },
  { label: "昨天", value: "yesterday" },
  { label: "前天", value: "beforeYesterday" },
  { label: "自定义", value: "custom" }
];

const typeLabelMap: Record<string, string> = {
  all: "全部类型",
  text: "文本",
  link: "链接",
  image: "图片",
  file: "文件",
  html: "HTML",
  color: "颜色"
};

// 生效中的非默认筛选条件数量，用于在筛选按钮上做角标提示
const activeFilterCount = computed(() => {
  let count = 0;
  if (selectedType.value !== "all") count += 1;
  if (!favoritesOnly.value && activeDate.value !== "today") count += 1;
  return count;
});

// 底部常驻摘要：筛选面板收起时也能一眼看出当前在看哪个范围
const filterSummary = computed(() => {
  const parts: string[] = [];
  if (favoritesOnly.value) {
    parts.push("收藏");
  } else {
    parts.push(
      activeDate.value === "custom"
        ? `${customDate.value.getMonth() + 1}月${customDate.value.getDate()}日`
        : (dateOptions.find((option) => option.value === activeDate.value)?.label ?? "今天")
    );
  }
  parts.push(typeLabelMap[selectedType.value] ?? "全部类型");
  const trimmedKeyword = keyword.value.trim();
  if (trimmedKeyword) {
    parts.push(`包含“${trimmedKeyword}”`);
  }
  return parts.join(" · ");
});

function toggleFilterPanel() {
  filterExpanded.value = !filterExpanded.value;
}

function closeFilterPanel() {
  filterExpanded.value = false;
}

/** 收藏/全部切换：点亮后列表只显示收藏快照（仍支持关键词与类型筛选）。 */
function toggleFavoritesOnly() {
  favoritesOnly.value = !favoritesOnly.value;
  loadHistory();
}

function selectDate(value: DateRangeType) {
  activeDate.value = value;
  loadHistory();
}

// 图片预览统一走全局组件 ImageViewer，这里只负责按需读取原图并打开
/** 打开全局全屏预览：与主界面、历史管理共用同一个组件 */
async function openImagePreview(item: ClipboardItem) {
  try {
    const data = await getClipboardImage(item.id, true, currentSource());
    if (!data) {
      ElMessage.warning("图片已失效或读取失败");
      return;
    }
    openImageViewer(`data:image/png;base64,${data}`, `剪贴板图片_${formatTime(item.createdAt).replace(":", "")}`);
  } catch {
    ElMessage.error("加载图片失败");
  }
}

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

/** 当前列表的数据来源：收藏模式下条目 id 指向 favorites 表快照。 */
function currentSource(): ItemSource | undefined {
  return favoritesOnly.value ? "favorite" : undefined;
}

async function loadHistory() {
  const requestId = ++loadRequestId;
  loading.value = true;
  try {
    if (favoritesOnly.value) {
      const favorites = await fetchFavorites(
        keyword.value,
        selectedType.value === "all" ? undefined : selectedType.value
      );
      if (requestId === loadRequestId) {
        items.value = favorites;
        activeIndex.value = favorites.length > 0 ? 0 : -1;
        scrollActiveIntoView();
      }
      return;
    }
    const range = getDateRange(activeDate.value);
    const result = await fetchHistoryPage({
      startTs: range.startTs,
      endTs: range.endTs,
      keyword: keyword.value,
      formats: selectedType.value === "all" ? [] : [selectedType.value],
      categories: [],
      page: 1,
      pageSize: 100
    });
    if (requestId === loadRequestId) {
      items.value = result.items;
      activeIndex.value = result.items.length > 0 ? 0 : -1;
      scrollActiveIntoView();
    }
  } catch (err) {
    ElMessage.error("加载历史失败，请稍后重试。");
  } finally {
    if (requestId === loadRequestId) {
      loading.value = false;
    }
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

async function handleItemClick(item: ClipboardItem) {
  if (pasting.value) return;
  pasting.value = true;
  const searchKeyword = keyword.value.trim();
  try {
    await pasteHistoryItem(item.id, currentSource());
    // 粘贴成功才说明这次搜索确实有用，此时把关键词记进历史搜索。
    recordSearchKeyword(searchKeyword);
  } catch (err) {
    // Rust 侧会在这里回传可操作的原因（例如 macOS 未授予辅助功能权限），
    // 直接展示比笼统的"失败"更有用。
    ElMessage.error(typeof err === "string" ? err : "写入剪贴板失败。");
  } finally {
    pasting.value = false;
  }
}

/** 收藏切换：全部模式下按历史 id 切换；收藏模式下星标即取消收藏（条目移除）。 */
async function handleToggleFavorite(item: ClipboardItem) {
  try {
    if (favoritesOnly.value) {
      await removeFavorite(item.id);
    } else {
      await toggleFavorite(item.id);
    }
    await loadHistory();
  } catch (err) {
    ElMessage.error("收藏操作失败。");
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

function handleWindowBlur() {
  closeImageViewer();
  hidePopup().catch(() => undefined);
}

/** 自绘标题栏上的关闭按钮：与 Esc、失焦一致，只隐藏窗口 */
function handleCloseClick() {
  hidePopup().catch(() => undefined);
}

function loadSearchHistory() {
  try {
    const raw = window.localStorage.getItem(SEARCH_HISTORY_KEY);
    if (!raw) return;
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return;
    searchHistory.value = parsed
      .filter((entry): entry is string => typeof entry === "string" && entry.trim().length > 0)
      .slice(0, SEARCH_HISTORY_LIMIT);
  } catch {
    searchHistory.value = [];
  }
}

function persistSearchHistory() {
  try {
    window.localStorage.setItem(SEARCH_HISTORY_KEY, JSON.stringify(searchHistory.value));
  } catch {
    // localStorage 不可用时静默降级，不影响搜索本身
  }
}

function recordSearchKeyword(rawKeyword: string) {
  const value = rawKeyword.trim();
  if (!value) return;
  searchHistory.value = [
    value,
    ...searchHistory.value.filter((entry) => entry !== value)
  ].slice(0, SEARCH_HISTORY_LIMIT);
  persistSearchHistory();
}

function clearSearchHistory() {
  searchHistory.value = [];
  historyExpanded.value = false;
  persistSearchHistory();
}

function applySearchHistory(value: string) {
  historyExpanded.value = false;
  keyword.value = value;
  loadHistory();
  // 点完历史项把焦点交回搜索框，方便继续改词或直接 ↑↓ + Enter 粘贴。
  focusSearchInput();
}

function focusSearchInput() {
  searchInputRef.value?.focus();
}

function isTypingTarget(target: unknown) {
  const element = target as HTMLElement | null;
  if (!element) return false;
  const tag = element.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || element.isContentEditable;
}

/**
 * 快捷窗口每次显示时复位：清空搜索词、收起历史搜索、关闭图片预览、
 * 列表回顶并重新加载，最后把焦点交给搜索框，让用户可以直接打字搜索。
 */
async function handlePopupShown() {
  if (debounceHandle.value != null) {
    window.clearTimeout(debounceHandle.value);
    debounceHandle.value = null;
  }
  keyword.value = "";
  favoritesOnly.value = false;
  historyExpanded.value = false;
  // 每次弹出都回到收起状态，保证列表拿到最大可视高度
  closeFilterPanel();
  closeImageViewer();
  await loadHistory();
  if (popupBodyRef.value) {
    popupBodyRef.value.scrollTop = 0;
  }
  await nextTick();
  focusSearchInput();
}

function scrollActiveIntoView() {
  requestAnimationFrame(() => {
    const body = popupBodyRef.value;
    if (!body) return;
    const el = body.querySelector<HTMLElement>(".history-item.is-active");
    if (!el) return;
    const elTop = el.offsetTop;
    const elBottom = elTop + el.offsetHeight;
    const viewTop = body.scrollTop;
    const viewBottom = viewTop + body.clientHeight;
    if (elTop < viewTop) {
      body.scrollTop = elTop;
    } else if (elBottom > viewBottom) {
      body.scrollTop = elBottom - body.clientHeight;
    }
  });
}

function moveSelection(step: number) {
  if (items.value.length === 0) return;
  const current = activeIndex.value;
  let next = current < 0 ? (step > 0 ? 0 : items.value.length - 1) : current + step;
  if (next < 0) next = 0;
  if (next > items.value.length - 1) next = items.value.length - 1;
  activeIndex.value = next;
  scrollActiveIntoView();
}

async function confirmSelection() {
  const item = items.value[activeIndex.value];
  if (!item) return;
  await handleItemClick(item);
}

function handleKeydown(event: KeyboardEvent) {
  const key = event.key;
  const isComposing = (event as KeyboardEvent & { isComposing?: boolean }).isComposing;

  if (key === "Escape") {
    event.preventDefault();
    // 优先级：图片预览 → 筛选面板 → 展开的历史搜索 → 隐藏快捷窗口。
    if (imageViewerVisible.value) {
      closeImageViewer();
      return;
    }
    if (filterExpanded.value) {
      closeFilterPanel();
      return;
    }
    if (historyExpanded.value) {
      historyExpanded.value = false;
      return;
    }
    hidePopup().catch(() => undefined);
    return;
  }

  // 图片放大预览时，方向键/回车交给预览层处理。
  if (imageViewerVisible.value) return;

  if (isComposing) return;

  // 兜底：系统焦点还没落到搜索框时，敲下的第一个字符就把焦点交给搜索框，
  // 避免"需要先用鼠标点一下才能搜索"。
  if (!isTypingTarget(event.target) && key.length === 1 && key !== " ") {
    focusSearchInput();
  }

  if (key === "ArrowDown") {
    event.preventDefault();
    moveSelection(1);
    return;
  }

  if (key === "ArrowUp") {
    event.preventDefault();
    moveSelection(-1);
    return;
  }

  if (key === "Enter") {
    event.preventDefault();
    confirmSelection().catch(() => undefined);
  }
}

onMounted(async () => {
  loadSearchHistory();
  await loadHistory();
  
  // 监听剪贴板更新事件
  try {
    const unlisten = await subscribeClipboardUpdates(() => {
      console.log("PopupWindow: clipboard://updated event received");
      loadHistory();
    });
    unlistenHandle.value = unlisten;
    console.log("PopupWindow: clipboard://updated listener registered");
  } catch (err) {
    console.error("PopupWindow: clipboard://updated listener failed", err);
  }

  // 窗口每次被显示时复位搜索状态（清空搜索词并聚焦搜索框）
  try {
    unlistenShownHandle.value = await subscribePopupShown(() => {
      handlePopupShown().catch(() => undefined);
    });
  } catch (err) {
    console.error("PopupWindow: popup://shown listener failed", err);
  }
  
  window.addEventListener("blur", handleWindowBlur);
  window.addEventListener("keydown", handleKeydown);
  
  // 监听窗口显示事件，滚动到顶部
  try {
    const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const unlistenShow = await getCurrentWebviewWindow().listen("tauri://focus", () => {
      console.log("PopupWindow: window focused, scrolling to top");
      if (popupBodyRef.value) {
        popupBodyRef.value.scrollTop = 0;
      }
    });
    unlistenFocusHandle.value = unlistenShow;
  } catch (err) {
    console.error("Failed to setup focus listener", err);
  }
});

onBeforeUnmount(() => {
  if (unlistenHandle.value) {
    unlistenHandle.value();
  }
  if (unlistenFocusHandle.value) {
    unlistenFocusHandle.value();
  }
  if (unlistenShownHandle.value) {
    unlistenShownHandle.value();
  }
  window.removeEventListener("blur", handleWindowBlur);
  window.removeEventListener("keydown", handleKeydown);
});

watch(customDate, () => {
  if (activeDate.value === "custom") {
    loadHistory();
  }
});
</script>

<template>
  <div class="popup-shell">
    <AmbientLayer />
    <!-- 无系统边框的浮层：这条工具条同时充当拖拽区（点空白处可移动窗口） -->
    <div class="search-toolbar" data-tauri-drag-region>
      <el-input
        ref="searchInputRef"
        v-model="keyword"
        :prefix-icon="Search"
        placeholder="搜索剪贴板内容"
        size="small"
        clearable
        @input="scheduleLoad"
      />
      <button
        type="button"
        class="fav-toggle"
        :class="{ 'is-active': favoritesOnly }"
        :title="favoritesOnly ? '显示全部剪贴板内容' : '只看收藏'"
        @click="toggleFavoritesOnly"
      >
        <el-icon :size="13">
          <StarFilled v-if="favoritesOnly" />
          <Star v-else />
        </el-icon>
        <span>收藏</span>
      </button>
      <button
        type="button"
        class="filter-toggle"
        :class="{ 'is-active': filterExpanded, 'is-filtered': activeFilterCount > 0 }"
        :title="filterExpanded ? '收起筛选条件' : '展开类型、时间等筛选条件'"
        @click="toggleFilterPanel"
      >
        <el-icon><Filter /></el-icon>
        <span>筛选</span>
        <span v-if="activeFilterCount > 0" class="filter-badge">{{ activeFilterCount }}</span>
        <el-icon class="filter-arrow" :class="{ 'is-open': filterExpanded }"><ArrowDown /></el-icon>
      </button>
      <button type="button" class="popup-close" title="关闭（Esc）" @click="handleCloseClick">
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" /></svg>
      </button>
    </div>

    <div v-if="filterExpanded" class="filter-panel">
      <div class="filter-row">
        <span class="filter-label">类型</span>
        <el-select
          v-model="selectedType"
          size="small"
          class="filter-control"
          aria-label="按类型筛选"
          @change="loadHistory"
        >
          <el-option label="全部" value="all" />
          <el-option label="文本" value="text" />
          <el-option label="链接" value="link" />
          <el-option label="图片" value="image" />
          <el-option label="文件" value="file" />
          <el-option label="HTML" value="html" />
          <el-option label="颜色" value="color" />
        </el-select>
      </div>

      <!-- 收藏按收藏时间排序，与历史的时间范围语义不同，收藏模式下不展示时间筛选 -->
      <div v-if="!favoritesOnly" class="filter-row">
        <span class="filter-label">时间</span>
        <div class="date-segments">
          <button
            v-for="option in dateOptions"
            :key="option.value"
            type="button"
            class="date-segment"
            :class="{ 'is-active': activeDate === option.value }"
            @click="selectDate(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
      </div>

      <div v-if="!favoritesOnly && activeDate === 'custom'" class="filter-row">
        <span class="filter-label">日期</span>
        <el-date-picker
          v-model="customDate"
          type="date"
          placeholder="选择日期"
          size="small"
          class="filter-control"
        />
      </div>

      <div v-if="searchHistory.length > 0" class="filter-history">
        <div class="search-history-header">
          <span class="search-history-title">历史搜索</span>
          <span class="search-history-actions">
            <button
              v-if="searchHistory.length > SEARCH_HISTORY_COLLAPSED_LIMIT"
              type="button"
              class="search-history-action"
              @click="historyExpanded = !historyExpanded"
            >
              {{ historyExpanded ? "收起" : "展开" }}
            </button>
            <button type="button" class="search-history-action" @click="clearSearchHistory">
              清空
            </button>
          </span>
        </div>
        <div class="search-history-list" :class="{ 'is-expanded': historyExpanded }">
          <button
            v-for="entry in visibleSearchHistory"
            :key="entry"
            type="button"
            class="search-history-item"
            :title="entry"
            @click="applySearchHistory(entry)"
          >
            {{ entry }}
          </button>
        </div>
      </div>
    </div>

    <div class="popup-body" ref="popupBodyRef">
      <div
        v-for="(item, index) in items"
        :key="item.id"
        class="history-item"
        :class="{ 'is-active': index === activeIndex }"
        @click="handleItemClick(item)"
        @mouseenter="activeIndex = index"
      >
        <div class="history-meta">
          <span>{{ itemTypeLabel(item) }}</span>
          <span class="history-meta-right">
            <button
              type="button"
              class="fav-star-btn"
              :class="{ 'is-favorited': item.favorited }"
              :title="item.favorited ? '取消收藏' : '收藏'"
              @click.stop="handleToggleFavorite(item)"
            >
              <el-icon :size="13">
                <StarFilled v-if="item.favorited" />
                <Star v-else />
              </el-icon>
            </button>
            <!-- 收藏快照不参与复制计数，徽标只在全部模式下展示 -->
            <span v-if="!favoritesOnly" class="copy-count">× {{ item.copyCount }}</span>
            <span>{{ formatTime(item.createdAt) }}</span>
            <!-- 预览入口独立成文字按钮，图片本体不再拦截点击，保证整条 item 点击即粘贴 -->
            <button
              v-if="item.format === 'image'"
              type="button"
              class="preview-text-btn"
              title="放大预览图片"
              @click.stop="openImagePreview(item)"
            >
              预览
            </button>
          </span>
        </div>
        <div v-if="item.format === 'image'" class="history-image-preview">
          <LazyClipboardImage :item-id="item.id" :source="currentSource()" class="thumbnail" alt="预览" />
        </div>
        <div
          v-else
          class="history-preview"
          v-html="highlightText(shortPreview(item), keyword)"
        />
      </div>
      <el-empty v-if="!loading && items.length === 0" description="暂无记录" />
    </div>

    <div class="popup-footer">
      <span class="footer-summary" :title="filterSummary">{{ filterSummary }}</span>
      <span class="footer-hint">↑↓ 选择 · Enter 粘贴 · Esc 关闭</span>
    </div>

    <!-- 全局图片预览器：与主界面、历史管理共用 -->
    <ImageViewer />
  </div>
</template>

<style scoped>
.popup-shell {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--panel);
  overflow: hidden;
  /* 无系统边框后整窗由自己绘制，四周留一圈描边把浮层和桌面区分开 */
  border: 1px solid var(--stroke);
}

/* 常驻工具条：只留搜索框 + 收藏 + 筛选 + 关闭，整体高度压到最小 */
.search-toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 52px 72px 26px;
  gap: 6px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--stroke);
  background: var(--panel-subtle);
}

/* 自绘标题栏的关闭按钮 */
.popup-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 28px;
  padding: 0;
  border: 1px solid var(--stroke-strong);
  border-radius: var(--radius-sm);
  background: var(--control-bg);
  color: var(--text);
  cursor: pointer;
  transition: background-color 0.12s ease, border-color 0.12s ease, color 0.12s ease;
}

.popup-close svg {
  width: 9px;
  height: 9px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1;
}

.popup-close:hover {
  background: #c42b1c;
  border-color: #c42b1c;
  color: #ffffff;
}

.search-toolbar :deep(.el-input__wrapper) {
  min-height: 28px;
}

/* 收藏筛选开关：与筛选按钮同款观感，点亮表示当前只看收藏 */
.fav-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  height: 28px;
  padding: 0 6px;
  border: 1px solid var(--stroke-strong);
  border-radius: var(--radius-sm);
  background: var(--control-bg);
  color: var(--text);
  font-size: 12px;
  line-height: 1;
  cursor: pointer;
}

.fav-toggle:hover {
  background: var(--control-bg-hover);
  border-color: var(--border-strong-hover);
}

.fav-toggle.is-active {
  color: var(--accent);
  border-color: var(--accent);
  box-shadow: inset 0 0 0 1px var(--accent);
}

.filter-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  height: 28px;
  padding: 0 6px;
  border: 1px solid var(--stroke-strong);
  border-radius: var(--radius-sm);
  background: var(--control-bg);
  color: var(--text);
  font-size: 12px;
  line-height: 1;
  cursor: pointer;
}

.filter-toggle:hover {
  background: var(--control-bg-hover);
  border-color: var(--border-strong-hover);
}

.filter-toggle.is-active {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 1px var(--accent);
}

/* 有生效中的非默认条件时高亮，收起面板也能看出来筛过了 */
.filter-toggle.is-filtered {
  color: var(--accent);
  border-color: var(--accent);
}

.filter-arrow {
  transition: transform 0.15s ease;
}

.filter-arrow.is-open {
  transform: rotate(180deg);
}

.filter-badge {
min-width: 14px;
  height: 14px;
  padding: 0 3px;
  border-radius: var(--radius-pill);
  background: var(--accent);
  color: var(--accent-contrast);
  font-size: 10px;
  line-height: 14px;
  text-align: center;
}

/* 筛选面板：按需展开，收起时完全不占高度 */
.filter-panel {
  flex-shrink: 0;
  /* 最多占窗口 45%，保证展开时列表仍有超过一半的高度 */
  max-height: 45%;
  overflow-y: auto;
  padding: 8px;
  border-bottom: 1px solid var(--stroke);
  background: var(--panel-subtle);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.filter-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-label {
  flex-shrink: 0;
  width: 28px;
  color: var(--muted);
  font-size: 12px;
}

.filter-control {
  flex: 1;
  min-width: 0;
}

.filter-panel :deep(.el-input__wrapper),
.filter-panel :deep(.el-select__wrapper) {
  min-height: 28px;
}

/* 日期分段按钮：替代原 el-tabs，高度 46px → 28px */
.date-segments {
  display: flex;
  flex: 1;
  gap: 4px;
}

.date-segment {
  flex: 1;
  height: 28px;
  padding: 0 4px;
  border: 1px solid var(--stroke-strong);
  border-radius: var(--radius-sm);
  background: var(--control-bg);
  color: var(--text);
  font-size: 12px;
  line-height: 1;
  cursor: pointer;
}

.date-segment:hover {
  background: var(--control-bg-hover);
  border-color: var(--border-strong-hover);
}

.date-segment.is-active {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--accent-contrast);
}

.filter-history {
  padding-top: 4px;
  border-top: 1px dashed var(--stroke);
}

.search-history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 4px;
}

.search-history-title {
  font-size: 12px;
  color: var(--muted);
}

.search-history-actions {
  display: inline-flex;
  align-items: center;
  gap: 10px;
}

.search-history-action {
  padding: 0;
  border: none;
  background: transparent;
  color: var(--accent);
  font-size: 12px;
  line-height: 1.4;
  cursor: pointer;
}

.search-history-action:hover {
  color: var(--accent-hover);
  text-decoration: underline;
}

.search-history-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

/* 展开后固定在区域高度内滚动，不把下面的剪贴板列表挤没 */
.search-history-list.is-expanded {
  max-height: 120px;
  overflow-y: auto;
  padding-right: 2px;
}

.search-history-item {
  display: block;
  width: 100%;
  height: 24px;
  padding: 3px 6px;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text);
  font-size: 13px;
  line-height: 18px;
  text-align: left;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
}

.search-history-item:hover {
  background: var(--selection-bg);
  color: var(--accent);
}

.popup-body {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  /* 不再单独铺底色：交由 .popup-shell 一层半透明面板承担，
     皮肤背景才能从列表后面透出来（经典皮肤下与原来外观一致） */
  background: transparent;
}

.popup-body .history-item {
  margin-bottom: 6px;
  padding: 9px 10px;
  background: var(--panel-alt);
}

.popup-body .history-item:last-child {
  margin-bottom: 0;
}

.popup-body .history-item:hover {
  background: var(--hover-bg);
  border-color: var(--selection-border);
}

.popup-body .history-item.is-active {
  background: var(--selection-bg);
  border-color: var(--accent);
  box-shadow: inset 0 0 0 1px var(--accent);
}

.popup-body .history-meta {
  margin-bottom: 4px;
}

.preview-text-btn {
  padding: 0;
  border: none;
  background: transparent;
  color: var(--accent);
  font-size: 12px;
  line-height: 1.4;
  text-decoration: underline;
  cursor: pointer;
}

.preview-text-btn:hover {
  color: var(--accent-hover);
}

.preview-text-btn:focus-visible {
  outline: 1px solid var(--accent);
  outline-offset: 1px;
}

.copy-count {
padding: 1px 6px;
  color: var(--accent);
  font-weight: 600;
  background: var(--selection-bg);
  border-radius: var(--radius-pill);
}

.popup-body .history-preview {
  font-size: 14px;
  line-height: 1.45;
}

.popup-footer {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 5px 10px;
  font-size: 11px;
  color: var(--muted);
  border-top: 1px solid var(--stroke);
  background: var(--panel-subtle);
}

/* 左侧常驻筛选摘要：面板收起时也知道当前看的是哪个范围 */
.footer-summary {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.footer-hint {
  flex-shrink: 0;
}
</style>
