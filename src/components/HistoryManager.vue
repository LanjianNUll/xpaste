<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { ArrowDown, Search } from "@element-plus/icons-vue";
import zhCn from "element-plus/dist/locale/zh-cn.mjs";
import type { ClipboardItem, DateRangeType, DateRange } from "@/types";
import {
  fetchHistoryByDate,
  clearHistory,
  deleteHistoryItem,
  deleteHistoryByFormat,
  deleteHistoryByCategory,
  getFormatStats,
  getCategoryStats
} from "@/services/api";

const filters = ref({
  dateRange: "today" as DateRangeType,
  customDateRange: [new Date(), new Date()] as [Date, Date],
  formats: [] as string[],
  categories: [] as string[],
  keyword: ""
});

const items = ref<ClipboardItem[]>([]);
const selectedItems = ref<ClipboardItem[]>([]);
const loading = ref(false);
const pagination = ref({ currentPage: 1, pageSize: 20, total: 0 });
const formatStats = ref<Array<[string, number]>>([]);
const categoryStats = ref<Array<[string, number]>>([]);

const debounceHandle = ref<number | null>(null);

// 图片预览
const imagePreviewVisible = ref(false);
const imagePreviewUrl = ref("");

const formatLabel: Record<string, string> = {
  text: "文本",
  image: "图片",
  html: "HTML",
  file: "文件",
  color: "颜色",
  link: "链接"
};

const categoryLabel: Record<string, string> = {
  link: "链接",
  image: "图片",
  text: "文本",
  file: "文件"
};

function getDateRange(type: DateRangeType): DateRange | null {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  
  switch(type) {
    case "all":
      return null; // 返回null表示不限制时间范围
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
    case "customRange":
      if (filters.value.customDateRange && filters.value.customDateRange.length === 2) {
        const startDate = new Date(filters.value.customDateRange[0].getFullYear(), filters.value.customDateRange[0].getMonth(), filters.value.customDateRange[0].getDate());
        const endDate = new Date(filters.value.customDateRange[1].getFullYear(), filters.value.customDateRange[1].getMonth(), filters.value.customDateRange[1].getDate());
        return {
          startTs: startDate.getTime(),
          endTs: endDate.getTime() + 86400000 - 1
        };
      }
      return {
        startTs: today.getTime(),
        endTs: today.getTime() + 86400000 - 1
      };
  }
}

const paginatedItems = computed(() => {
  const start = (pagination.value.currentPage - 1) * pagination.value.pageSize;
  const end = start + pagination.value.pageSize;
  return items.value.slice(start, end);
});

async function loadHistory() {
  loading.value = true;
  try {
    const range = getDateRange(filters.value.dateRange);
    let data: ClipboardItem[];
    
    // 如果是"全部时间"，不传时间范围参数
    if (range === null) {
      data = await fetchHistoryByDate(0, Date.now() + 86400000, filters.value.keyword);
    } else {
      data = await fetchHistoryByDate(range.startTs, range.endTs, filters.value.keyword);
    }
    
    // 前端按类型过滤
    if (filters.value.formats.length > 0) {
      data = data.filter(item => {
        // 如果筛选包含"link"，需要同时检查 format 和 category
        if (filters.value.formats.includes('link')) {
          return item.category === 'link' || filters.value.formats.includes(item.format);
        }
        return filters.value.formats.includes(item.format);
      });
    }
    
    // 前端按分类过滤
    if (filters.value.categories.length > 0) {
      data = data.filter(item => filters.value.categories.includes(item.category));
    }
    
    items.value = data;
    pagination.value.total = data.length;
    pagination.value.currentPage = 1;
    
    // 加载统计信息
    await loadFormatStats();
    await loadCategoryStats();
  } catch (err) {
    ElMessage.error("加载历史记录失败");
  } finally {
    loading.value = false;
  }
}

async function loadFormatStats() {
  try {
    formatStats.value = await getFormatStats();
  } catch (err) {
    console.error("加载格式统计信息失败", err);
  }
}

async function loadCategoryStats() {
  try {
    categoryStats.value = await getCategoryStats();
  } catch (err) {
    console.error("加载分类统计信息失败", err);
  }
}

function debounceLoad() {
  if (debounceHandle.value != null) {
    window.clearTimeout(debounceHandle.value);
  }
  debounceHandle.value = window.setTimeout(() => {
    loadHistory();
  }, 300);
}

