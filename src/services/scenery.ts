/**
 * 皮肤背景（scenery）生成器。
 *
 * 每套皮肤的背景有两种画法，由主题状态里的 backdropStyle 决定：
 *
 * 1. "photo"（默认）：实景照片。图片来源见 assets/skins/CREDITS.md，
 *    均为 Pixabay 素材（Pixabay 内容许可，可商用、无需署名），
 *    已经统一压成 1920x1080 的 JPEG 随应用一起打包。
 * 2. "gradient"：纯 CSS 渐变图层拼出的风景（天空、日月、云层、丘陵、
 *    地面色带、扫描线…），不依赖图片资源，任何分辨率都不糊。
 *
 * 两种画法都只生成一段 background-image 字符串，最终注入 --skin-backdrop-*
 * 变量，由 app.css 的 .skin-backdrop 固定层铺满窗口（在所有内容之下）。
 * 生成结果同时用于 ThemeSettings 的皮肤卡片预览，保证"卡片里看到的"
 * 和"窗口里的"一致。
 *
 * 图层顺序遵循 CSS 约定：数组里靠前的图层画在上面。
 */

import type { BackdropStyle, CustomSkinConfig } from "./theme";

import springPhoto from "@/assets/skins/spring.jpg";
import summerPhoto from "@/assets/skins/summer.jpg";
import autumnPhoto from "@/assets/skins/autumn.jpg";
import winterPhoto from "@/assets/skins/winter.jpg";
import animePhoto from "@/assets/skins/anime.jpg";
import geekPhoto from "@/assets/skins/geek.jpg";
import lavenderPhoto from "@/assets/skins/lavender.jpg";
import chineseRedPhoto from "@/assets/skins/chinese-red.jpg";
import chineseInkPhoto from "@/assets/skins/chinese-ink.jpg";
import techPhoto from "@/assets/skins/tech.jpg";

export interface SceneryResult {
  /** 完整的 background-image 图层串 */
  image: string;
  /** 天空基底色（background-color，兼作渐变加载前的兜底） */
  color: string;
  /** 面板透出背景的不透明度（%），数值越小背景越明显 */
  surfaceAlpha: number;
  /** 背景上方的色晕强度，避免与背景双层叠加导致过艳 */
  glowOpacity: number;
}

interface Hill {
  x: number;
  y: number;
  rx: number;
  ry: number;
  color: string;
  /** true 时用柔和渐隐，false 用硬边剪影（更靠前的山丘） */
  soft?: boolean;
}

interface Cloud {
  x: number;
  y: number;
  rx: number;
  ry: number;
  alpha: number;
  /** 云的颜色（默认白色，中国红皮肤用金色祥云） */
  color?: string;
}

interface Luminary {
  x: number;
  y: number;
  rx: number;
  ry: number;
  /** 日/月本体颜色 */
  core: string;
  /** 外层光晕颜色 */
  halo: string;
}

interface ScenerySpec {
  /** 天空渐变：从上往下 2~3 个色标 */
  sky: string[];
  /** 日/月与光晕（极客皮肤没有） */
  luminary?: Luminary;
  clouds?: Cloud[];
  /** 丘陵，从近到远排列 */
  hills?: Hill[];
  /** 地平线以下的水面/地面色带；sheen 是叠在色带上的高光（如水面反光） */
  ground?: { height: number; color: string; sheen?: string };
  /** 撒在丘陵线上的树丛/灌木（用固定随机数生成，保证每次一致） */
  shrubs?: { count: number; color: string; size: number };
  /** 夜空星点 */
  stars?: { count: number; color: string; alpha: number };
  /** 追加的图层（极客皮肤的扫描线、科技的网格等） */
  extras?: string[];
  /** 面板透出背景的程度（%），不填用默认值；颜色越艳的皮肤调得越实，保证文字清晰 */
  surfaceAlpha?: number;
}

const SURFACE_ALPHA = { light: 68, dark: 70 } as const;
const GLOW_OPACITY = { light: 0.2, dark: 0.16 } as const;

/* ========== 内置皮肤的照片背景 ==========
 * veil：照片上盖的雾（0~1，浅色用白雾、深色用暗雾），越大照片越淡；
 * alpha：面板不透明度（%），画面越花调得越实，保证面板上的文字清晰。
 */
interface PhotoBackground {
  src: string;
  veilLight: number;
  veilDark: number;
  alphaLight: number;
  alphaDark: number;
}

