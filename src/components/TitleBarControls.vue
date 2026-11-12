<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";

/**
 * 无边框窗口（decorations:false）的自绘窗口控制按钮，仅用于 Windows：
 * 右上角「最小化 / 最大化 / 关闭」，关闭键悬停红底（Win11 习惯）。
 *
 * macOS 不再自绘：主窗口改用系统原生标题栏（titleBarStyle: Overlay），
 * 红绿灯由窗口服务器绘制，位置、悬停符号、全屏行为都是系统标准做法，
 * 比自绘更贴近原生，也不会出现位置偏差。
 *
 * 关闭统一为隐藏窗口（应用常驻托盘），与 Rust 侧 CloseRequested 的拦截
 * 行为保持一致，真正退出走托盘菜单。浏览器里预览页面没有 Tauri 运行时，
 * 按钮静默失效，不影响其它功能。
 */
const isMaximized = ref(false);
let unlistenResize: (() => void) | null = null;

async function currentWindow() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  } catch {
    return null;
  }
}

async function minimizeWindow() {
  const win = await currentWindow();
  await win?.minimize().catch(() => undefined);
}

async function toggleMaximizeWindow() {
  const win = await currentWindow();
  if (!win) return;
  await win.toggleMaximize().catch(() => undefined);
  isMaximized.value = await win.isMaximized().catch(() => false);
}

async function hideMainWindow() {
  const win = await currentWindow();
  await win?.hide().catch(() => undefined);
}

onMounted(async () => {
  const win = await currentWindow();
  if (!win) return;
  isMaximized.value = await win.isMaximized().catch(() => false);
  unlistenResize = await win
    .onResized(() => {
      void win
        .isMaximized()
        .then((value) => {
          isMaximized.value = value;
        })
        .catch(() => undefined);
    })
    .catch(() => null);
});

onBeforeUnmount(() => {
  unlistenResize?.();
});
</script>

<template>
  <div class="win-controls">
    <button type="button" class="win-btn" title="最小化" @click="minimizeWindow">
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" /></svg>
    </button>
    <button
      type="button"
      class="win-btn"
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
    <button type="button" class="win-btn win-btn-close" title="隐藏到托盘" @click="hideMainWindow">
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" /></svg>
    </button>
  </div>
</template>

<style scoped>
.win-controls {
  display: flex;
  align-items: stretch;
  align-self: stretch;
  flex: 0 0 auto;
  margin-left: 8px;
}

.win-btn {
  width: 46px;
  padding: 0;
  border: none;
  border-radius: 0;
  background: transparent;
  color: var(--text-soft);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease;
}

.win-btn svg {
  width: 10px;
  height: 10px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1;
}

.win-btn:hover {
  background: color-mix(in srgb, var(--text) 10%, transparent);
  color: var(--text);
}

.win-btn:focus-visible {
  outline: 1px solid var(--accent);
  outline-offset: -1px;
}

.win-btn:active {
  background: color-mix(in srgb, var(--text) 16%, transparent);
}

/* 关闭键沿用 Windows 的红色悬停反馈 */
.win-btn-close:hover {
  background: #c42b1c;
  color: #ffffff;
}

.win-btn-close:active {
  background: #b02719;
  color: #ffffff;
}
</style>
