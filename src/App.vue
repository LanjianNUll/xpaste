<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import { ElMessage } from "element-plus";
import hljs from "highlight.js/lib/common";
import { listen } from "@tauri-apps/api/event";
import type { ClipboardItem } from "@/types";
import { fetchHistory, setClipboard } from "@/services/api";

const query = ref("");
const items = ref<ClipboardItem[]>([]);
const loading = ref(false);
const selectedId = ref<number | null>(null);

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

async function loadHistory() {
  loading.value = true;
  try {
    const data = await fetchHistory(query.value);
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

async function handlePaste(item: ClipboardItem | null) {
  if (!item) return;
  try {
    await setClipboard(item.id);
    ElMessage.success("已写入剪贴板，可直接粘贴。");
  } catch (err) {
    ElMessage.error("写入剪贴板失败。");
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
  try {
    const unlisten = await listen("clipboard://updated", () => {
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
  if (unlistenHandle.value) {
    unlistenHandle.value();
  }
});
watch(query, scheduleLoad);
</script>

<template>
  <div class="app-shell">
    <div class="toolbar">
      <div class="toolbar-title">Paste App</div>
      <el-input
        v-model="query"
        placeholder="搜索剪贴板历史..."
        clearable
        style="max-width: 360px"
      />
      <el-button type="primary" @click="loadHistory" :loading="loading">刷新</el-button>
    </div>

    <div class="content">
      <section class="panel">
        <div class="panel-header">
          <span>历史记录</span>
          <span style="font-size: 12px; color: var(--muted)">{{ items.length }} 条</span>
        </div>
        <div class="panel-body">
          <div
            v-for="item in items"
            :key="item.id"
            class="history-item"
            :class="{ active: item.id === selectedItem?.id }"
            @click="selectItem(item)"
          >
            <div class="history-meta">
              <span>{{ categoryLabel[item.category] }} / {{ formatLabel[item.format] }}</span>
              <span>{{ formatTime(item.createdAt) }}</span>
            </div>
            <div class="history-preview" v-html="highlightText(shortPreview(item), query)" />
          </div>
          <el-empty v-if="!loading && items.length === 0" description="暂无记录" />
        </div>
      </section>

      <section class="panel">
        <div class="panel-header">
          <span>预览</span>
          <el-button type="primary" @click="handlePaste(selectedItem)">写入剪贴板</el-button>
        </div>
        <div class="panel-body" v-if="selectedItem">
          <div class="preview-title">{{ formatLabel[selectedItem.format] }}</div>

          <template v-if="selectedItem.format === 'image'">
            <img class="preview-image" :src="imageSrc(selectedItem)" alt="clipboard" />
          </template>

          <template v-else-if="selectedItem.format === 'color'">
            <div style="display: flex; align-items: center; gap: 12px;">
              <div
                style="width: 42px; height: 42px; border-radius: 8px; border: 1px solid var(--border);"
                :style="{ background: selectedItem.color || selectedItem.text || '#fff' }"
              />
              <div class="preview-text">{{ selectedItem.color || selectedItem.text }}</div>
            </div>
          </template>

          <template v-else-if="selectedItem.format === 'file'">
            <div class="preview-text">{{ selectedItem.filePath }}</div>
          </template>

          <template v-else>
            <div v-if="selectedItem.text && isCodeLike(selectedItem.text)" class="preview-code" v-html="codeHtml(selectedItem.text)" />
            <div v-else class="preview-text" v-html="highlightText(selectedItem.text || selectedItem.html || '', query)" />
          </template>
        </div>
        <div class="panel-body" v-else>
          <el-empty description="请选择一条记录" />
        </div>
      </section>
    </div>
  </div>
</template>
