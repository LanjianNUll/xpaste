<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from "vue";
import { ElMessage } from "element-plus";
import { Search } from "@element-plus/icons-vue";
import type { ClipboardItem, DateRangeType, DateRange } from "@/types";
import {
  fetchHistoryPage,
  getClipboardImage,
  hidePopup,
  pasteHistoryItem,
  subscribeClipboardUpdates,
  subscribePopupShown
} from "@/services/api";
import LazyClipboardImage from "@/components/LazyClipboardImage.vue";

const items = ref<ClipboardItem[]>([]);
const loading = ref(false);
const activeDate = ref<DateRangeType>("today");
const customDate = ref<Date>(new Date());
const keyword = ref("");
const selectedType = ref("all");
const activeIndex = ref(-1);
const pasting = ref(false);
let loadRequestId = 0;

const categoryLabel: Record<ClipboardItem["category"], string> = {
  link: "链接",
  image: "图片",
  text: "文本",
  file: "文件"
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

// 图片放大预览：快捷窗口只有 360x500，因此支持滚轮缩放与拖拽平移。
const previewVisible = ref(false);
const previewUrl = ref("");
const previewBodyRef = ref<HTMLElement | null>(null);
const zoom = ref(1);
const MIN_ZOOM = 1;
const MAX_ZOOM = 8;
let panState: { x: number; y: number; left: number; top: number } | null = null;

const previewImageStyle = computed(() =>
  zoom.value === MIN_ZOOM
    ? { width: "auto", maxWidth: "100%", maxHeight: "100%" }
    : { width: `${zoom.value * 100}%`, maxWidth: "none", maxHeight: "none" }
);

async function openImagePreview(item: ClipboardItem) {
  try {
    const data = await getClipboardImage(item.id);
    previewUrl.value = `data:image/png;base64,${data}`;
    zoom.value = MIN_ZOOM;
    previewVisible.value = true;
  } catch {
    ElMessage.error("加载图片失败");
  }
}

function closeImagePreview() {
  previewVisible.value = false;
  previewUrl.value = "";
  zoom.value = MIN_ZOOM;
  panState = null;
}

function handlePreviewWheel(event: WheelEvent) {
  event.preventDefault();
  const factor = event.deltaY < 0 ? 1.25 : 1 / 1.25;
  zoom.value = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Number((zoom.value * factor).toFixed(3))));
}

function startPan(event: PointerEvent) {
  const body = previewBodyRef.value;
  if (!body || zoom.value === MIN_ZOOM) return;
  panState = { x: event.clientX, y: event.clientY, left: body.scrollLeft, top: body.scrollTop };
  body.setPointerCapture(event.pointerId);
}

function movePan(event: PointerEvent) {
  const body = previewBodyRef.value;
  if (!body || !panState) return;
  body.scrollLeft = panState.left - (event.clientX - panState.x);
  body.scrollTop = panState.top - (event.clientY - panState.y);
}

function endPan() {
  panState = null;
}

