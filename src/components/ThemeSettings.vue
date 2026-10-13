<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { Check } from "@element-plus/icons-vue";
import {
  AMBIENT_DENSITY_OPTIONS,
  AMBIENT_EFFECT_OPTIONS,
  BACKDROP_STYLE_OPTIONS,
  BUILTIN_SKINS,
  loadThemeState,
  removeCustomSkin,
  resolveTheme,
  saveCustomSkin,
  setBackdropStyle,
  setEffectDensity,
  setEffectsEnabled,
  setSkin,
  setThemeMode,
  subscribeThemeApplied
} from "@/services/theme";
import type {
  AmbientDensity,
  AmbientEffectId,
  BackdropStyle,
  CustomSkinConfig,
  ThemeMode,
  ThemeState
} from "@/services/theme";
import { buildSkinScenery } from "@/services/scenery";

/**
 * 外观设置：主题模式 + 皮肤 + 背景动效 + 自定义皮肤。
 * 全部即时生效并持久化，不需要点击"保存"。
 */
const props = defineProps<{
  /** 对话框是否展开，展开时重新读取存储，避免与快速切换按钮状态不同步 */
  active: boolean;
}>();

const emit = defineEmits<{
  (event: "changed", state: ThemeState): void;
}>();

const mode = ref<ThemeMode>("light");
const skinId = ref("classic");
const savedCustomSkin = ref<CustomSkinConfig | null>(null);
const backdropStyle = ref<BackdropStyle>("photo");
const effectsEnabled = ref(true);
const effectDensity = ref<AmbientDensity>("medium");
/** 当前生效的深浅色，决定皮肤卡片预览用哪套背景 */
const resolvedDark = ref(resolveTheme(loadThemeState().mode));

const editorVisible = ref(false);
const fileInputRef = ref<HTMLInputElement | null>(null);
const draft = reactive({
  name: "",
  primary: "#e63946",
  tintLight: "",
  tintDark: "",
  effect: "none" as AmbientEffectId,
  backgroundImage: ""
});

const presetColors = [
  "#e63946", "#f4a261", "#ffd166", "#06d6a0", "#1fce6d", "#0891b2",
  "#0078d4", "#547bb0", "#7c5cd6", "#ec4899", "#6d6875", "#2b2d42"
];

const modeOptions: Array<{ value: ThemeMode; label: string }> = [
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
  { value: "system", label: "跟随系统" }
];

function reloadState() {
  const state = loadThemeState();
  mode.value = state.mode;
  skinId.value = state.skin;
  savedCustomSkin.value = state.customSkin;
  backdropStyle.value = state.backdropStyle;
  effectsEnabled.value = state.effectsEnabled;
  effectDensity.value = state.effectDensity;
  resolvedDark.value = resolveTheme(state.mode);
}

function applyState(state: ThemeState) {
  mode.value = state.mode;
  skinId.value = state.skin;
  savedCustomSkin.value = state.customSkin;
  backdropStyle.value = state.backdropStyle;
  effectsEnabled.value = state.effectsEnabled;
  effectDensity.value = state.effectDensity;
  resolvedDark.value = resolveTheme(state.mode);
  emit("changed", state);
}

const HEX_COLOR = /^#[0-9a-fA-F]{6}$/;

/**
 * 浅色 / 深色两张卡片的缩略图用纯色块表示，而不是风景缩略图——
 * 这两张卡的本质是"无背景的主题底色"，画风景反而会误导。
 * 色块上再叠一层示意用的界面元素，让人一眼看出浅色是白底、深色是黑底。
 */
function modeSwatchStyle(value: ThemeMode) {
  if (value === "dark") {
    return { background: "linear-gradient(160deg, #2f2f2f, #171717)" };
  }
  if (value === "system") {
    // 跟随系统：左半浅右半深，视觉上直接表达"跟随"的意思
    return { background: "linear-gradient(105deg, #f6f6f6 0 48%, #2b2b2b 52% 100%)" };
  }
  return { background: "linear-gradient(160deg, #ffffff, #f0f0f0)" };
}

/**
 * 皮肤卡片色样：直接复用窗口背景的生成结果（services/scenery.ts），
 * 卡片里看到的风景就是选中后窗口里的风景；经典皮肤没有背景，用面板底色兜底。
 */