function handleSelectionChange(selection: ClipboardItem[]) {
  selectedItems.value = selection;
}

function handlePageChange() {
  // 分页变化时自动滚动到顶部
  window.scrollTo({ top: 0, behavior: 'smooth' });
}

function formatTime(ts: number) {
  const date = new Date(ts);
  return `${date.getMonth() + 1}/${date.getDate()} ${date.getHours().toString().padStart(2, "0")}:${date
    .getMinutes()
    .toString()
    .padStart(2, "0")}`;
}

function shortPreview(item: ClipboardItem) {
  if (item.format === "image") {
    return "[图片]";
  }
  if (item.format === "file") {
    const path = item.filePath ?? "[文件]";
    const fileName = path.split(/[\\/]/).pop();
    return fileName ?? path;
  }
  if (item.format === "color") {
    return item.color ?? item.text ?? "[颜色]";
  }
  const text = item.text ?? item.html ?? "";
  return text.length > 100 ? text.substring(0, 100) + "..." : text;
}

function imageSrc(item: ClipboardItem) {
  if (!item.imageBase64) return "";
  return `data:image/png;base64,${item.imageBase64}`;
}

function openImagePreview(item: ClipboardItem) {
  const src = imageSrc(item);
  if (src) {
    imagePreviewUrl.value = src;
    imagePreviewVisible.value = true;
  }
}

async function openLink(item: ClipboardItem) {
  const url = item.text ?? item.html ?? "";
  if (!url) {
    ElMessage.warning("无效的链接");
    return;
  }
  
  try {
    // 使用 Tauri API 在默认浏览器中打开链接
    const { open } = await import("@tauri-apps/plugin-shell");
    await open(url);
  } catch (err) {
    console.error("打开链接失败:", err);
    ElMessage.error("打开链接失败");
  }
}

async function handleDeleteItem(item: ClipboardItem) {
  try {
    await ElMessageBox.confirm(
      `确定要删除这条${formatLabel[item.format]}记录吗？`,
      "确认删除",
      {
        type: "warning",
        confirmButtonText: "删除",
        cancelButtonText: "取消"
      }
    );
    
    await deleteHistoryItem(item.id);
    ElMessage.success("已删除");
    await loadHistory();
  } catch (err: any) {
    if (err !== "cancel") {
      ElMessage.error("删除失败");
    }
  }
}

async function handleBatchDelete() {
  if (selectedItems.value.length === 0) {
    ElMessage.warning("请先选择要删除的记录");
    return;
  }
  
  try {
    await ElMessageBox.confirm(
      `确定要删除选中的 ${selectedItems.value.length} 条记录吗？`,
      "确认批量删除",
      {
        type: "warning",
        confirmButtonText: "删除",
        cancelButtonText: "取消"
      }
    );
    
    for (const item of selectedItems.value) {
      await deleteHistoryItem(item.id);
    }
    
    ElMessage.success(`已删除 ${selectedItems.value.length} 条记录`);
    selectedItems.value = [];
    await loadHistory();
  } catch (err: any) {
    if (err !== "cancel") {
      ElMessage.error("批量删除失败");
    }
  }
}

async function handleDeleteByFormat(format: string) {
  const label = formatLabel[format];
  
  try {
    await ElMessageBox.confirm(
      `确定要删除所有${label}类型的记录吗？此操作不可恢复！`,
      "确认删除",
      {
        type: "warning",
        confirmButtonText: "删除",
        cancelButtonText: "取消"
      }
    );
    
    const count = await deleteHistoryByFormat(format);
    ElMessage.success(`已删除 ${count} 条${label}记录`);
    await loadHistory();
  } catch (err: any) {
    if (err !== "cancel") {
      ElMessage.error("删除失败");
    }
  }
}

async function handleDeleteByCategory(category: string) {
  const label = categoryLabel[category] || formatLabel[category] || category;
  
  try {
    await ElMessageBox.confirm(
      `确定要删除所有${label}类型的记录吗？此操作不可恢复！`,
      "确认删除",
      {
        type: "warning",
        confirmButtonText: "删除",
        cancelButtonText: "取消"
      }
    );
    
    const count = await deleteHistoryByCategory(category);
    ElMessage.success(`已删除 ${count} 条${label}记录`);
    await loadHistory();
  } catch (err: any) {
    if (err !== "cancel") {
      ElMessage.error("删除失败");
    }
  }
}

