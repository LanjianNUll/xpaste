/**
 * 主题与皮肤管理。
 *
 * - 模式（mode）：light / dark / system（跟随系统）。
 * - 皮肤（skin）：内置皮肤 id 或 "custom"（自定义），每个皮肤自带主色、
 *   背景风景（services/scenery.ts 生成，注入 --skin-backdrop-* 变量）、
 *   背景色晕（styles/theme.css 中按 data-skin 定义）与背景粒子动效。
 * - 动效（ambient）：全局开关 effectEnabled + 密度 density；粒子渲染由
 *   services/ambient.ts 引擎负责，AmbientLayer.vue 挂载。
 * - 状态持久化在 localStorage（两个窗口同源共享），变更时通过
 *   Tauri 事件 theme://changed 通知其它窗口即时刷新。
 * - 主题令牌在 styles/theme.css 中由 data-theme / data-skin 驱动，
 *   自定义皮肤与皮肤背景通过 documentElement 上的内联 CSS 变量注入。
 */

import { buildSkinScenery } from "./scenery";

export type ThemeMode = "light" | "dark" | "system";

/** 背景粒子动效 id（由 ambient.ts 引擎渲染） */
export type AmbientEffectId =
  | "none"
  | "snow"
  | "petals"
  | "catkins"
  | "leaves"
  | "fireflies"
  | "stardust"
  | "spark"
  | "matrix";

/** 背景动效密度 */
export type AmbientDensity = "low" | "medium" | "high";

/** 皮肤背景的两种画法：实景照片 / 纯 CSS 渐变风景 */
export type BackdropStyle = "photo" | "gradient";

export const BACKDROP_STYLE_OPTIONS: Array<{ value: BackdropStyle; label: string }> = [
  { value: "photo", label: "实景照片" },
  { value: "gradient", label: "简约渐变" }
];

export const AMBIENT_EFFECT_OPTIONS: Array<{ value: AmbientEffectId; label: string }> = [
  { value: "none", label: "无（纯净）" },
  { value: "snow", label: "下雪" },
  { value: "petals", label: "飘落花瓣" },
  { value: "catkins", label: "柳絮飞舞" },
  { value: "leaves", label: "落叶纷飞" },
  { value: "fireflies", label: "萤火虫" },
  { value: "stardust", label: "星尘闪烁" },
  { value: "spark", label: "光点上升" },
  { value: "matrix", label: "代码雨" }
];

export const AMBIENT_EFFECT_LABEL: Record<AmbientEffectId, string> = Object.fromEntries(
  AMBIENT_EFFECT_OPTIONS.map((option) => [option.value, option.label])
) as Record<AmbientEffectId, string>;

export const AMBIENT_DENSITY_OPTIONS: Array<{ value: AmbientDensity; label: string }> = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" }
];

export interface SkinInfo {
  id: string;
  name: string;
  description: string;
  /** 皮肤主色，用于设置界面里的皮肤卡片预览 */
  primary: string;
  /** 该皮肤内置的背景粒子动效 */
  effect: AmbientEffectId;
}

export interface CustomSkinConfig {
  name: string;
  primary: string;
  /** 浅色模式下的背景色调（可选） */
  tintLight?: string;
  /** 深色模式下的背景色调（可选） */
  tintDark?: string;
  /** 自定义皮肤使用的背景动效 */
  effect?: AmbientEffectId;
  /**
   * 自定义背景图（压缩后的 data URL，可选）。
   * 设置好后会替代自动生成的丘陵风景，铺满窗口并盖一层薄雾。
   */
  backgroundImage?: string;
}

export interface ThemeState {
  mode: ThemeMode;
  skin: string;
  customSkin: CustomSkinConfig | null;
  /** 皮肤背景画法：实景照片 / 渐变风景 */
  backdropStyle: BackdropStyle;
  /** 背景动效总开关 */
  effectsEnabled: boolean;
  /** 背景动效密度 */
  effectDensity: AmbientDensity;
}

export const THEME_STORAGE_KEY = "xpaste.theme";
export const CUSTOM_SKIN_STORAGE_KEY = "xpaste.customSkin";
export const THEME_CHANGED_EVENT = "theme://changed";

