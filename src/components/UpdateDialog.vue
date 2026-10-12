<script setup lang="ts">
import { computed } from "vue";
import { ElMessage } from "element-plus";
import { Download, Refresh, RefreshRight } from "@element-plus/icons-vue";
import {
  hideUpdateDialog,
  installUpdate,
  startDownload,
  updateDialogVisible,
  updateState
} from "@/services/update";

/**
 * 更新提示：发现新版本时弹出，展示版本号与更新日志，
 * 并按"有新版本就自动下载"的策略在后台下载，下载完由用户决定何时重启安装。
 */
const phaseText = computed(() => {
  switch (updateState.phase) {
    case "available":
      return "准备下载…";
    case "downloading":
      return `正在下载 ${updateState.percent}%`;
    case "ready":
      return "下载完成，重启后即可用上新版本";
    case "installing":
      return "正在启动安装程序…";
    case "error":
      return updateState.error || "更新失败";
    default:
      return "";
  }
});

const isBusy = computed(
  () => updateState.phase === "available" || updateState.phase === "downloading" || updateState.phase === "installing"
);

const canInstall = computed(() => updateState.phase === "ready");

async function handleInstall() {
  if (!canInstall.value) return;
  ElMessage.info("正在启动安装程序，应用会自动重启…");
  await installUpdate();
}

async function handleRetryDownload() {
  await startDownload();
}
</script>

<template>
  <el-dialog
    :model-value="updateDialogVisible"
    title="发现新版本"
    width="min(520px, 92vw)"
    :close-on-click-modal="false"
    :close-on-press-escape="false"
    class="update-dialog"
    @close="hideUpdateDialog"
  >
    <div class="update-head">
      <span class="update-badge">v{{ updateState.version }}</span>
      <span class="update-current">当前版本 v{{ updateState.currentVersion }}</span>
    </div>

    <div class="update-logs-label">更新日志</div>
    <pre class="update-logs">{{ updateState.notes.trim() || "该版本没有提供更新说明。" }}</pre>

    <div class="update-progress">
      <el-progress
        v-if="isBusy || canInstall"
        :percentage="updateState.percent"
        :status="updateState.phase === 'error' ? 'exception' : canInstall ? 'success' : undefined"
        :stroke-width="8"
      />
      <div class="update-progress-text" :class="{ 'is-error': updateState.phase === 'error' }">
        {{ phaseText }}
      </div>
    </div>

    <template #footer>
      <el-button v-if="updateState.phase === 'error'" @click="handleRetryDownload">
        <el-icon><RefreshRight /></el-icon>
        <span>重试下载</span>
      </el-button>
      <el-button @click="hideUpdateDialog">稍后再说</el-button>
      <el-button type="primary" :disabled="!canInstall" @click="handleInstall">
        <el-icon>
          <Download v-if="!canInstall" />
          <Refresh v-else />
        </el-icon>
        <span>{{ canInstall ? "立即重启安装" : "下载中…" }}</span>
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.update-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 10px;
}

.update-badge {
  font-size: 16px;
  font-weight: 600;
  color: var(--accent);
}

.update-current {
  font-size: 12px;
  color: var(--muted);
}

.update-logs-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  margin-bottom: 6px;
}

/* 更新日志是 GitHub 自动生成的 Markdown，这里按原样展示（保留换行即可） */
.update-logs {
  margin: 0;
  padding: 10px 12px;
  max-height: 240px;
  overflow: auto;
  border: 1px solid var(--stroke);
  border-radius: var(--radius-md);
  background: var(--panel-alt);
  color: var(--text-soft);
  font-family: inherit;
  font-size: 13px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.update-progress {
  margin-top: 12px;
}

.update-progress-text {
  margin-top: 6px;
  font-size: 12px;
  color: var(--muted);
}

.update-progress-text.is-error {
  color: #c42b1c;
}
</style>
