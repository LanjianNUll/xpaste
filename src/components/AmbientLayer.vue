<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  loadThemeState,
  resolveAmbientEffect,
  subscribeThemeApplied,
  type ThemeState
} from "@/services/theme";
import { createAmbientEngine, type AmbientEngine } from "@/services/ambient";

/**
 * 背景氛围层：皮肤色晕（CSS 渐变）+ 粒子动效（Canvas 2D）。
 * 悬浮于应用内容之上、弹窗之下，pointer-events 穿透不影响交互；
 * 系统开启"减少动态效果"时只保留静态色晕，不播放粒子。
 */
const canvasRef = ref<HTMLCanvasElement | null>(null);
const state = ref<ThemeState>(loadThemeState());
const resolvedDark = ref(state.value.mode === "dark");

let engine: AmbientEngine | null = null;
let unsubscribe: (() => void) | null = null;
let motionQuery: MediaQueryList | null = null;
let motionHandler: (() => void) | null = null;

const glowStyle = computed(() => ({
  opacity: resolvedDark.value ? "0.32" : "0.5"
}));

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
  <div class="ambient-layer" aria-hidden="true">
    <div class="ambient-glow" :style="glowStyle"></div>
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

.ambient-glow {
  background: var(--skin-glow, none);
  transition: opacity 0.3s ease;
}
</style>