function sceneryStyle(skinIdValue: string, custom?: CustomSkinConfig | null) {
  const config = custom ?? savedCustomSkin.value;
  const usableConfig = config && (HEX_COLOR.test(config.primary ?? "") || config.backgroundImage);
  if (skinIdValue === "custom" && !usableConfig) {
    return { background: "var(--panel-subtle)" };
  }
  const scenery = buildSkinScenery(skinIdValue, resolvedDark.value, config, backdropStyle.value);
  if (!scenery) {
    // 经典皮肤没有背景风景，用一层中性渐变占位，卡片不至于看起来是空的
    return { background: "linear-gradient(160deg, var(--panel-subtle), var(--panel-alt))" };
  }
  return { backgroundColor: scenery.color, backgroundImage: scenery.image };
}

/** 编辑器里的草稿预览：改主色/背景调/背景图时立刻能看到效果 */
function draftSceneryStyle() {
  return sceneryStyle("custom", {
    name: draft.name || "自定义",
    primary: draft.primary,
    tintLight: HEX_COLOR.test(draft.tintLight) ? draft.tintLight : undefined,
    tintDark: HEX_COLOR.test(draft.tintDark) ? draft.tintDark : undefined,
    effect: draft.effect,
    backgroundImage: draft.backgroundImage || undefined
  });
}

/** 自定义皮肤的 localStorage 配额约 5MB，图片数据超过这个量级就存不下了 */
const BACKGROUND_IMAGE_SIZE_LIMIT = 2600000;

/**
 * 把用户选的图片压到最长边 1600px 并转成 JPEG。
 * 背景是铺满窗口的，原图动辄几 MB，直接存 localStorage 会爆配额。
 */
async function compressBackgroundImage(file: File): Promise<string | null> {
  const dataUrl = await new Promise<string | null>((resolve) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : null);
    reader.onerror = () => resolve(null);
    reader.readAsDataURL(file);
  });
  if (!dataUrl) return null;

  const image = await new Promise<HTMLImageElement | null>((resolve) => {
    const element = new Image();
    element.onload = () => resolve(element);
    element.onerror = () => resolve(null);
    element.src = dataUrl;
  });
  if (!image || !image.width || !image.height) return null;

  const scale = Math.min(1, 1600 / Math.max(image.width, image.height));
  const width = Math.max(1, Math.round(image.width * scale));
  const height = Math.max(1, Math.round(image.height * scale));
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return dataUrl;
  context.drawImage(image, 0, 0, width, height);
  return canvas.toDataURL("image/jpeg", 0.82);
}

async function handlePickBackgroundImage(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  // 立刻清空，保证再次选择同一个文件也能触发 change
  input.value = "";
  if (!file) return;
  if (!file.type.startsWith("image/")) {
    ElMessage.warning("请选择图片文件");
    return;
  }
  const dataUrl = await compressBackgroundImage(file);
  if (!dataUrl) {
    ElMessage.error("读取图片失败，请换一张试试");
    return;
  }
  draft.backgroundImage = dataUrl;
  if (dataUrl.length > BACKGROUND_IMAGE_SIZE_LIMIT) {
    ElMessage.warning("图片偏大，可能无法保存到本地，建议换一张更小的图");
  } else {
    ElMessage.success("背景图已就绪，点击“保存并应用”生效");
  }
}

function clearBackgroundImage() {
  draft.backgroundImage = "";
}

/** 宫格里的自定义皮肤卡片：编辑中显示草稿颜色，否则显示已保存的颜色 */
const customCardInfo = computed(() => {
  const source = editorVisible.value
    ? { name: draft.name, primary: draft.primary }
    : savedCustomSkin.value;
  if (!source) return null;
  return {
    name: source.name?.trim() || "自定义",
    primary: source.primary
  };
});

async function handleSelectMode(value: ThemeMode) {
  if (mode.value === value) return;
  applyState(await setThemeMode(value));
}

async function handleSelectSkin(value: string) {
  if (skinId.value === value) return;
  applyState(await setSkin(value));
}

async function handleSelectBackdropStyle(value: BackdropStyle) {
  if (backdropStyle.value === value) return;
  applyState(await setBackdropStyle(value));
}

async function handleToggleEffects(enabled: boolean | string | number) {
  applyState(await setEffectsEnabled(Boolean(enabled)));
}

async function handleDensityChange(value: AmbientDensity) {
  if (value === effectDensity.value) return;
  effectDensity.value = value;
  applyState(await setEffectDensity(value));
}

function openEditor() {
  const saved = savedCustomSkin.value;
  draft.name = saved?.name ?? "";
  draft.primary = saved?.primary ?? "#e63946";
  draft.tintLight = saved?.tintLight ?? "";
  draft.tintDark = saved?.tintDark ?? "";
  draft.effect = saved?.effect ?? "none";
  draft.backgroundImage = saved?.backgroundImage ?? "";
  editorVisible.value = true;
}

