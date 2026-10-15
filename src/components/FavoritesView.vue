<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from "vue";
import { ElMessage } from "element-plus";
import { StarFilled } from "@element-plus/icons-vue";
import type { ClipboardItem } from "@/types";
import {
  fetchFavorites,
  getClipboardImage,
  removeFavorite,
  setClipboard,
  subscribeClipboardUpdates
} from "@/services/api";
import { openImageViewer } from "@/services/imageViewer";
import LazyClipboardImage from "@/components/LazyClipboardImage.vue";

const query = ref("");
const items = ref<ClipboardItem[]>([]);
const loading = ref(false);

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

/** 收藏列表展示的是收藏时间（后端把 favorited_at 映射到 createdAt）。 */
async function loadFavorites() {
  loading.value = true;
  try {
    items.value = await fetchFavorites(query.value);
  } catch (err) {
    ElMessage.error("加载收藏失败，请稍后重试。");
  } finally {
    loading.value = false;
  }
}

function scheduleLoad() {
  if (debounceHandle.value != null) {
    window.clearTimeout(debounceHandle.value);
  }
  debounceHandle.value = window.setTimeout(() => {
    loadFavorites();
  }, 300);
}

async function handleItemClick(item: ClipboardItem) {
  try {
    await setClipboard(item.id, "favorite");
    ElMessage.success("已写入剪贴板，可直接粘贴。");
  } catch (err) {
    ElMessage.error("写入剪贴板失败。");
  }
}

async function handleUnfavorite(item: ClipboardItem) {
  try {
    await removeFavorite(item.id);
    ElMessage.success("已取消收藏");
    await loadFavorites();
  } catch (err) {
    ElMessage.error("取消收藏失败。");
  }
}

/** 点击收藏里的图片缩略图：按收藏快照读取原图并打开全局预览。 */
async function handleImageClick(item: ClipboardItem) {
  try {
    const data = await getClipboardImage(item.id, true, "favorite");
    if (!data) {
      ElMessage.warning("图片已失效或读取失败。");
      return;
    }
    openImageViewer(`data:image/png;base64,${data}`, `收藏图片_${formatTime(item.createdAt).replace(":", "")}`);
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

onMounted(async () => {
  await loadFavorites();
  try {
    unlistenHandle.value = await subscribeClipboardUpdates(() => {
      loadFavorites();
    });
  } catch (err) {
    console.error("clipboard://updated listener failed for favorites", err);
  }
});

onBeforeUnmount(() => {
  if (debounceHandle.value != null) {
    window.clearTimeout(debounceHandle.value);
  }
  if (unlistenHandle.value) {
    unlistenHandle.value();
  }
});

watch(query, scheduleLoad);
</script>

<template>
  <div class="content">
    <section class="panel">
      <div class="panel-header">
        <span>收藏列表</span>
        <div class="fav-toolbar">
          <el-input
            v-model="query"
            placeholder="搜索收藏..."
            clearable
            style="max-width: 240px"
          />
          <span style="font-size: 12px; color: var(--muted)">{{ items.length }} 条</span>
        </div>
      </div>
      <div class="panel-body">
        <div
          v-for="item in items"
          :key="item.id"
          class="history-item"
          @click="handleItemClick(item)"
        >
          <div class="history-meta">
            <span>{{ itemTypeLabel(item) }}</span>
            <span class="history-meta-right">
              <button
                type="button"
                class="fav-star-btn is-favorited"
                title="取消收藏"
                @click.stop="handleUnfavorite(item)"
              >
                <el-icon :size="14"><StarFilled /></el-icon>
              </button>
              <span>{{ formatTime(item.createdAt) }}</span>
            </span>
          </div>
          <div
            v-if="item.format === 'image'"
            class="history-image-preview"
            title="点击查看大图"
            @click.stop="handleImageClick(item)"
          >
            <LazyClipboardImage :item-id="item.id" source="favorite" class="thumbnail" alt="预览" />
            <span class="image-expand-hint">点击查看大图</span>
          </div>
          <div v-else class="history-preview" v-html="highlightText(shortPreview(item), query)" />
        </div>
        <el-empty
          v-if="!loading && items.length === 0"
          :description="query.trim() ? '没有匹配的收藏' : '暂无收藏，在剪贴板或快捷面板点亮 ☆ 即可收藏'"
        />
      </div>
    </section>
  </div>
</template>

<style scoped>
.fav-toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
}
</style>