async function handleClearAll() {
  try {
    await ElMessageBox.confirm(
      "确定要清空所有历史记录吗？此操作不可恢复！",
      "确认清空",
      {
        type: "error",
        confirmButtonText: "清空",
        cancelButtonText: "取消"
      }
    );
    
    await clearHistory();
    ElMessage.success("已清空所有历史记录");
    await loadHistory();
  } catch (err: any) {
    if (err !== "cancel") {
      ElMessage.error("清空失败");
    }
  }
}

onMounted(() => {
  loadHistory();
});

watch(() => filters.value.customDateRange, () => {
  if (filters.value.dateRange === "customRange") {
    loadHistory();
  }
}, { deep: true });
</script>

<template>
  <div class="history-manager">
    <!-- 筛选区域 -->
    <el-card class="filter-section" shadow="never">
      <div class="filter-container">
        <!-- 左侧筛选条件 -->
        <el-space wrap class="filter-space">
          <!-- 时间范围选择 -->
          <el-select 
            v-model="filters.dateRange" 
            placeholder="时间范围"
            @change="loadHistory"
            style="width: 130px"
          >
            <el-option label="全部" value="all" />
            <el-option label="今天" value="today" />
            <el-option label="昨天" value="yesterday" />
            <el-option label="前天" value="beforeYesterday" />
            <el-option label="自定义范围" value="customRange" />
          </el-select>
          
          <!-- 自定义时间范围选择器 -->
          <el-date-picker
            v-if="filters.dateRange === 'customRange'"
            v-model="filters.customDateRange"
            type="daterange"
            range-separator="至"
            start-placeholder="开始日期"
            end-placeholder="结束日期"
            @change="loadHistory"
            style="width: 260px"
          />
          
          <!-- 分类筛选 -->
          <el-select
            v-model="filters.categories"
            multiple
            placeholder="分类筛选"
            @change="loadHistory"
            style="width: 150px"
            collapse-tags
            collapse-tags-tooltip
          >
            <el-option label="链接" value="link" />
            <el-option label="文本" value="text" />
            <el-option label="图片" value="image" />
            <el-option label="文件" value="file" />
          </el-select>
          
          <!-- 关键字搜索 -->
          <el-input
            v-model="filters.keyword"
            placeholder="搜索内容..."
            clearable
            @input="debounceLoad"
            :prefix-icon="Search"
            style="width: 200px"
          />
        </el-space>

        <!-- 右侧批量操作按钮 -->
        <div class="batch-operations">
          <el-space wrap>
            <el-button 
              type="danger" 
              @click="handleBatchDelete" 
              :disabled="selectedItems.length === 0"
              size="small"
            >
              批量删除 ({{ selectedItems.length }})
            </el-button>
            
            <el-dropdown @command="handleDeleteByCategory">
              <el-button type="warning" size="small">
                按类型删除
                <el-icon class="el-icon--right"><ArrowDown /></el-icon>
              </el-button>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item command="link">删除所有链接</el-dropdown-item>
                  <el-dropdown-item command="text">删除所有文本</el-dropdown-item>
                  <el-dropdown-item command="image">删除所有图片</el-dropdown-item>
                  <el-dropdown-item command="file">删除所有文件</el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
            
            <el-button type="danger" plain size="small" @click="handleClearAll">清空全部</el-button>
          </el-space>
        </div>
      </div>
    </el-card>

    <!-- 统计信息 -->
    <div class="stats-bar">
      <span class="stats-total">共 {{ pagination.total }} 条记录</span>
      <el-tag 
        v-for="stat in categoryStats" 
        :key="stat[0]" 
        size="small"
        :type="stat[0] === 'link' ? 'success' : 'info'"
      >
        {{ categoryLabel[stat[0]] || formatLabel[stat[0]] || stat[0] }}: {{ stat[1] }}
      </el-tag>
    </div>

    <!-- 记录列表 -->
    <div class="table-container">
      <el-table
        :data="paginatedItems"
        @selection-change="handleSelectionChange"
        v-loading="loading"
        stripe
        style="width: 100%"
        empty-text="暂无记录"
      >
      <el-table-column type="selection" width="55" />
      
      <el-table-column label="时间" width="120">
        <template #default="{ row }">
          {{ formatTime(row.createdAt) }}
        </template>
      </el-table-column>
      
      <el-table-column label="类型" width="100">
        <template #default="{ row }">
          <el-tag 
            size="small" 
            :type="row.category === 'link' ? 'success' : 'primary'"
          >
            {{ row.category === 'link' ? '链接' : formatLabel[row.format] }}
          </el-tag>
        </template>
      </el-table-column>
      
      <el-table-column label="内容预览" min-width="400">
        <template #default="{ row }">
          <div v-if="row.format === 'image'" class="preview-image" @click="openImagePreview(row)">
            <img :src="imageSrc(row)" class="thumbnail" alt="预览" />
            <span class="image-label">[图片] 点击放大</span>
          </div>
          <div v-else-if="row.category === 'link'" class="preview-link" @click="openLink(row)">
            <span class="link-icon">🔗</span>
            <span class="link-text">{{ shortPreview(row) }}</span>
          </div>
          <div v-else-if="row.format === 'color'" class="preview-color">
            <div 
              class="color-block" 
              :style="{ background: row.color || row.text || '#fff' }"
            />
            <span>{{ row.color || row.text }}</span>
          </div>
          <div v-else class="preview-text">
            {{ shortPreview(row) }}
          </div>
        </template>
      </el-table-column>
      
      <el-table-column label="操作" width="100" fixed="right">
        <template #default="{ row }">
          <el-button 
            type="danger" 
            size="small" 
            @click="handleDeleteItem(row)"
            link
          >
            删除
          </el-button>
        </template>
      </el-table-column>
    </el-table>
    </div>

    <!-- 分页 -->
    <el-config-provider :locale="zhCn">
      <el-pagination
        v-model:current-page="pagination.currentPage"
        v-model:page-size="pagination.pageSize"
        :page-sizes="[10, 20, 50, 100]"
        :total="pagination.total"
        layout="total, sizes, prev, pager, next, jumper"
        @size-change="loadHistory"
        @current-change="handlePageChange"
        class="pagination"
      />
    </el-config-provider>

    <!-- 图片预览对话框 -->
    <el-dialog
      v-model="imagePreviewVisible"
      title="图片预览"
      width="80%"
      top="5vh"
      class="image-preview-dialog"
    >
      <div class="image-preview-container">
        <img :src="imagePreviewUrl" alt="预览" class="preview-full-image" />
      </div>
    </el-dialog>
  </div>