function closeEditor() {
  editorVisible.value = false;
}

async function handleSaveCustom() {
  if (!/^#[0-9a-fA-F]{6}$/.test(draft.primary)) {
    ElMessage.warning("请选择一个有效的主题色");
    return;
  }
  applyState(await saveCustomSkin({
    name: draft.name,
    primary: draft.primary,
    tintLight: draft.tintLight || undefined,
    tintDark: draft.tintDark || undefined,
    effect: draft.effect,
    backgroundImage: draft.backgroundImage || undefined
  }));
  editorVisible.value = false;
  ElMessage.success("自定义皮肤已应用");
}

async function handleRemoveCustom() {
  applyState(await removeCustomSkin());
  editorVisible.value = false;
  ElMessage.success("已删除自定义皮肤");
}

watch(
  () => props.active,
  (visible) => {
    if (visible) {
      reloadState();
      editorVisible.value = false;
    }
  }
);

// 跟随系统深浅色变化时，皮肤卡片的背景预览也要跟着换一套配色
let unsubscribeTheme: (() => void) | null = null;

onMounted(() => {
  unsubscribeTheme = subscribeThemeApplied((_state, dark) => {
    resolvedDark.value = dark;
  });
});

onBeforeUnmount(() => {
  unsubscribeTheme?.();
});
</script>