export const BUILTIN_SKINS: SkinInfo[] = [
  {
    id: "classic",
    name: "经典",
    description: "Windows 风格的经典蓝，纯净无动效",
    primary: "#0078d4",
    effect: "none"
  },
  {
    id: "spring",
    name: "春日",
    description: "草木嫩绿，柳絮飞舞",
    primary: "#4caf6d",
    effect: "catkins"
  },
  {
    id: "summer",
    name: "盛夏",
    description: "碧海清凉，夏夜萤火",
    primary: "#0891b2",
    effect: "fireflies"
  },
  {
    id: "autumn",
    name: "秋叶",
    description: "暖阳琥珀，落叶纷飞",
    primary: "#d97706",
    effect: "leaves"
  },
  {
    id: "winter",
    name: "冬雪",
    description: "霜天雾蓝，静雪飘落",
    primary: "#547bb0",
    effect: "snow"
  },
  {
    id: "anime",
    name: "樱花",
    description: "二次元樱花粉，花瓣飘落",
    primary: "#ec4899",
    effect: "petals"
  },
  {
    id: "geek",
    name: "极客",
    description: "终端荧光绿，代码雨，深色更搭",
    primary: "#1fce6d",
    effect: "matrix"
  },
  {
    id: "lavender",
    name: "紫罗兰",
    description: "静谧优雅的紫，星尘闪烁",
    primary: "#7c5cd6",
    effect: "stardust"
  },
  {
    id: "chinese-red",
    name: "中国红",
    description: "正红鎏金，祥云流金",
    primary: "#c8102e",
    effect: "stardust"
  },
  {
    id: "chinese-ink",
    name: "中国风",
    description: "宣纸留白，水墨远山",
    primary: "#55636f",
    effect: "none"
  },
  {
    id: "tech",
    name: "科技",
    description: "数字网格，霓虹光带",
    primary: "#2f7bff",
    effect: "spark"
  }
];

export const DEFAULT_THEME_STATE: ThemeState = {
  mode: "light",
  skin: "classic",
  customSkin: null,
  backdropStyle: "photo",
  effectsEnabled: true,
  effectDensity: "medium"
};

export function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

export function isBackdropStyle(value: unknown): value is BackdropStyle {
  return value === "photo" || value === "gradient";
}

export function isAmbientEffect(value: unknown): value is AmbientEffectId {
  return typeof value === "string" && AMBIENT_EFFECT_OPTIONS.some((option) => option.value === value);
}

function normalizeDensity(value: unknown): AmbientDensity {
  return value === "low" || value === "high" || value === "medium" ? value : "medium";
}

export function loadCustomSkin(): CustomSkinConfig | null {
  try {
    const raw = window.localStorage.getItem(CUSTOM_SKIN_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<CustomSkinConfig>;
    if (typeof parsed.primary !== "string" || !/^#[0-9a-fA-F]{6}$/.test(parsed.primary)) {
      return null;
    }
    return {
      name: typeof parsed.name === "string" && parsed.name.trim() ? parsed.name.trim() : "自定义",
      primary: parsed.primary,
      tintLight: normalizeTint(parsed.tintLight),
      tintDark: normalizeTint(parsed.tintDark),
      effect: isAmbientEffect(parsed.effect) ? parsed.effect : "none",
      backgroundImage: normalizeBackgroundImage(parsed.backgroundImage)
    };
  } catch {
    return null;
  }
}

function normalizeTint(value: unknown): string | undefined {
  return typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value) ? value : undefined;
}

/** 只接受图片类型的 data URL，避免把任意字符串塞进 CSS 的 url()。 */
function normalizeBackgroundImage(value: unknown): string | undefined {
  return typeof value === "string" && /^data:image\/(png|jpeg|webp|gif);base64,/.test(value)
    ? value
    : undefined;
}

function isValidSkin(skin: string, customSkin: CustomSkinConfig | null): boolean {
  if (skin === "custom") {
    return customSkin != null;
  }
  return BUILTIN_SKINS.some((item) => item.id === skin);
}

/** 读取持久化的主题状态；数据损坏时回退默认值。 */
export function loadThemeState(): ThemeState {
  try {
    const raw = window.localStorage.getItem(THEME_STORAGE_KEY);
    const customSkin = loadCustomSkin();
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<ThemeState>;
      let skin = typeof parsed.skin === "string" && parsed.skin ? parsed.skin : DEFAULT_THEME_STATE.skin;
      if (!isValidSkin(skin, customSkin)) {
        skin = DEFAULT_THEME_STATE.skin;
      }
      return {
        mode: isThemeMode(parsed.mode) ? parsed.mode : DEFAULT_THEME_STATE.mode,
        skin,
        customSkin,
        backdropStyle: isBackdropStyle(parsed.backdropStyle) ? parsed.backdropStyle : DEFAULT_THEME_STATE.backdropStyle,
        effectsEnabled: parsed.effectsEnabled !== false,
        effectDensity: normalizeDensity(parsed.effectDensity)
      };
    }
    return { ...DEFAULT_THEME_STATE, customSkin };
  } catch {
    // 忽略损坏数据，使用默认值
    return { ...DEFAULT_THEME_STATE, customSkin: loadCustomSkin() };
  }
}

/** 把 mode=system 解析成实际生效的 light / dark。 */
export function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return mode;
}

