<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { Check, Moon, Monitor, Sunny } from "@element-plus/icons-vue";
import {
  AMBIENT_DENSITY_OPTIONS,
  AMBIENT_EFFECT_LABEL,
  AMBIENT_EFFECT_OPTIONS,
  BUILTIN_SKINS,
  loadThemeState,
  removeCustomSkin,
  saveCustomSkin,
  setEffectDensity,
  setEffectsEnabled,
  setSkin,
  setThemeMode
} from "@/services/theme";
import type {
  AmbientDensity,
  AmbientEffectId,
  CustomSkinConfig,
  ThemeMode,
  ThemeState
} from "@/services/theme";

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
const effectsEnabled = ref(true);
const effectDensity = ref<AmbientDensity>("medium");

const editorVisible = ref(false);
const draft = reactive({
  name: "",
  primary: "#e63946",
  tintLight: "",
  tintDark: "",
  effect: "none" as AmbientEffectId
});

const presetColors = [
  "#e63946", "#f4a261", "#ffd166", "#06d6a0", "#1fce6d", "#0891b2",
  "#0078d4", "#547bb0", "#7c5cd6", "#ec4899", "#6d6875", "#2b2d42"
];

const modeOptions: Array<{ value: ThemeMode; label: string; icon: unknown }> = [
  { value: "light", label: "浅色", icon: Sunny },
  { value: "dark", label: "深色", icon: Moon },
  { value: "system", label: "跟随系统", icon: Monitor }
];

function reloadState() {
  const state = loadThemeState();
  mode.value = state.mode;
  skinId.value = state.skin;
  savedCustomSkin.value = state.customSkin;
  effectsEnabled.value = state.effectsEnabled;
  effectDensity.value = state.effectDensity;
}

function applyState(state: ThemeState) {
  mode.value = state.mode;
  skinId.value = state.skin;
  savedCustomSkin.value = state.customSkin;
  effectsEnabled.value = state.effectsEnabled;
  effectDensity.value = state.effectDensity;
  emit("changed", state);
}