<template>
  <div class="theme-settings">
    <!-- 全部主题：深浅色模式与皮肤合在一个网格里，所见即所得 -->
    <section class="settings-block">
      <div class="settings-block-header">
        <span class="settings-block-title">全部主题</span>
        <span class="settings-block-hint">缩略图即为窗口实际背景，点击即时切换</span>
      </div>

      <div class="theme-gallery">
        <!-- 深浅色模式：纯色块示意，不画风景 -->
        <button
          v-for="option in modeOptions"
          :key="option.value"
          type="button"
          class="theme-card"
          :class="{ 'is-active': mode === option.value }"
          :title="option.label"
          @click="handleSelectMode(option.value)"
        >
          <span class="theme-thumb" :style="modeSwatchStyle(option.value)">
            <!-- 色块上叠一层示意界面，让浅色/深色的对比一眼可辨 -->
            <span
              class="theme-mock"
              :class="option.value === 'dark' ? 'is-dark' : option.value === 'system' ? 'is-auto' : 'is-light'"
            >
              <i class="mock-bar"></i>
              <i class="mock-line"></i>
              <i class="mock-line is-short"></i>
            </span>
            <span v-if="mode === option.value" class="theme-check">
              <el-icon :size="12"><Check /></el-icon>
            </span>
          </span>
          <span class="theme-name">{{ option.label }}</span>
        </button>

        <!-- 内置皮肤：缩略图复用窗口真实背景 -->
        <button
          v-for="skin in BUILTIN_SKINS"
          :key="skin.id"
          type="button"
          class="theme-card"
          :class="{ 'is-active': skinId === skin.id }"
          :title="skin.description"
          @click="handleSelectSkin(skin.id)"
        >
          <span class="theme-thumb" :style="sceneryStyle(skin.id)">
            <span v-if="skinId === skin.id" class="theme-check">
              <el-icon :size="12"><Check /></el-icon>
            </span>
          </span>
          <span class="theme-name">{{ skin.name }}</span>
        </button>

        <!-- 已保存的自定义皮肤 -->
        <button
          v-if="customCardInfo && !editorVisible"
          type="button"
          class="theme-card"
          :class="{ 'is-active': skinId === 'custom' }"
          title="自定义皮肤"
          @click="handleSelectSkin('custom')"
        >
          <span class="theme-thumb" :style="sceneryStyle('custom')">
            <span v-if="skinId === 'custom'" class="theme-check">
              <el-icon :size="12"><Check /></el-icon>
            </span>
          </span>
          <span class="theme-name">{{ customCardInfo.name }}</span>
        </button>

        <!-- 新建自定义皮肤 -->
        <button
          v-if="!editorVisible"
          type="button"
          class="theme-card theme-card-create"
          title="创建自定义皮肤"
          @click="openEditor"
        >
          <span class="theme-thumb theme-thumb-create">＋</span>
          <span class="theme-name">自定义</span>
        </button>
      </div>
    </section>

    <!-- 动效与画法：行式设置，控件右对齐 -->
    <div class="settings-group">
      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">背景画法</div>
          <div class="settings-row-desc">
            {{ backdropStyle === "photo"
              ? "实景照片（来源见 src/assets/skins/CREDITS.md）"
              : "纯 CSS 渐变风景，分辨率无关、不占体积" }}
          </div>
        </div>
        <div class="settings-row-control">
          <div class="settings-segmented">
            <button
              v-for="option in BACKDROP_STYLE_OPTIONS"
              :key="option.value"
              type="button"
              class="settings-segment"
              :class="{ 'is-active': backdropStyle === option.value }"
              @click="handleSelectBackdropStyle(option.value)"
            >
              {{ option.label }}
            </button>
          </div>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">背景动效</div>
          <div class="settings-row-desc">
            粒子随皮肤变化（柳絮 / 落叶 / 飞雪 / 星尘…）；系统开启"减少动态效果"时自动停用
          </div>
        </div>
        <div class="settings-row-control">
          <el-switch :model-value="effectsEnabled" @change="handleToggleEffects" />
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">动效密度</div>
        </div>
        <div class="settings-row-control">
          <div class="settings-segmented" :class="{ 'is-disabled': !effectsEnabled }">
            <button
              v-for="option in AMBIENT_DENSITY_OPTIONS"
              :key="option.value"
              type="button"
              class="settings-segment"
              :disabled="!effectsEnabled"
              :class="{ 'is-active': effectDensity === option.value }"
              @click="handleDensityChange(option.value)"
            >
              {{ option.label }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 自定义皮肤编辑器（高级）：展开时以嵌入面板呈现 -->
    <section v-if="editorVisible" class="custom-editor">
      <div class="custom-editor-header">
        <span class="custom-editor-title">自定义皮肤</span>
        <button type="button" class="text-btn" @click="closeEditor">收起</button>
      </div>

      <div class="editor-grid">
        <div class="editor-field">
          <span class="editor-label">名称</span>
          <el-input
            v-model="draft.name"
            size="small"
            placeholder="我的专属配色"
            maxlength="12"
          />
        </div>
        <div class="editor-field">
          <span class="editor-label">主题色</span>
          <el-color-picker v-model="draft.primary" :predefine="presetColors" size="small" />
        </div>
        <div class="editor-field">
          <span class="editor-label">浅色背景调</span>
          <el-color-picker v-model="draft.tintLight" :predefine="presetColors" size="small" />
        </div>
        <div class="editor-field">
          <span class="editor-label">深色背景调</span>
          <el-color-picker v-model="draft.tintDark" :predefine="presetColors" size="small" />
        </div>
        <div class="editor-field">
          <span class="editor-label">背景动效</span>
          <el-select v-model="draft.effect" size="small" style="width: 120px">
            <el-option
              v-for="option in AMBIENT_EFFECT_OPTIONS"
              :key="option.value"
              :label="option.label"
              :value="option.value"
            />
          </el-select>
        </div>
      </div>

      <div class="editor-field editor-field-wide">
        <span class="editor-label">背景图</span>
        <input
          ref="fileInputRef"
          type="file"
          accept="image/*"
          class="hidden-file-input"
          @change="handlePickBackgroundImage"
        />
        <el-button size="small" @click="fileInputRef?.click()">
          {{ draft.backgroundImage ? "更换图片" : "选择图片" }}
        </el-button>
        <el-button v-if="draft.backgroundImage" size="small" text @click="clearBackgroundImage">
          清除
        </el-button>
        <span class="ts-hint">可选：用自己的图片当背景（自动压缩后仅存本地）</span>
      </div>

      <div class="custom-editor-preview">
        <span class="editor-preview-thumb" :style="draftSceneryStyle()"></span>
        <span class="ts-hint">
          {{ draft.backgroundImage
            ? "背景图会铺满窗口，并自动叠加一层薄雾保证文字清晰"
            : "按钮、背景风景与背景动效都会按以上配置自动生成配套颜色" }}
        </span>
      </div>

      <div class="custom-editor-actions">
        <el-button size="small" type="primary" @click="handleSaveCustom">保存并应用</el-button>
        <el-button v-if="savedCustomSkin" size="small" type="danger" plain @click="handleRemoveCustom">
          删除
        </el-button>
        <el-button size="small" text @click="closeEditor">取消</el-button>
      </div>
    </section>

    <div class="ts-footer-hint">外观改动即时生效并自动保存，所有窗口同步切换。</div>
  </div>
</template>

<style scoped>
/*
 * 外观设置内部样式。
 * 行式布局的公共样式（.settings-group / .settings-row / .settings-segmented
 * 等）由 app.css 提供，这里只负责外观页特有的控件：皮肤宫格与颜色编辑器。
 */
.theme-settings {
  display: flex;
  flex-direction: column;
  gap: 22px;
}

.ts-hint {
  font-size: 12px;
  font-weight: 400;
  color: var(--muted);
}

/*
 * 全部主题网格：宽幅横向缩略图（16:9），名称在图下方居中。
 * 用 auto-fill + minmax 而不是固定列数：设置面板宽度随窗口变化，
 * 让卡片始终保持接近 16:9 的比例，不会出现"一行挤 5 张、下一行只放 2 张"的空档。
 */
.theme-gallery {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(128px, 1fr));
  gap: 14px 12px;
}