/** 解析当前状态下应播放的背景动效（总开关关闭时返回 none）。 */
export function resolveAmbientEffect(state: ThemeState): AmbientEffectId {
  if (!state.effectsEnabled) return "none";
  if (state.skin === "custom") {
    return state.customSkin?.effect ?? "none";
  }
  return BUILTIN_SKINS.find((item) => item.id === state.skin)?.effect ?? "none";
}

/** 按 WCAG 相对亮度计算主色上应使用的文字颜色（过亮的主色用深色文字）。 */
function accentContrastFor(primary: string): string {
  const hex = primary.replace("#", "");
  const rgb = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const linear = rgb.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  const luminance = 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
  return luminance > 0.45 ? "#1b1b1b" : "#ffffff";
}

const CUSTOM_SKIN_VARS = [
  "--skin-primary",
  "--skin-primary-hover",
  "--skin-bg-tint-light",
  "--skin-bg-tint-dark"
] as const;

/** 皮肤背景相关的内联变量（见 services/scenery.ts） */
const SCENERY_VARS = [
  "--skin-backdrop-image",
  "--skin-backdrop-color",
  "--skin-surface-alpha",
  "--skin-glow-opacity"
] as const;

type ThemeAppliedListener = (state: ThemeState, resolvedDark: boolean) => void;

const appliedListeners = new Set<ThemeAppliedListener>();

/** 订阅主题应用事件（本窗口每次 DOM 主题变化都会回调）。 */
export function subscribeThemeApplied(listener: ThemeAppliedListener): () => void {
  appliedListeners.add(listener);
  return () => appliedListeners.delete(listener);
}

/** 把主题状态落到 DOM（data 属性、dark 类、自定义皮肤内联变量）。 */
export function applyThemeState(state: ThemeState): boolean {
  const root = document.documentElement;
  const resolved = resolveTheme(state.mode);
  // 自定义皮肤配置缺失时退回经典皮肤，避免主色变量失去定义
  const skin = state.skin === "custom" && !state.customSkin ? "classic" : state.skin;

  root.dataset.theme = resolved;
  root.dataset.skin = skin;
  root.classList.toggle("dark", resolved === "dark");

  // 自定义皮肤：清理旧变量后重新注入
  for (const name of CUSTOM_SKIN_VARS) {
    root.style.removeProperty(name);
  }
  root.style.removeProperty("--accent-contrast");

  if (skin === "custom" && state.customSkin) {
    const { primary, tintLight, tintDark } = state.customSkin;
    root.style.setProperty("--skin-primary", primary);
    root.style.setProperty("--skin-primary-hover", shade(primary, -0.12));
    if (tintLight) root.style.setProperty("--skin-bg-tint-light", tintLight);
    if (tintDark) root.style.setProperty("--skin-bg-tint-dark", tintDark);
    root.style.setProperty("--accent-contrast", accentContrastFor(primary));
  }

  // 皮肤背景：经典皮肤返回 null，此时清空变量回到纯净底色
  for (const name of SCENERY_VARS) {
    root.style.removeProperty(name);
  }
  const scenery = buildSkinScenery(skin, resolved === "dark", state.customSkin, state.backdropStyle);
  if (scenery) {
    root.style.setProperty("--skin-backdrop-image", scenery.image);
    root.style.setProperty("--skin-backdrop-color", scenery.color);
    root.style.setProperty("--skin-surface-alpha", `${scenery.surfaceAlpha}%`);
    root.style.setProperty("--skin-glow-opacity", String(scenery.glowOpacity));
  }

  for (const listener of appliedListeners) {
    try {
      listener(state, resolved === "dark");
    } catch {
      // 单个监听器异常不影响其它消费者
    }
  }
  return resolved === "dark";
}

/** 简单的明度微调：amount 为正提亮、为负压暗。 */
function shade(hex: string, amount: number): string {
  const normalized = hex.replace("#", "");
  const rgb = [0, 2, 4].map((i) => {
    const channel = parseInt(normalized.slice(i, i + 2), 16);
    const adjusted = amount >= 0
      ? channel + (255 - channel) * amount
      : channel * (1 + amount);
    return Math.round(Math.min(255, Math.max(0, adjusted)));
  });
  return `#${rgb.map((c) => c.toString(16).padStart(2, "0")).join("")}`;
}

function persistThemeState(state: ThemeState): void {
  try {
    window.localStorage.setItem(
      THEME_STORAGE_KEY,
      JSON.stringify({
        mode: state.mode,
        skin: state.skin,
        backdropStyle: state.backdropStyle,
        effectsEnabled: state.effectsEnabled,
        effectDensity: state.effectDensity
      })
    );
  } catch {
    // localStorage 不可用时静默降级为仅本次会话生效
  }
}