const PHOTOS: Record<string, PhotoBackground> = {
  spring: { src: springPhoto, veilLight: 0.22, veilDark: 0.62, alphaLight: 80, alphaDark: 85 },
  summer: { src: summerPhoto, veilLight: 0.24, veilDark: 0.58, alphaLight: 80, alphaDark: 84 },
  autumn: { src: autumnPhoto, veilLight: 0.22, veilDark: 0.6, alphaLight: 80, alphaDark: 85 },
  winter: { src: winterPhoto, veilLight: 0.26, veilDark: 0.58, alphaLight: 78, alphaDark: 83 },
  anime: { src: animePhoto, veilLight: 0.26, veilDark: 0.6, alphaLight: 78, alphaDark: 83 },
  // 代码屏细节太碎，已经预模糊成柔和的绿色光斑，面板不用压太实
  geek: { src: geekPhoto, veilLight: 0.32, veilDark: 0.6, alphaLight: 88, alphaDark: 86 },
  lavender: { src: lavenderPhoto, veilLight: 0.22, veilDark: 0.52, alphaLight: 78, alphaDark: 83 },
  "chinese-red": { src: chineseRedPhoto, veilLight: 0.24, veilDark: 0.56, alphaLight: 78, alphaDark: 83 },
  "chinese-ink": { src: chineseInkPhoto, veilLight: 0.3, veilDark: 0.56, alphaLight: 78, alphaDark: 83 },
  tech: { src: techPhoto, veilLight: 0.28, veilDark: 0.54, alphaLight: 78, alphaDark: 83 }
};

function photoScenery(photo: PhotoBackground, dark: boolean): SceneryResult {
  const veil = dark ? `8, 10, 14, ${photo.veilDark}` : `255, 255, 255, ${photo.veilLight}`;
  return {
    image: `linear-gradient(rgba(${veil}), rgba(${veil})), url("${photo.src}")`,
    color: dark ? "#101318" : "#f2f3f5",
    surfaceAlpha: dark ? photo.alphaDark : photo.alphaLight,
    glowOpacity: dark ? 0.14 : 0.18
  };
}

/* ========== 内置皮肤配色（浅色 / 深色各一套） ========== */