function resetZoom() {
  zoom.value = MIN_ZOOM;
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

async function loadHistory() {
  const requestId = ++loadRequestId;
  loading.value = true;
  try {
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
    await pasteHistoryItem(item.id);
    // 粘贴成功才说明这次搜索确实有用，此时把关键词记进历史搜索。
    recordSearchKeyword(searchKeyword);
  } catch (err) {
    ElMessage.error("写入剪贴板失败。");
  } finally {
    pasting.value = false;
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

function shortPreview(item: ClipboardItem) {
  if (item.format === "image") {
    return "[图片]";
  }
  if (item.format === "file") {
    return item.filePath ?? "[文件]";
  }
  if (item.format === "color") {
    return item.color ?? item.text ?? "[颜色]";
  }
  return item.text ?? item.html ?? "";
}

function handleWindowBlur() {
  closeImagePreview();
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
  historyExpanded.value = false;
  closeImagePreview();
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
    // 优先级：图片预览 → 展开的历史搜索 → 隐藏快捷窗口。
    if (previewVisible.value) {
      closeImagePreview();
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
  if (previewVisible.value) return;

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
    <div class="search-toolbar">
      <el-input
        ref="searchInputRef"
        v-model="keyword"
        :prefix-icon="Search"
        placeholder="搜索剪贴板内容"
        clearable
        @input="scheduleLoad"
      />
      <el-select
        v-model="selectedType"
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

    <div v-if="searchHistory.length > 0" class="search-history">
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

    <div class="date-tabs">
      <el-tabs v-model="activeDate" @tab-change="handleDateChange" size="small">
        <el-tab-pane label="今天" name="today" />
        <el-tab-pane label="昨天" name="yesterday" />
        <el-tab-pane label="前天" name="beforeYesterday" />
        <el-tab-pane label="自定义" name="custom">
          <el-date-picker
            v-model="customDate"
            type="date"
            placeholder="选择日期"
            size="small"
            style="width: 100%; margin-top: 8px"
          />
        </el-tab-pane>
      </el-tabs>
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
          <span>{{ categoryLabel[item.category] }} / {{ formatLabel[item.format] }}</span>
          <span class="history-meta-right">
            <span class="copy-count">× {{ item.copyCount }}</span>
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
          <LazyClipboardImage :item-id="item.id" class="thumbnail" alt="预览" />
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
      <span>↑↓ 选择 · Enter 粘贴 · Esc 关闭</span>
    </div>

    <div v-if="previewVisible" class="image-preview-overlay">
      <div class="image-preview-header">
        <span>图片预览</span>
        <button type="button" class="image-preview-close" @click="closeImagePreview">✕</button>
      </div>
      <div
        ref="previewBodyRef"
        class="image-preview-body"
        :class="{ pannable: zoom > MIN_ZOOM }"
        @wheel="handlePreviewWheel"
        @pointerdown="startPan"
        @pointermove="movePan"
        @pointerup="endPan"
        @pointercancel="endPan"
      >
        <img
          :src="previewUrl"
          :style="previewImageStyle"
          class="image-preview-full"
          alt="放大预览"
          draggable="false"
          @dblclick="resetZoom"
        />
      </div>
      <div class="image-preview-footer">
        <span>滚轮缩放 · 拖拽平移 · 双击还原</span>
        <span>{{ Math.round(zoom * 100) }}%</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.popup-shell {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--panel);
  overflow: hidden;
  border-top: 1px solid #d6d6d6;
}

.search-toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 118px;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
  background: #fafafa;
}

.search-toolbar :deep(.el-input__wrapper),
.search-toolbar :deep(.el-select__wrapper) {
  min-height: 32px;
}

.search-history {
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  background: #fafafa;
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
  border-radius: 2px;
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
  background: #e5f3fb;
  color: var(--accent);
}

.popup-body {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  background: #ffffff;
}

.date-tabs {
  padding: 0 12px 4px;
  border-bottom: 1px solid var(--border);
  background: #fafafa;
}

.date-tabs :deep(.el-tabs__header) {
  margin: 0;
}

.date-tabs :deep(.el-tabs__nav-wrap::after) {
  height: 1px;
  background: #dedede;
}

.date-tabs :deep(.el-tabs__item) {
  height: 42px;
  padding: 0 17px;
  font-size: 14px;
}

.popup-body .history-item {
  margin-bottom: 6px;
  padding: 9px 10px;
  background: #f7f7f7;
}

.popup-body .history-item:last-child {
  margin-bottom: 0;
}

.popup-body .history-item:hover {
  background: #e5f3fb;
  border-color: #99c9ed;
}

.popup-body .history-item.is-active {
  background: #e5f3fb;
  border-color: #409eff;
  box-shadow: inset 0 0 0 1px #409eff;
}

.popup-body .history-meta {
  margin-bottom: 4px;
}

.history-meta-right {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
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
  color: #1769aa;
  font-weight: 600;
  background: #e5f3fb;
  border-radius: 10px;
}

.popup-body .history-preview {
  font-size: 14px;
  line-height: 1.45;
}

.popup-footer {
  flex-shrink: 0;
  padding: 5px 12px;
  font-size: 11px;
  color: var(--muted);
  text-align: center;
  border-top: 1px solid var(--border);
  background: #fafafa;
}

.image-preview-overlay {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: flex;
  flex-direction: column;
  background: #1f1f1f;
}

.image-preview-header,
.image-preview-footer {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 10px;
  font-size: 12px;
  color: #dcdcdc;
  background: #2d2d2d;
}

.image-preview-close {
  border: none;
  background: transparent;
  color: #dcdcdc;
  font-size: 14px;
  line-height: 1;
  padding: 2px 6px;
  cursor: pointer;
}

.image-preview-close:hover {
  color: #ffffff;
  background: #3f3f3f;
}

.image-preview-body {
  flex: 1;
  display: flex;
  overflow: auto;
  min-height: 0;
}

.image-preview-body.pannable {
  cursor: grab;
}

.image-preview-body.pannable:active {
  cursor: grabbing;
}

.image-preview-full {
  flex: none;
  margin: auto;
  display: block;
  user-select: none;
  background: #ffffff;
}
</style>