async function syncNativeWindowTheme(state: ThemeState): Promise<void> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setTheme(resolveTheme(state.mode));
  } catch {
    // 权限缺失或平台不支持时忽略，Web 内容主题不受影响
  }
}

async function broadcastThemeChange(state: ThemeState): Promise<void> {
  try {
    const { emit } = await import("@tauri-apps/api/event");
    await emit(THEME_CHANGED_EVENT, {
      mode: state.mode,
      skin: state.skin,
      backdropStyle: state.backdropStyle,
      effectsEnabled: state.effectsEnabled,
      effectDensity: state.effectDensity
    });
  } catch {
    // 事件权限缺失时仅本窗口生效（localStorage 里的状态仍在，重开窗口后一致）
  }
}

async function commitThemeState(state: ThemeState): Promise<void> {
  applyThemeState(state);
  persistThemeState(state);
  await Promise.all([broadcastThemeChange(state), syncNativeWindowTheme(state)]);
}

export async function setThemeMode(mode: ThemeMode): Promise<ThemeState> {
  const state = loadThemeState();
  state.mode = mode;
  await commitThemeState(state);
  return state;
}

export async function setSkin(skinId: string): Promise<ThemeState> {
  const state = loadThemeState();
  state.skin = skinId;
  await commitThemeState(state);
  return state;
}

export async function setBackdropStyle(style: BackdropStyle): Promise<ThemeState> {
  const state = loadThemeState();
  state.backdropStyle = style;
  await commitThemeState(state);
  return state;
}

export async function setEffectsEnabled(enabled: boolean): Promise<ThemeState> {
  const state = loadThemeState();
  state.effectsEnabled = enabled;
  await commitThemeState(state);
  return state;
}

export async function setEffectDensity(density: AmbientDensity): Promise<ThemeState> {
  const state = loadThemeState();
  state.effectDensity = density;
  await commitThemeState(state);
  return state;
}

/** 保存自定义皮肤配置；保存即同步持久化并广播。 */
export async function saveCustomSkin(config: CustomSkinConfig): Promise<ThemeState> {
  const normalized: CustomSkinConfig = {
    name: config.name.trim() || "自定义",
    primary: config.primary,
    tintLight: normalizeTint(config.tintLight),
    tintDark: normalizeTint(config.tintDark),
    effect: isAmbientEffect(config.effect) ? config.effect : "none",
    backgroundImage: normalizeBackgroundImage(config.backgroundImage)
  };
  try {
    window.localStorage.setItem(CUSTOM_SKIN_STORAGE_KEY, JSON.stringify(normalized));
  } catch {
    // 持久化失败时仍可本次会话内使用
  }
  const state = loadThemeState();
  state.customSkin = normalized;
  if (state.skin !== "custom") {
    state.skin = "custom";
  }
  await commitThemeState(state);
  return state;
}

export async function removeCustomSkin(): Promise<ThemeState> {
  try {
    window.localStorage.removeItem(CUSTOM_SKIN_STORAGE_KEY);
  } catch {
    // 忽略
  }
  const state = loadThemeState();
  state.customSkin = null;
  if (state.skin === "custom") {
    state.skin = "classic";
    await commitThemeState(state);
  }
  return state;
}

/**
 * 初始化当前窗口的主题：
 * 1. 立即应用持久化状态（入口 html 中的内联脚本已提前应用过一次，这里兜底）；
 * 2. 监听系统深浅色变化（mode=system 时实时跟随）；
 * 3. 监听其它窗口广播的主题变更。
 */
export async function initTheme(): Promise<void> {
  const state = loadThemeState();
  applyThemeState(state);
  await syncNativeWindowTheme(state);

  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    const current = loadThemeState();
    if (current.mode === "system") {
      applyThemeState(current);
      void syncNativeWindowTheme(current);
    }
  });

  try {
    const { listen } = await import("@tauri-apps/api/event");
    await listen<{
      mode: ThemeMode;
      skin: string;
      backdropStyle?: BackdropStyle;
      effectsEnabled?: boolean;
      effectDensity?: AmbientDensity;
    }>(THEME_CHANGED_EVENT, (event) => {
      const payload = event.payload;
      const next: ThemeState = {
        mode: isThemeMode(payload?.mode) ? payload.mode : "light",
        skin: typeof payload?.skin === "string" ? payload.skin : "classic",
        customSkin: loadCustomSkin(),
        backdropStyle: isBackdropStyle(payload?.backdropStyle)
          ? payload.backdropStyle
          : DEFAULT_THEME_STATE.backdropStyle,
        effectsEnabled: payload?.effectsEnabled !== false,
        effectDensity: normalizeDensity(payload?.effectDensity)
      };
      applyThemeState(next);
      void syncNativeWindowTheme(next);
    });
  } catch {
    // 事件权限缺失时退化为各窗口独立（共享 localStorage，重开后仍一致）
  }
}