</template>

<style scoped>
.history-manager {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
  overflow: hidden;
}

.filter-section {
  margin-bottom: 8px;
  flex-shrink: 0;
}

.filter-container {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}

.filter-space {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: center;
  flex: 1;
}

.batch-operations {
  flex-shrink: 0;
}

.stats-bar {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 8px 0;
  font-size: 14px;
  color: var(--el-text-color-regular);
  flex-wrap: wrap;
  flex-shrink: 0;
}

.table-container {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.stats-total {
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.preview-image {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  transition: opacity 0.2s;
}

.preview-image:hover {
  opacity: 0.8;
}

.preview-link {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  color: var(--el-color-success);
  transition: all 0.2s;
}

.preview-link:hover {
  opacity: 0.8;
  text-decoration: underline;
}

.link-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.link-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}

.thumbnail {
  max-width: 60px;
  max-height: 60px;
  object-fit: cover;
  border-radius: 4px;
  border: 1px solid var(--el-border-color);
}

.image-label {
  color: var(--el-text-color-regular);
  font-size: 12px;
}

.preview-color {
  display: flex;
  align-items: center;
  gap: 8px;
}

.color-block {
  width: 24px;
  height: 24px;
  border-radius: 4px;
  border: 1px solid var(--el-border-color);
  flex-shrink: 0;
}

.preview-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
  line-height: 1.5;
}

.pagination {
  justify-content: flex-end;
  margin-top: 16px;
  flex-shrink: 0;
}

:deep(.el-table) {
  --el-table-border-color: var(--el-border-color-lighter);
}

:deep(.el-table__row) {
  cursor: default;
}

:deep(.el-card__body) {
  padding: 16px;
}

.image-preview-container {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 400px;
  background: #f5f5f5;
  border-radius: 8px;
}

.preview-full-image {
  max-width: 100%;
  max-height: 80vh;
  object-fit: contain;
  border-radius: 4px;
}

:deep(.image-preview-dialog) {
  .el-dialog__body {
    padding: 20px;
  }
}
</style>