const LIGHT: Record<string, ScenerySpec> = {
  spring: {
    sky: ["#cfe9d5", "#e3f4e7", "#f1faf3"],
    luminary: { x: 79, y: 15, rx: 9, ry: 13, core: "rgba(255,251,228,0.98)", halo: "rgba(255,236,172,0.42)" },
    clouds: [
      { x: 20, y: 17, rx: 20, ry: 7, alpha: 0.75 },
      { x: 58, y: 26, rx: 14, ry: 5, alpha: 0.5 }
    ],
    hills: [
      { x: 14, y: 117, rx: 60, ry: 44, color: "#8ac79a" },
      { x: 78, y: 121, rx: 70, ry: 46, color: "#a4d5b0" },
      { x: 46, y: 126, rx: 96, ry: 42, color: "#c6e6cd", soft: true }
    ],
    ground: { height: 9, color: "#81bf92" },
    shrubs: { count: 16, color: "#6fb181", size: 1.7 },
    stars: undefined
  },
  summer: {
    sky: ["#cdeefb", "#e6f8fe", "#f5fcff"],
    luminary: { x: 76, y: 13, rx: 10, ry: 14, core: "rgba(255,253,236,0.98)", halo: "rgba(255,226,138,0.45)" },
    clouds: [
      { x: 24, y: 20, rx: 22, ry: 8, alpha: 0.8 },
      { x: 66, y: 30, rx: 16, ry: 6, alpha: 0.55 }
    ],
    hills: [
      { x: 12, y: 118, rx: 62, ry: 42, color: "#5cb8cf" },
      { x: 82, y: 122, rx: 68, ry: 44, color: "#83cbdd" },
      { x: 48, y: 127, rx: 98, ry: 40, color: "#aadcea", soft: true }
    ],
    ground: {
      height: 24,
      color: "#5ab7ce",
      sheen: "linear-gradient(to top, rgba(255,255,255,0.06) 0 24%, rgba(255,255,255,0.42) 24% 25.6%, rgba(255,255,255,0) 31%)"
    }
  },
  autumn: {
    sky: ["#fbe2c2", "#fdf0dd", "#fef8ef"],
    luminary: { x: 81, y: 16, rx: 9, ry: 13, core: "rgba(255,242,208,0.98)", halo: "rgba(255,192,104,0.48)" },
    clouds: [
      { x: 22, y: 18, rx: 19, ry: 6, alpha: 0.65 },
      { x: 60, y: 27, rx: 13, ry: 5, alpha: 0.45 }
    ],
    hills: [
      { x: 16, y: 117, rx: 58, ry: 42, color: "#c8873f" },
      { x: 80, y: 121, rx: 68, ry: 45, color: "#dda55c" },
      { x: 45, y: 126, rx: 94, ry: 40, color: "#eac78e", soft: true }
    ],
    ground: { height: 11, color: "#b9783a" },
    shrubs: { count: 18, color: "#a56c2c", size: 1.4 }
  },
  winter: {
    sky: ["#dde9f6", "#edf5fb", "#f8fbfe"],
    luminary: { x: 77, y: 14, rx: 9, ry: 12, core: "rgba(255,255,255,0.95)", halo: "rgba(200,221,242,0.5)" },
    clouds: [
      { x: 22, y: 18, rx: 22, ry: 7, alpha: 0.85 },
      { x: 62, y: 27, rx: 16, ry: 6, alpha: 0.6 }
    ],
    hills: [
      { x: 14, y: 118, rx: 62, ry: 40, color: "#c1d4ea" },
      { x: 80, y: 122, rx: 70, ry: 43, color: "#d5e3f2" },
      { x: 44, y: 127, rx: 96, ry: 38, color: "#e7f0f9", soft: true }
    ],
    ground: { height: 15, color: "#d9e7f5" }
  },
  anime: {
    sky: ["#ffe2ef", "#fff0f6", "#fff9fb"],
    luminary: { x: 71, y: 38, rx: 21, ry: 26, core: "rgba(255,255,255,0.82)", halo: "rgba(255,186,214,0.42)" },
    clouds: [
      { x: 24, y: 18, rx: 20, ry: 6, alpha: 0.6 },
      { x: 62, y: 24, rx: 15, ry: 5, alpha: 0.42 }
    ],
    hills: [
      { x: 15, y: 117, rx: 60, ry: 43, color: "#efa2c3" },
      { x: 80, y: 121, rx: 68, ry: 45, color: "#f6bed6" },
      { x: 46, y: 126, rx: 95, ry: 40, color: "#fbd9e7", soft: true }
    ],
    ground: { height: 11, color: "#ea99bb" },
    shrubs: { count: 12, color: "#e07fa8", size: 1.4 }
  },
  geek: {
    sky: ["#e3f2e8", "#eef8f1", "#f6fbf7"],
    hills: [
      { x: 14, y: 118, rx: 60, ry: 42, color: "#9fd4b3" },
      { x: 80, y: 122, rx: 70, ry: 44, color: "#bde0c9" },
      { x: 46, y: 127, rx: 96, ry: 40, color: "#d6ecdf", soft: true }
    ],
    ground: { height: 13, color: "#96ccaa" },
    extras: [
      "radial-gradient(ellipse 72% 26% at 50% 100%, rgba(31,206,109,0.20), transparent 100%)",
      "repeating-linear-gradient(0deg, rgba(31,170,100,0.05) 0 1px, transparent 1px 6px)"
    ]
  },
  lavender: {
    sky: ["#ece6fa", "#f5f1fe", "#fbf9ff"],
    luminary: { x: 78, y: 15, rx: 9, ry: 12, core: "rgba(255,255,255,0.94)", halo: "rgba(192,174,240,0.42)" },
    clouds: [
      { x: 22, y: 18, rx: 20, ry: 6, alpha: 0.6 },
      { x: 60, y: 26, rx: 14, ry: 5, alpha: 0.4 }
    ],
    hills: [
      { x: 15, y: 117, rx: 60, ry: 42, color: "#9c85dc" },
      { x: 80, y: 121, rx: 68, ry: 44, color: "#b7a5e8" },
      { x: 46, y: 126, rx: 95, ry: 40, color: "#d4c9f3", soft: true }
    ],
    ground: { height: 11, color: "#9b85db" },
    stars: { count: 12, color: "#ffffff", alpha: 0.55 }
  },
  "chinese-red": {
    sky: ["#9d1420", "#bd2130", "#d43947"],
    // 地平线上方的金色光晕 + 金色放射光束，构成"鎏金朝阳"的地平线
    luminary: { x: 50, y: 120, rx: 64, ry: 48, core: "rgba(255,214,124,0.46)", halo: "rgba(255,196,96,0.2)" },
    clouds: [
      { x: 22, y: 19, rx: 22, ry: 7, alpha: 0.26, color: "#ffd98a" },
      { x: 66, y: 29, rx: 16, ry: 6, alpha: 0.18, color: "#ffd98a" }
    ],
    hills: [
      { x: 14, y: 118, rx: 60, ry: 44, color: "#88111e" },
      { x: 80, y: 122, rx: 70, ry: 46, color: "#9a1724" },
      { x: 46, y: 127, rx: 96, ry: 42, color: "#aa2130", soft: true }
    ],
    ground: { height: 10, color: "#88111e" },
    extras: [
      "repeating-conic-gradient(from 0deg at 50% 120%, rgba(255,206,112,0.11) 0 5deg, transparent 5deg 13deg)"
    ],
    surfaceAlpha: 75
  },
  "chinese-ink": {
    // 宣纸底色 + 朱砂小日 + 三层水墨远山 + 山脚雾气
    sky: ["#f1ece0", "#f8f5ee", "#fdfcf9"],
    luminary: { x: 76, y: 21, rx: 6, ry: 8, core: "rgba(180,53,44,0.9)", halo: "rgba(180,53,44,0.16)" },
    hills: [
      { x: 12, y: 116, rx: 62, ry: 40, color: "#55636f" },
      { x: 74, y: 120, rx: 66, ry: 42, color: "#8492a0", soft: true },
      { x: 44, y: 126, rx: 98, ry: 40, color: "#b4bfc9", soft: true }
    ],
    ground: { height: 8, color: "#e8e2d4" },
    extras: ["linear-gradient(to top, rgba(255,255,255,0.62) 0 5%, rgba(255,255,255,0) 13%)"]
  },
  tech: {
    sky: ["#e9f1fe", "#eef5ff", "#f7faff"],
    luminary: { x: 78, y: 16, rx: 10, ry: 14, core: "rgba(255,255,255,0.9)", halo: "rgba(90,150,255,0.3)" },
    hills: [
      { x: 12, y: 118, rx: 62, ry: 42, color: "#a8bde0" },
      { x: 82, y: 122, rx: 68, ry: 44, color: "#c3d2ec" },
      { x: 46, y: 127, rx: 98, ry: 40, color: "#dce6f7", soft: true }
    ],
    ground: { height: 12, color: "#a9bee1" },
    extras: [
      "radial-gradient(ellipse 70% 24% at 50% 100%, rgba(64,140,255,0.2), transparent 100%)",
      "repeating-linear-gradient(90deg, rgba(60,120,220,0.09) 0 1px, transparent 1px 42px)",
      "repeating-linear-gradient(0deg, rgba(60,120,220,0.08) 0 1px, transparent 1px 34px)"
    ]
  }
};