/** 皮肤卡片色样：主色 + 淡色延伸，直观预览换肤效果 */
function swatchStyle(primary: string) {
  return {
    background: `linear-gradient(120deg, ${primary} 0%, ${primary} 46%, color-mix(in srgb, ${primary} 16%, #ffffff) 100%)`
  };
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
    effect: draft.effect
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
</script>

<template>
  <div class="theme-settings">
    <section class="ts-section">
      <div class="ts-label">主题模式</div>
      <div class="mode-options">
        <button
          v-for="option in modeOptions"
          :key="option.value"
          type="button"
          class="mode-option"
          :class="{ 'is-active': mode === option.value }"
          @click="handleSelectMode(option.value)"
        >
          <el-icon :size="14"><component :is="option.icon" /></el-icon>
          <span>{{ option.label }}</span>
        </button>
      </div>
    </section>

    <section class="ts-section">
      <div class="ts-label">
        皮肤
        <span class="ts-hint">每套皮肤自带专属背景动效</span>
      </div>
      <div class="skin-grid">
        <button
          v-for="skin in BUILTIN_SKINS"
          :key="skin.id"
          type="button"
          class="skin-card"
          :class="{ 'is-active': skinId === skin.id }"
          :title="skin.description"
          @click="handleSelectSkin(skin.id)"
        >
          <span class="skin-swatch" :style="swatchStyle(skin.primary)">
            <el-icon v-if="skinId === skin.id" class="skin-check" :size="12">
              <Check />
            </el-icon>
          </span>
          <span class="skin-name">{{ skin.name }}</span>
          <span class="skin-effect">{{ AMBIENT_EFFECT_LABEL[skin.effect] }}</span>
        </button>

        <button
          v-if="customCardInfo && !editorVisible"
          type="button"
          class="skin-card"
          :class="{ 'is-active': skinId === 'custom' }"
          title="自定义皮肤"
          @click="handleSelectSkin('custom')"
        >
          <span class="skin-swatch" :style="swatchStyle(customCardInfo.primary)">
            <el-icon v-if="skinId === 'custom'" class="skin-check" :size="12">
              <Check />
            </el-icon>
          </span>
          <span class="skin-name">{{ customCardInfo.name }}</span>
          <span class="skin-effect">自定义</span>
        </button>

        <button
          v-if="!editorVisible"
          type="button"
          class="skin-card skin-card-create"
          title="创建自定义皮肤"
          @click="openEditor"
        >
          <span class="skin-swatch skin-swatch-create">＋</span>
          <span class="skin-name">自定义</span>
          <span class="skin-effect">打造专属</span>
        </button>
      </div>
    </section>

    <section class="ts-section">
      <div class="ts-label">背景动效</div>
      <div class="effect-row">
        <el-switch
          :model-value="effectsEnabled"
          @change="handleToggleEffects"
        />
        <span class="effect-state">{{ effectsEnabled ? "已开启" : "已关闭" }}</span>
        <div class="effect-density" :class="{ 'is-disabled': !effectsEnabled }">
          <button
            v-for="option in AMBIENT_DENSITY_OPTIONS"
            :key="option.value"
            type="button"
            class="density-segment"
            :disabled="!effectsEnabled"
            :class="{ 'is-active': effectDensity === option.value }"
            @click="handleDensityChange(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
      </div>
      <div class="ts-hint effect-hint">
        粒子随皮肤变化（柳絮 / 落叶 / 飞雪 / 星尘…）；系统开启"减少动态效果"时自动停用。
      </div>
    </section>

    <!-- 自定义皮肤编辑器（高级） -->
    <section v-if="editorVisible" class="ts-section custom-editor">
      <div class="custom-editor-header">
        <span class="ts-label">自定义皮肤</span>
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

      <div class="custom-editor-preview">
        <span class="skin-swatch preview-swatch" :style="swatchStyle(draft.primary)" />
        <span class="ts-hint">按钮、选中态与背景动效都会按以上配置自动生成配套颜色</span>
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
.theme-settings {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.ts-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ts-label {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

.ts-hint {
  font-size: 12px;
  font-weight: 400;
  color: var(--muted);
}

.mode-options {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.mode-option {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 32px;
  border: 1px solid var(--border);
  border-radius: 2px;
  background: var(--control-bg);
  color: var(--text);
  font-size: 13px;
  cursor: pointer;
  transition: border-color 0.12s ease, color 0.12s ease, box-shadow 0.12s ease;
}

.mode-option:hover {
  background: var(--control-bg-hover);
}

.mode-option.is-active {
  border-color: var(--accent);
  color: var(--accent);
  box-shadow: inset 0 0 0 1px var(--accent);
}

.skin-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}

.skin-card {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: 2px;
  background: var(--panel-alt);
  cursor: pointer;
  transition: border-color 0.12s ease, box-shadow 0.12s ease;
}

.skin-card:hover {
  border-color: var(--border-strong-hover);
}

.skin-card.is-active {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 1px var(--accent);
}

.skin-swatch {
  position: relative;
  height: 26px;
  border-radius: 2px;
  border: 1px solid var(--border);
  overflow: hidden;
}

.skin-check {
  position: absolute;
  right: 3px;
  top: 50%;
  transform: translateY(-50%);
  background: rgba(255, 255, 255, 0.92);
  border-radius: 50%;
  padding: 1px;
  color: #1b1b1b;
}

.skin-swatch-create {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--panel-subtle);
  color: var(--muted);
  font-size: 15px;
}

.skin-card-create {
  border-style: dashed;
}

.skin-name {
  font-size: 12px;
  color: var(--text);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.skin-effect {
  font-size: 10px;
  color: var(--muted);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.effect-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.effect-state {
  font-size: 12px;
  color: var(--muted);
  min-width: 44px;
}

.effect-density {
  display: inline-flex;
  gap: 0;
  margin-left: auto;
  border: 1px solid var(--border);
  border-radius: 2px;
  overflow: hidden;
}

.density-segment {
  padding: 4px 12px;
  border: none;
  background: var(--control-bg);
  color: var(--muted);
  font-size: 12px;
  cursor: pointer;
}

.density-segment + .density-segment {
  border-left: 1px solid var(--border);
}

.density-segment:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.density-segment.is-active {
  background: var(--accent);
  color: var(--accent-contrast);
}

.effect-hint {
  line-height: 1.5;
}

.custom-editor {
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 2px;
  background: var(--panel-alt);
}

.custom-editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.editor-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 14px;
}

.editor-field {
  display: flex;
  align-items: center;
  gap: 6px;
}

.editor-label {
  font-size: 12px;
  color: var(--muted);
  white-space: nowrap;
}

.editor-grid .el-input {
  width: 130px;
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
}

.preview-swatch {
  width: 110px;
  height: 20px;
  flex-shrink: 0;
}

.custom-editor-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ts-footer-hint {
  font-size: 12px;
  color: var(--muted);
}

@media (max-width: 560px) {
  .skin-grid {
    grid-template-columns: repeat(3, 1fr);
  }

  .effect-density {
    margin-left: 0;
  }

  .effect-row {
    flex-wrap: wrap;
  }
}
</style>
