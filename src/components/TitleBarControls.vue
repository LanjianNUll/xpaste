<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import { isMacOS } from "@/services/platform";

/**
 * 无边框窗口（decorations:false）的自绘窗口控制按钮，按宿主系统分别渲染：
 * - Windows：右上角「最小化 / 最大化 / 关闭」，关闭键悬停红底（Win11 习惯）；
 * - macOS：左上角红黄绿「红绿灯」，悬停整组时显出符号（macOS 习惯）。
 *
 * 关闭统一为隐藏窗口（应用常驻托盘），与 Rust 侧 CloseRequested 的拦截
 * 行为保持一致，真正退出走托盘菜单。浏览器里预览页面没有 Tauri 运行时，
 * 按钮静默失效，不影响其它功能。
 */
const isMaximized = ref(false);
const focused = ref(true);
let unlistenResize: (() => void) | null = null;
let unlistenFocus: (() => void) | null = null;

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
  unlistenFocus = await win
    .onFocusChanged(({ payload }) => {
      focused.value = payload;
    })
    .catch(() => null);
});

onBeforeUnmount(() => {
  unlistenResize?.();
  unlistenFocus?.();
});
</script>

<template>
  <!-- macOS：左上角红绿灯 -->
  <div v-if="isMacOS" class="tl-group" :class="{ 'is-inactive': !focused }">
    <button type="button" class="tl tl-close" title="隐藏窗口" @click="hideMainWindow">
      <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M1.2 1.2l5.6 5.6M6.8 1.2L1.2 6.8" /></svg>
    </button>
    <button type="button" class="tl tl-min" title="最小化" @click="minimizeWindow">
      <svg viewBox="0 0 8 8" aria-hidden="true"><path d="M1.2 4h5.6" /></svg>
    </button>
    <button
      type="button"
      class="tl tl-zoom"
      :title="isMaximized ? '还原' : '最大化'"
      @click="toggleMaximizeWindow"
    >
      <svg viewBox="0 0 8 8" aria-hidden="true">
        <path class="tl-glyph-fill" d="M4 1h3v3z" />
        <path class="tl-glyph-fill" d="M4 7H1V4z" />
      </svg>
    </button>
  </div>

  <!-- Windows：右上角标题栏按钮 -->
  <div v-else class="win-controls">
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
/* ===== macOS 红绿灯 ===== */

.tl-group {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}

.tl {
  width: 12px;
  height: 12px;
  padding: 0;
  border: 0.5px solid rgba(0, 0, 0, 0.18);
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: filter 0.12s ease, background-color 0.12s ease;
}

.tl:hover {
  filter: brightness(0.92);
}

.tl:focus-visible {
  outline: 1px solid var(--accent);
  outline-offset: 1px;
}

.tl svg {
  width: 8px;
  height: 8px;
  fill: none;
  stroke: rgba(0, 0, 0, 0.55);
  stroke-width: 1.1;
  opacity: 0;
  transition: opacity 0.12s ease;
}

.tl svg .tl-glyph-fill {
  fill: rgba(0, 0, 0, 0.55);
  stroke: none;
}

/* 悬停整组才显出符号，与 macOS 原生行为一致 */
.tl-group:hover .tl svg {
  opacity: 1;
}

.tl-close {
  background: #ff5f57;
}

.tl-min {
  background: #febc2e;
}

.tl-zoom {
  background: #28c840;
}

/* 窗口失焦时整组变灰，符号不再出现 */
.tl-group.is-inactive .tl {
  background: var(--tl-inactive);
  border-color: var(--tl-inactive-border);
}

.tl-group.is-inactive .tl svg {
  opacity: 0;
}

/* ===== Windows 标题栏按钮 ===== */

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
