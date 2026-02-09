<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from "vue";
import { ElMessage } from "element-plus";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen } from "@tauri-apps/api/event";
import type { ClipboardItem, DateRangeType, DateRange } from "@/types";
import { fetchHistoryByDate, setClipboardAndPaste } from "@/services/api";

const items = ref<ClipboardItem[]>([]);
const loading = ref(false);
const activeDate = ref<DateRangeType>("today");
const customDate = ref<Date>(new Date());

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
  loading.value = true;
  try {
    const range = getDateRange(activeDate.value);
    const data = await fetchHistoryByDate(range.startTs, range.endTs, '');
    items.value = data;
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

async function handleItemClick(item: ClipboardItem) {
  try {
    // 先隐藏窗口，让焦点回到原输入框
    const appWindow = getCurrentWebviewWindow();
    await appWindow.hide();
    
    // 等待一小段时间确保窗口已隐藏
    await new Promise(resolve => setTimeout(resolve, 100));
    
    // 然后写入剪贴板并模拟粘贴
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

function imageSrc(item: ClipboardItem) {
  if (!item.imageBase64) return "";
  return `data:image/png;base64,${item.imageBase64}`;
}

onMounted(async () => {
  await loadHistory();
  
  // 监听剪贴板更新事件
  try {
    const unlisten = await listen("clipboard://updated", () => {
      console.log("PopupWindow: clipboard://updated event received");
      loadHistory();
    });
    unlistenHandle.value = unlisten;
    console.log("PopupWindow: clipboard://updated listener registered");
  } catch (err) {
    console.error("PopupWindow: clipboard://updated listener failed", err);
  }
  
  const appWindow = getCurrentWebviewWindow();
  
  // 监听窗口失焦事件 - 使用 blur 事件
  try {
    window.addEventListener('blur', () => {
      appWindow.hide();
    });
  } catch (err) {
    console.error("Failed to setup blur listener", err);
  }
  
  // 监听窗口显示事件，滚动到顶部
  try {
    const unlistenShow = await appWindow.listen('tauri://focus', () => {
      console.log("PopupWindow: window focused, scrolling to top");
      if (popupBodyRef.value) {
        popupBodyRef.value.scrollTop = 0;
      }
    });
    // 也可以在这里保存 unlistenShow 以便清理
  } catch (err) {
    console.error("Failed to setup focus listener", err);
  }
});

onBeforeUnmount(() => {
  if (unlistenHandle.value) {
    unlistenHandle.value();
  }
});

watch(customDate, () => {
  if (activeDate.value === "custom") {
    loadHistory();
  }
});
</script>

<template>
  <div class="popup-shell">
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
          <span>{{ formatTime(item.createdAt) }}</span>
        </div>
        <div v-if="item.format === 'image'" class="history-image-preview">
          <img :src="imageSrc(item)" class="thumbnail" alt="预览" />
        </div>
        <div v-else class="history-preview" v-html="highlightText(shortPreview(item), '')" />
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
}

.popup-header {
  padding: 12px;
  border-bottom: 1px solid var(--border);
}

.popup-body {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.date-tabs {
  padding: 0 12px 8px;
  border-bottom: 1px solid var(--border);
}
</style>
