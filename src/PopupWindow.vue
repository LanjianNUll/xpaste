<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from "vue";
import { ElMessage } from "element-plus";
import { Search } from "@element-plus/icons-vue";
import type { ClipboardItem, DateRangeType, DateRange } from "@/types";
import {
  fetchHistoryPage,
  hidePopup,
  setClipboardAndPaste,
  subscribeClipboardUpdates
} from "@/services/api";
import LazyClipboardImage from "@/components/LazyClipboardImage.vue";

const items = ref<ClipboardItem[]>([]);
const loading = ref(false);
const activeDate = ref<DateRangeType>("today");
const customDate = ref<Date>(new Date());
const keyword = ref("");
const selectedType = ref("all");
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
const popupBodyRef = ref<HTMLElement | null>(null);

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
  try {
    await setClipboardAndPaste(item.id);
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
  hidePopup().catch(() => undefined);
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    hidePopup().catch(() => undefined);
  }
}

onMounted(async () => {
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
        v-for="item in items"
        :key="item.id"
        class="history-item"
        @click="handleItemClick(item)"
      >
        <div class="history-meta">
          <span>{{ categoryLabel[item.category] }} / {{ formatLabel[item.format] }}</span>
          <span class="history-meta-right">
            <span class="copy-count">× {{ item.copyCount }}</span>
            <span>{{ formatTime(item.createdAt) }}</span>
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

.popup-body .history-meta {
  margin-bottom: 4px;
}

.history-meta-right {
  display: inline-flex;
  align-items: center;
  gap: 8px;
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
</style>
