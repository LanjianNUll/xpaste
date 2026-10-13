<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { Close, Download, FullScreen, Minus, Plus, RefreshRight } from "@element-plus/icons-vue";
import { closeImageViewer, imageViewerName, imageViewerSrc, imageViewerVisible } from "@/services/imageViewer";

/**
 * 全局图片预览器：全屏遮罩 + 顶部工具栏，支持缩放、旋转、下载。
 *
 * 挂载在根组件模板上，状态由 services/imageViewer 统一持有，
 * 任意位置调用 openImageViewer(src, name) 即可打开，无需层层传递 props。
 * 交互参照常见看图工具：
 * - 滚轮缩放（以鼠标位置为锚点）
 * - 按住拖动平移
 * - 双击复位
 * - 键盘：+/-缩放、方向键平移、R 旋转、0 复位、Esc 关闭
 */

const visible = imageViewerVisible;
const imageSrc = imageViewerSrc;
const imageName = imageViewerName;

const scale = ref(1);
const rotation = ref(0);
/** 拖拽平移的实时偏移 */
const offsetX = ref(0);
const offsetY = ref(0);
const dragging = ref(false);

/** 缩放上下限，避免缩到看不清或放大到卡顿 */
const MIN_SCALE = 0.2;
const MAX_SCALE = 8;
const ZOOM_STEP = 0.2;

let dragStartX = 0;
let dragStartY = 0;
let dragOriginX = 0;
let dragOriginY = 0;

/** 缩放百分比：小于 100% 时显示到整数，大于时保留一位小数 */
const scaleLabel = computed(() => {
  const percent = scale.value * 100;
  return percent < 100 ? `${Math.round(percent)}%` : `${percent.toFixed(0)}%`;
});

/** 图片的变换样式：缩放 + 旋转 + 平移 */
const imageStyle = computed(() => ({
  transform: `translate(${offsetX.value}px, ${offsetY.value}px) scale(${scale.value}) rotate(${rotation.value}deg)`
}));

/** 每次打开都复位，避免上一次的操作残留到下一张图 */
watch(visible, (isOpen) => {
  if (!isOpen) {
    scale.value = 1;
    rotation.value = 0;
    offsetX.value = 0;
    offsetY.value = 0;
    return;
  }
  scale.value = 1;
  rotation.value = 0;
  offsetX.value = 0;
  offsetY.value = 0;
  window.addEventListener("keydown", handleKeydown);
});

const close = () => closeImageViewer();

function zoomIn() {
  scale.value = Math.min(MAX_SCALE, Number((scale.value + ZOOM_STEP).toFixed(2)));
}

function zoomOut() {
  scale.value = Math.max(MIN_SCALE, Number((scale.value - ZOOM_STEP).toFixed(2)));
}

function rotate() {
  rotation.value = (rotation.value + 90) % 360;
}

/** 复位缩放与位移，但保留旋转角度（旋转是用户的主动选择） */
function resetView() {
  scale.value = 1;
  offsetX.value = 0;
  offsetY.value = 0;
}

/** 下载原图：文件名去掉非法字符，避免 Windows 报错 */
function download() {
  if (!imageSrc.value) return;
  const link = document.createElement("a");
  const safeName = imageName.value.replace(/[\\/:*?"<>|]/g, "_");
  link.download = `${safeName}.png`;
  link.href = imageSrc.value;
  link.click();
}

/**
 * 滚轮缩放：以鼠标所在位置为锚点，让指针下的像素保持不动。
 * 换算方式：先算出指针相对图片中心的偏移，按缩放比例回缩，再补上放大带来的增量。
 */
function handleWheel(event: WheelEvent) {
  event.preventDefault();
  const direction = event.deltaY < 0 ? 1 : -1;
  const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale.value + direction * ZOOM_STEP));
  const container = event.currentTarget as HTMLElement;
  const rect = container.getBoundingClientRect();
  const pointerX = event.clientX - rect.left - rect.width / 2;
  const pointerY = event.clientY - rect.top - rect.height / 2;
  const ratio = next / scale.value;
  offsetX.value = pointerX - (pointerX - offsetX.value) * ratio;
  offsetY.value = pointerY - (pointerY - offsetY.value) * ratio;
  scale.value = Number(next.toFixed(2));
}

function startDrag(event: PointerEvent) {
  dragging.value = true;
  dragStartX = event.clientX;
  dragStartY = event.clientY;
  dragOriginX = offsetX.value;
  dragOriginY = offsetY.value;
  (event.target as HTMLElement).setPointerCapture?.(event.pointerId);
}