.theme-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 7px;
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.theme-thumb {
  position: relative;
  width: 100%;
  aspect-ratio: 16 / 9;
  border: 1px solid var(--stroke);
  border-radius: var(--radius-md);
  overflow: hidden;
  /* 缩略图用的是与窗口背景同一套图层，按 cover 铺满 */
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  transition: border-color 0.14s ease, box-shadow 0.14s ease, transform 0.14s ease;
}

.theme-card:hover .theme-thumb {
  transform: translateY(-2px);
  box-shadow: var(--shadow-card);
}

.theme-card.is-active .theme-thumb {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--selection-border);
}

/* 选中角标：贴在缩略图右上角，用主色实心圆点，一眼可辨当前选中项 */
.theme-check {
  position: absolute;
  top: 6px;
  right: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--accent);
  color: var(--accent-contrast);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.28);
}

.theme-name {
  max-width: 100%;
  font-size: 12px;
  color: var(--text);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.theme-card.is-active .theme-name {
  color: var(--accent);
  font-weight: 600;
}

/*
 * 浅色 / 深色 / 跟随系统三张卡片：色块上叠一层示意界面元素
 * （顶栏 + 两行文字），比纯色块更能让用户预判切换后的真实观感。
 */
.theme-mock {
  position: absolute;
  left: 8px;
  right: 8px;
  top: 8px;
  bottom: 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 6px;
  border-radius: var(--radius-xs);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
}

.theme-mock.is-light {
  background: rgba(255, 255, 255, 0.96);
}

.theme-mock.is-dark {
  background: rgba(43, 43, 43, 0.96);
}

.mock-bar {
  height: 5px;
  border-radius: 3px;
  background: rgba(120, 120, 128, 0.45);
}

.mock-line {
  height: 4px;
  border-radius: 2px;
  background: rgba(120, 120, 128, 0.28);
}

.mock-line.is-short {
  width: 62%;
}

/* 跟随系统：色块本身已是一半浅一半深，无需再叠示意界面 */
.theme-mock.is-auto {
  display: none;
}

.theme-thumb-create {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--card-inset);
  border-style: dashed;
  color: var(--muted);
  font-size: 20px;
}

.theme-card-create:hover .theme-thumb {
  border-color: var(--stroke-strong);
  color: var(--text);
}

.custom-editor {
  border: 1px solid var(--selection-border);
  background: var(--card-inset);
  border-radius: var(--radius-md);
  padding: 14px 16px;
}

.custom-editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.custom-editor-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}

.editor-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 16px;
  margin-bottom: 12px;
}

.editor-field {
  display: flex;
  align-items: center;
  gap: 8px;
}

.editor-label {
  font-size: 12px;
  color: var(--muted);
  white-space: nowrap;
}

.editor-grid .el-input {
  width: 130px;
}

/* 背景图选择：原生 file input 只作为触发器，界面上只显示按钮 */
.editor-field-wide {
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.hidden-file-input {
  display: none;
}

.text-btn {
  border: none;
  background: transparent;
  color: var(--accent);
  font-size: 12px;
  cursor: pointer;
  padding: 0;
}

.text-btn:hover {
  color: var(--accent-hover);
  text-decoration: underline;
}

.custom-editor-preview {
  display: flex;
  align-items: center;
  gap: 10px;
  padding-top: 12px;
  border-top: 1px solid var(--stroke);
  margin-bottom: 12px;
}

/* 编辑器里的实时预览缩略图：与网格卡片同样的取景方式，所见即所得 */
.editor-preview-thumb {
  width: 120px;
  aspect-ratio: 16 / 9;
  flex-shrink: 0;
  border: 1px solid var(--stroke);
  border-radius: var(--radius-sm);
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
}

.custom-editor-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ts-footer-hint {
  font-size: 12px;
  color: var(--muted);
  text-align: center;
}
</style>