const DARK: Record<string, ScenerySpec> = {
  spring: {
    sky: ["#0e1711", "#142119", "#1b2b20"],
    luminary: { x: 79, y: 15, rx: 8, ry: 11, core: "rgba(230,243,232,0.94)", halo: "rgba(120,180,132,0.26)" },
    clouds: [
      { x: 20, y: 17, rx: 20, ry: 7, alpha: 0.3 },
      { x: 58, y: 26, rx: 14, ry: 5, alpha: 0.2 }
    ],
    hills: [
      { x: 14, y: 117, rx: 60, ry: 44, color: "#20382a" },
      { x: 78, y: 121, rx: 70, ry: 46, color: "#1a2f23" },
      { x: 46, y: 126, rx: 96, ry: 42, color: "#152619", soft: true }
    ],
    ground: { height: 9, color: "#1d3325" },
    shrubs: { count: 16, color: "#2b4a35", size: 1.7 },
    stars: { count: 26, color: "#d9eddc", alpha: 0.7 }
  },
  summer: {
    sky: ["#0b1a21", "#102530", "#15303b"],
    luminary: { x: 76, y: 13, rx: 9, ry: 13, core: "rgba(228,245,250,0.95)", halo: "rgba(110,196,222,0.26)" },
    clouds: [
      { x: 24, y: 20, rx: 22, ry: 8, alpha: 0.28 },
      { x: 66, y: 30, rx: 16, ry: 6, alpha: 0.18 }
    ],
    hills: [
      { x: 12, y: 118, rx: 62, ry: 42, color: "#14414d" },
      { x: 82, y: 122, rx: 68, ry: 44, color: "#1a4f5d" },
      { x: 48, y: 127, rx: 98, ry: 40, color: "#215e6e", soft: true }
    ],
    ground: {
      height: 24,
      color: "#123a46",
      sheen: "linear-gradient(to top, rgba(180,235,250,0.04) 0 24%, rgba(180,235,250,0.16) 24% 25.6%, rgba(180,235,250,0) 31%)"
    },
    stars: { count: 22, color: "#cfeaf5", alpha: 0.6 }
  },
  autumn: {
    sky: ["#191209", "#241a10", "#2d2116"],
    luminary: { x: 81, y: 16, rx: 8, ry: 12, core: "rgba(255,228,176,0.9)", halo: "rgba(214,142,64,0.26)" },
    clouds: [
      { x: 22, y: 18, rx: 19, ry: 6, alpha: 0.24 },
      { x: 60, y: 27, rx: 13, ry: 5, alpha: 0.16 }
    ],
    hills: [
      { x: 16, y: 117, rx: 58, ry: 42, color: "#3a2a15" },
      { x: 80, y: 121, rx: 68, ry: 45, color: "#45321a" },
      { x: 45, y: 126, rx: 94, ry: 40, color: "#513c20", soft: true }
    ],
    ground: { height: 11, color: "#3d2c17" },
    shrubs: { count: 18, color: "#5a4223", size: 1.4 },
    stars: { count: 20, color: "#f4dfc0", alpha: 0.55 }
  },
  winter: {
    sky: ["#0e1520", "#131d2a", "#192433"],
    luminary: { x: 77, y: 14, rx: 8, ry: 11, core: "rgba(232,242,253,0.95)", halo: "rgba(128,172,222,0.26)" },
    clouds: [
      { x: 22, y: 18, rx: 22, ry: 7, alpha: 0.26 },
      { x: 62, y: 27, rx: 16, ry: 6, alpha: 0.18 }
    ],
    hills: [
      { x: 14, y: 118, rx: 62, ry: 40, color: "#28323f" },
      { x: 80, y: 122, rx: 70, ry: 43, color: "#2f3a48" },
      { x: 44, y: 127, rx: 96, ry: 38, color: "#374352", soft: true }
    ],
    ground: { height: 15, color: "#2b3543" },
    stars: { count: 30, color: "#e2eefb", alpha: 0.75 }
  },
  anime: {
    sky: ["#1d1118", "#261720", "#311f2a"],
    luminary: { x: 71, y: 38, rx: 20, ry: 25, core: "rgba(255,235,244,0.85)", halo: "rgba(236,116,178,0.24)" },
    clouds: [
      { x: 24, y: 18, rx: 20, ry: 6, alpha: 0.22 },
      { x: 62, y: 24, rx: 15, ry: 5, alpha: 0.15 }
    ],
    hills: [
      { x: 15, y: 117, rx: 60, ry: 43, color: "#47283a" },
      { x: 80, y: 121, rx: 68, ry: 45, color: "#3c2130" },
      { x: 46, y: 126, rx: 95, ry: 40, color: "#331d29", soft: true }
    ],
    ground: { height: 11, color: "#3e2333" },
    shrubs: { count: 12, color: "#5c3048", size: 1.6 },
    stars: { count: 28, color: "#ffdfee", alpha: 0.7 }
  },
  geek: {
    sky: ["#060c09", "#0a140f", "#0d1a13"],
    hills: [
      { x: 14, y: 118, rx: 60, ry: 42, color: "#0f2a1c" },
      { x: 80, y: 122, rx: 70, ry: 44, color: "#0c2216" },
      { x: 46, y: 127, rx: 96, ry: 40, color: "#091a11", soft: true }
    ],
    ground: { height: 13, color: "#0d2417" },
    extras: [
      "radial-gradient(ellipse 72% 26% at 50% 100%, rgba(31,206,109,0.22), transparent 100%)",
      "repeating-linear-gradient(0deg, rgba(31,206,109,0.075) 0 1px, transparent 1px 6px)"
    ]
  },
  lavender: {
    sky: ["#0f0c1b", "#161125", "#1e1832"],
    luminary: { x: 78, y: 15, rx: 8, ry: 11, core: "rgba(240,234,255,0.95)", halo: "rgba(142,116,226,0.26)" },
    clouds: [
      { x: 22, y: 18, rx: 20, ry: 6, alpha: 0.24 },
      { x: 60, y: 26, rx: 14, ry: 5, alpha: 0.15 }
    ],
    hills: [
      { x: 15, y: 117, rx: 60, ry: 42, color: "#2a2243" },
      { x: 80, y: 121, rx: 68, ry: 44, color: "#332a50" },
      { x: 46, y: 126, rx: 95, ry: 40, color: "#3c315e", soft: true }
    ],
    ground: { height: 11, color: "#2b2345" },
    stars: { count: 34, color: "#e6ddff", alpha: 0.75 }
  },
  "chinese-red": {
    sky: ["#39060c", "#530a13", "#6c0e1a"],
    luminary: { x: 50, y: 120, rx: 64, ry: 48, core: "rgba(226,168,72,0.32)", halo: "rgba(196,140,60,0.16)" },
    clouds: [
      { x: 22, y: 19, rx: 22, ry: 7, alpha: 0.16, color: "#ffd98a" },
      { x: 66, y: 29, rx: 16, ry: 6, alpha: 0.1, color: "#ffd98a" }
    ],
    hills: [
      { x: 14, y: 118, rx: 60, ry: 44, color: "#240409" },
      { x: 80, y: 122, rx: 70, ry: 46, color: "#2c050b" },
      { x: 46, y: 127, rx: 96, ry: 42, color: "#36070e", soft: true }
    ],
    ground: { height: 10, color: "#260409" },
    extras: [
      "repeating-conic-gradient(from 0deg at 50% 120%, rgba(255,200,110,0.1) 0 5deg, transparent 5deg 13deg)"
    ],
    stars: { count: 20, color: "#ffd98a", alpha: 0.5 },
    surfaceAlpha: 75
  },
  "chinese-ink": {
    sky: ["#0e1319", "#141b23", "#1b242e"],
    luminary: { x: 76, y: 20, rx: 6, ry: 8, core: "rgba(238,242,246,0.9)", halo: "rgba(150,180,205,0.2)" },
    hills: [
      { x: 12, y: 116, rx: 62, ry: 40, color: "#2b3641" },
      { x: 74, y: 120, rx: 66, ry: 42, color: "#3a4652", soft: true },
      { x: 44, y: 126, rx: 98, ry: 40, color: "#4b5866", soft: true }
    ],
    ground: { height: 8, color: "#1d262f" },
    extras: ["linear-gradient(to top, rgba(190,215,235,0.13) 0 5%, rgba(255,255,255,0) 13%)"],
    stars: { count: 16, color: "#dbe8f3", alpha: 0.5 }
  },
  tech: {
    sky: ["#070c18", "#0a1222", "#0e1830"],
    luminary: { x: 78, y: 16, rx: 9, ry: 13, core: "rgba(210,235,255,0.92)", halo: "rgba(70,150,255,0.26)" },
    hills: [
      { x: 12, y: 118, rx: 62, ry: 42, color: "#0f1a30" },
      { x: 82, y: 122, rx: 68, ry: 44, color: "#142442" },
      { x: 46, y: 127, rx: 98, ry: 40, color: "#1a2c52", soft: true }
    ],
    ground: { height: 12, color: "#101c34" },
    extras: [
      "radial-gradient(ellipse 70% 24% at 50% 100%, rgba(64,160,255,0.24), transparent 100%)",
      "repeating-linear-gradient(90deg, rgba(80,170,255,0.13) 0 1px, transparent 1px 42px)",
      "repeating-linear-gradient(0deg, rgba(80,170,255,0.1) 0 1px, transparent 1px 34px)"
    ],
    stars: { count: 24, color: "#bfe0ff", alpha: 0.6 }
  }
};

