<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import {
  loadThemeState,
  resolveAmbientEffect,
  subscribeThemeApplied,
  type ThemeState
} from "@/services/theme";
import { createAmbientEngine, type AmbientEngine } from "@/services/ambient";

/**
 * 背景氛围层，由两部分组成：
 *
 * 1. .skin-backdrop —— 皮肤背景风景（services/scenery.ts 生成的渐变图层），
 *    固定层 + z-index:-1，压在 body 底色之上、所有面板之下；面板半透明时
 *    透出来。经典皮肤不注入变量，此层保持透明。
 * 2. .ambient-layer —— 色晕 + 粒子动效（Canvas 2D），悬浮于内容之上、
 *    弹窗之下，pointer-events 穿透不影响交互。
 *
 * 系统开启"减少动态效果"时只保留静态背景与色晕，不播放粒子。
 */
const canvasRef = ref<HTMLCanvasElement | null>(null);
const state = ref<ThemeState>(loadThemeState());
const resolvedDark = ref(state.value.mode === "dark");

let engine: AmbientEngine | null = null;
let unsubscribe: (() => void) | null = null;
let motionQuery: MediaQueryList | null = null;
let motionHandler: (() => void) | null = null;

function syncEngine(): void {
  if (!engine) return;
  engine.configure({
    effect: resolveAmbientEffect(state.value),
    dark: resolvedDark.value,
    density: state.value.effectDensity
  });
}

onMounted(() => {
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
  if (!reduced.matches) {
    if (canvasRef.value) {
      engine = createAmbientEngine(canvasRef.value);
    }
  }

  motionHandler = () => {
    if (motionQuery?.matches) {
      engine?.destroy();
      engine = null;
    } else if (!engine && canvasRef.value) {
      engine = createAmbientEngine(canvasRef.value);
      syncEngine();
    }
  };
  motionQuery = reduced;
  reduced.addEventListener("change", motionHandler);

  unsubscribe = subscribeThemeApplied((next, dark) => {
    state.value = next;
    resolvedDark.value = dark;
    syncEngine();
  });
  syncEngine();
});

onBeforeUnmount(() => {
  unsubscribe?.();
  motionQuery?.removeEventListener("change", motionHandler!);
  engine?.destroy();
  engine = null;
});
</script>

<template>
  <div class="skin-backdrop" aria-hidden="true"></div>
  <div class="ambient-layer" aria-hidden="true">
    <div class="ambient-glow"></div>
    <canvas ref="canvasRef" class="ambient-canvas"></canvas>
  </div>
</template>

<style scoped>
.ambient-layer {
  position: fixed;
  inset: 0;
  z-index: 1000;
  pointer-events: none;
}

.ambient-glow,
.ambient-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

/* 色晕强度随皮肤变化：有背景风景的皮肤会调低，避免与背景双层叠加过艳 */
.ambient-glow {
  background: var(--skin-glow, none);
  opacity: var(--skin-glow-opacity, 0.5);
  transition: opacity 0.3s ease;
}
</style>