function moveDrag(event: PointerEvent) {
  if (!dragging.value) return;
  offsetX.value = dragOriginX + (event.clientX - dragStartX);
  offsetY.value = dragOriginY + (event.clientY - dragStartY);
}

function endDrag() {
  dragging.value = false;
}

function handleKeydown(event: KeyboardEvent) {
  const moveStep = 40;
  switch (event.key) {
    case "Escape":
      close();
      break;
    case "+":
    case "=":
      zoomIn();
      break;
    case "-":
      zoomOut();
      break;
    case "0":
      resetView();
      break;
    case "r":
    case "R":
      rotate();
      break;
    case "ArrowLeft":
      offsetX.value += moveStep;
      break;
    case "ArrowRight":
      offsetX.value -= moveStep;
      break;
    case "ArrowUp":
      offsetY.value += moveStep;
      break;
    case "ArrowDown":
      offsetY.value -= moveStep;
      break;
    default:
      break;
  }
}

// 打开时就监听键盘：预览是全屏浮层，用户往往直接按 Esc 关闭
onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <Transition name="viewer-fade">
    <div v-if="visible" class="image-viewer" @wheel="handleWheel">
      <!-- 遮罩：点击空白处关闭 -->
      <div class="viewer-mask" @click="close"></div>

      <!-- 顶部工具栏 -->
      <div class="viewer-toolbar">
        <span class="viewer-name" :title="imageName">{{ imageName }}</span>
        <div class="viewer-controls">
          <button type="button" class="viewer-btn" title="缩小 (-)" @click="zoomOut">
            <el-icon :size="16"><Minus /></el-icon>
          </button>
          <span class="viewer-scale">{{ scaleLabel }}</span>
          <button type="button" class="viewer-btn" title="放大 (+)" @click="zoomIn">
            <el-icon :size="16"><Plus /></el-icon>
          </button>
          <button type="button" class="viewer-btn" title="复位 (0)" @click="resetView">
            <el-icon :size="16"><FullScreen /></el-icon>
          </button>
          <button type="button" class="viewer-btn" title="旋转 90° (R)" @click="rotate">
            <el-icon :size="16"><RefreshRight /></el-icon>
          </button>
          <button type="button" class="viewer-btn" title="下载原图" @click="download">
            <el-icon :size="16"><Download /></el-icon>
          </button>
          <button type="button" class="viewer-btn viewer-btn-close" title="关闭 (Esc)" @click="close">
            <el-icon :size="16"><Close /></el-icon>
          </button>
        </div>
      </div>

      <!-- 图片本体：拖拽平移，双击复位 -->
      <div
        class="viewer-stage"
        :class="{ 'is-dragging': dragging }"
        @pointerdown="startDrag"
        @pointermove="moveDrag"
        @pointerup="endDrag"
        @pointercancel="endDrag"
        @dblclick="resetView"
      >
        <img :src="imageSrc" :alt="imageName" class="viewer-image" :style="imageStyle" draggable="false" />
      </div>

      <div class="viewer-hint">滚轮缩放 · 拖动平移 · 双击复位 · Esc 关闭</div>
    </div>
  </Transition>
</template>

<style scoped>
.image-viewer {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: flex;
  flex-direction: column;
}

/* 半透明黑遮罩：压暗背景让图片成为唯一焦点 */
.viewer-mask {
  position: absolute;
  inset: 0;
  background: rgba(12, 14, 18, 0.88);
  backdrop-filter: blur(2px);
}

.viewer-toolbar {
  position: relative;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  background: rgba(24, 26, 32, 0.92);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.viewer-name {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.82);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.viewer-controls {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.viewer-scale {
  min-width: 52px;
  text-align: center;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.82);
  font-variant-numeric: tabular-nums;
}

.viewer-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: rgba(255, 255, 255, 0.82);
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease;
}

.viewer-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #ffffff;
}

.viewer-btn-close:hover {
  background: #c42b1c;
  color: #ffffff;
}

/* 图片舞台：占据工具栏之外的全部空间 */
.viewer-stage {
  position: relative;
  z-index: 1;
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  cursor: grab;
  touch-action: none;
}

.viewer-stage.is-dragging {
  cursor: grabbing;
}

.viewer-image {
  max-width: 92%;
  max-height: 92%;
  object-fit: contain;
  user-select: none;
  will-change: transform;
}

.viewer-hint {
  position: relative;
  z-index: 2;
  padding: 8px 14px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
  text-align: center;
  background: rgba(24, 26, 32, 0.6);
}

/* 开合淡入淡出 */
.viewer-fade-enter-active,
.viewer-fade-leave-active {
  transition: opacity 0.16s ease;
}

.viewer-fade-enter-from,
.viewer-fade-leave-to {
  opacity: 0;
}
</style>