/* ========== 颜色与随机数工具 ========== */

function hexToRgb(hex: string): [number, number, number] {
  const normalized = hex.replace("#", "");
  const value = normalized.length === 3
    ? normalized.split("").map((c) => c + c).join("")
    : normalized;
  return [
    parseInt(value.slice(0, 2), 16),
    parseInt(value.slice(2, 4), 16),
    parseInt(value.slice(4, 6), 16)
  ];
}

function toHex(rgb: [number, number, number]): string {
  return `#${rgb.map((c) => Math.round(Math.min(255, Math.max(0, c))).toString(16).padStart(2, "0")).join("")}`;
}

/** 按比例混合两个颜色，t=0 取 a，t=1 取 b。 */
function mix(a: string, b: string, t: number): string {
  const [r1, g1, b1] = hexToRgb(a);
  const [r2, g2, b2] = hexToRgb(b);
  return toHex([r1 + (r2 - r1) * t, g1 + (g2 - g1) * t, b1 + (b2 - b1) * t]);
}

function rgba(hex: string, alpha: number): string {
  const [r, g, b] = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** 固定种子的伪随机数，保证同一皮肤的星点/树丛每次生成都落在相同位置。 */
function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function seedOf(text: string): number {
  let hash = 2166136261;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

/* ========== 图层拼装 ========== */

function skyLayer(spec: ScenerySpec): string {
  const stops = spec.sky;
  const positions = stops.length === 3
    ? [0, 42, 100]
    : stops.map((_, index) => Math.round((index / Math.max(1, stops.length - 1)) * 100));
  return `linear-gradient(180deg, ${stops.map((color, index) => `${color} ${positions[index]}%`).join(", ")})`;
}

function luminaryLayer(luminary: Luminary): string {
  return [
    `radial-gradient(ellipse ${luminary.rx}% ${luminary.ry}% at ${luminary.x}% ${luminary.y}%,`,
    `${luminary.core} 0 26%, ${luminary.halo} 42%, transparent 100%)`
  ].join(" ");
}

function cloudLayer(cloud: Cloud): string {
  const color = cloud.color ?? "#ffffff";
  return `radial-gradient(ellipse ${cloud.rx}% ${cloud.ry}% at ${cloud.x}% ${cloud.y}%, ${rgba(color, cloud.alpha)} 0 42%, ${rgba(color, 0)} 100%)`;
}

function hillLayer(hill: Hill): string {
  const stops = hill.soft
    ? `${hill.color} 0 42%, transparent 100%`
    : `${hill.color} 0 68%, transparent 69%`;
  return `radial-gradient(ellipse ${hill.rx}% ${hill.ry}% at ${hill.x}% ${hill.y}%, ${stops})`;
}

function groundLayer(ground: NonNullable<ScenerySpec["ground"]>): string {
  const band = `linear-gradient(to top, ${ground.color} 0 ${ground.height}%, transparent ${ground.height}%)`;
  return ground.sheen ? `${ground.sheen}, ${band}` : band;
}

function shrubLayer(skinId: string, shrubs: NonNullable<ScenerySpec["shrubs"]>): string {
  const random = seededRandom(seedOf(`${skinId}-shrubs`));
  const layers: string[] = [];
  for (let i = 0; i < shrubs.count; i++) {
    const x = random() * 100;
    // 树丛贴着丘陵线，越靠左越远（位置更高一些）
    const y = 84 + random() * 13;
    // 半径必须用长度：radial-gradient 的 circle 不接受百分比
    const size = (2.4 + random() * 2.6) * shrubs.size;
    layers.push(
      `radial-gradient(circle ${size.toFixed(2)}px at ${x.toFixed(1)}% ${y.toFixed(1)}%, ${shrubs.color} 0 58%, transparent 100%)`
    );
  }
  return layers.join(", ");
}

function starLayer(skinId: string, stars: NonNullable<ScenerySpec["stars"]>): string {
  const random = seededRandom(seedOf(`${skinId}-stars`));
  const layers: string[] = [];
  for (let i = 0; i < stars.count; i++) {
    const x = random() * 100;
    const y = random() * 62;
    const alpha = (0.35 + random() * 0.65) * stars.alpha;
    layers.push(
      `radial-gradient(circle ${(0.8 + random() * 1.1).toFixed(2)}px at ${x.toFixed(1)}% ${y.toFixed(1)}%, ${rgba(stars.color, Number(alpha.toFixed(2)))} 0 55%, transparent 100%)`
    );
  }
  return layers.join(", ");
}

function buildFromSpec(skinId: string, spec: ScenerySpec, dark: boolean): SceneryResult {
  // 图层从上往下：星点与叠层特效 → 云 → 日月 → 树丛 → 丘陵（近到远）
  // → 地面/水面色带 → 天空渐变。
  const layers: string[] = [];

  if (spec.stars) layers.push(starLayer(skinId, spec.stars));
  for (const cloud of spec.clouds ?? []) layers.push(cloudLayer(cloud));
  if (spec.luminary) layers.push(luminaryLayer(spec.luminary));
  if (spec.shrubs) layers.push(shrubLayer(skinId, spec.shrubs));
  for (const hill of spec.hills ?? []) layers.push(hillLayer(hill));
  if (spec.ground) layers.push(groundLayer(spec.ground));
  for (const extra of spec.extras ?? []) layers.push(extra);
  layers.push(skyLayer(spec));

  return {
    image: layers.join(", "),
    color: spec.sky[spec.sky.length - 1],
    surfaceAlpha: spec.surfaceAlpha ?? (dark ? SURFACE_ALPHA.dark : SURFACE_ALPHA.light),
    glowOpacity: dark ? GLOW_OPACITY.dark : GLOW_OPACITY.light
  };
}

/** 自定义皮肤没有预设风景，按用户选的主色与背景调现场生成一套同色系丘陵。 */
function buildCustomScenery(config: CustomSkinConfig, dark: boolean): SceneryResult {
  // 用户自选了背景图（数据 URI，已在设置里压到 1600px 内）时优先用它：
  // 图片上面盖一层薄雾保证文字可读，面板也调得更实一些。
  if (config.backgroundImage) {
    const veil = dark
      ? "linear-gradient(rgba(8, 10, 14, 0.4), rgba(8, 10, 14, 0.4))"
      : "linear-gradient(rgba(255, 255, 255, 0.24), rgba(255, 255, 255, 0.24))";
    return {
      image: `${veil}, url("${config.backgroundImage}")`,
      color: dark ? "#0f1115" : "#f4f4f6",
      surfaceAlpha: dark ? 74 : 72,
      glowOpacity: 0.08
    };
  }

  const primary = config.primary;
  const tint = dark ? config.tintDark : config.tintLight;
  const base = tint ?? mix(primary, dark ? "#121212" : "#ffffff", dark ? 0.88 : 0.93);
  const spec: ScenerySpec = dark
    ? {
        sky: [mix(primary, base, 0.86), base, mix(base, "#ffffff", 0.08)],
        luminary: {
          x: 78,
          y: 15,
          rx: 8,
          ry: 11,
          core: "rgba(244,242,250,0.92)",
          halo: rgba(mix(primary, "#ffffff", 0.35), 0.28)
        },
        hills: [
          { x: 15, y: 117, rx: 60, ry: 42, color: mix(primary, base, 0.24) },
          { x: 80, y: 121, rx: 68, ry: 44, color: mix(primary, base, 0.14) },
          { x: 46, y: 126, rx: 95, ry: 40, color: mix(primary, base, 0.05), soft: true }
        ],
        ground: { height: 11, color: mix(primary, base, 0.2) },
        stars: { count: 24, color: mix(primary, "#ffffff", 0.7), alpha: 0.6 }
      }
    : {
        sky: [mix(primary, base, 0.22), base, mix(base, "#ffffff", 0.55)],
        luminary: {
          x: 79,
          y: 15,
          rx: 9,
          ry: 13,
          core: "rgba(255,253,246,0.95)",
          halo: rgba(mix(primary, "#ffffff", 0.55), 0.42)
        },
        clouds: [
          { x: 22, y: 18, rx: 20, ry: 7, alpha: 0.6 },
          { x: 60, y: 26, rx: 14, ry: 5, alpha: 0.4 }
        ],
        hills: [
          { x: 15, y: 117, rx: 60, ry: 43, color: mix(primary, base, 0.32) },
          { x: 80, y: 121, rx: 68, ry: 45, color: mix(primary, base, 0.5) },
          { x: 46, y: 126, rx: 95, ry: 40, color: mix(primary, base, 0.68), soft: true }
        ],
        ground: { height: 11, color: mix(primary, base, 0.36) },
        shrubs: { count: 12, color: mix(primary, base, 0.18), size: 1.6 }
      };

  return buildFromSpec(config.name || "custom", spec, dark);
}

/**
 * 生成指定皮肤的背景；经典皮肤与未配置的自定义皮肤返回 null（保持纯净底）。
 *
 * - 内置皮肤：backdropStyle 为 "photo" 时优先用实景照片，否则用渐变风景；
 * - 自定义皮肤：用户自选了背景图就用图片，否则按主色生成同色系渐变风景。
 */
export function buildSkinScenery(
  skinId: string,
  dark: boolean,
  customSkin?: CustomSkinConfig | null,
  style: BackdropStyle = "photo"
): SceneryResult | null {
  if (skinId === "custom") {
    return customSkin ? buildCustomScenery(customSkin, dark) : null;
  }
  const photo = style === "photo" ? PHOTOS[skinId] : undefined;
  if (photo) {
    return photoScenery(photo, dark);
  }
  const spec = (dark ? DARK : LIGHT)[skinId];
  if (!spec) return null;
  return buildFromSpec(skinId, spec, dark);
}
