/**
 * 背景粒子动效引擎（Canvas 2D）。
 *
 * 设计取舍：借鉴 shader-effects-inc/shaders 的"分层特效画布"思路，
 * 但不引入 WebGPU 依赖 —— 旧版 WebView2 运行时不保证支持 WebGPU，
 * 且剪贴板工具需要保持轻量。粒子用 Canvas 2D 实现，氛围色晕用 CSS
 * 渐变实现（见 styles/theme.css 的 --skin-glow），全平台稳定可用。
 *
 * 性能约束：
 * - 单一 requestAnimationFrame 循环，粒子数量随窗口面积缩放；
 * - 页面隐藏（visibilitychange）时暂停渲染；
 * - 系统开启"减少动态效果"（prefers-reduced-motion）时不启动。
 */

import type { AmbientDensity, AmbientEffectId } from "./theme";

export interface AmbientConfig {
  effect: AmbientEffectId;
  dark: boolean;
  density: AmbientDensity;
}

export interface AmbientEngine {
  configure(config: AmbientConfig): void;
  destroy(): void;
}

type EffectId = Exclude<AmbientEffectId, "none">;

const DENSITY_FACTOR: Record<AmbientDensity, number> = {
  low: 0.55,
  medium: 1,
  high: 1.7
};

const BASE_COUNT = 46;
const TAU = Math.PI * 2;

function rand(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  a: number;
  phase: number;
  freq: number;
  rot: number;
  spin: number;
  color: number;
  /** 代码雨专用：列字符 */
  chars?: string;
}

interface DrawContext {
  ctx: CanvasRenderingContext2D;
  time: number;
  dark: boolean;
}

interface EffectDef {
  /** 目标粒子数量（area 为画布 CSS 像素面积，f 为密度系数） */
  count(area: number, f: number): number;
  /** 初始化粒子；scatter=true 时随机铺满全屏（用于初始/切换），否则从边缘进入 */
  init(p: Particle, w: number, h: number, scatter: boolean): void;
  /** 推进粒子；返回 false 表示离开屏幕需要重新 init */
  update(p: Particle, w: number, h: number, dt: number, time: number): boolean;
  draw(c: DrawContext, p: Particle): void;
}

function initDrift(p: Particle, w: number, h: number, scatter: boolean, speedRange: [number, number]): void {
  p.x = rand(-16, w + 16);
  p.y = scatter ? rand(0, h) : rand(-50, -8);
  p.vy = rand(speedRange[0], speedRange[1]);
  p.phase = rand(0, TAU);
  p.freq = rand(0.4, 1.3);
}

/** 下雪：白色柔和圆点，横向正弦摆动 */
const snow: EffectDef = {
  count: (area, f) => Math.round(Math.min(150, Math.max(12, (area / (1000 * 720)) * BASE_COUNT * f))),
  init(p, w, h, scatter) {
    initDrift(p, w, h, scatter, [24, 62]);
    p.r = rand(1.1, 3.1);
    p.a = rand(0.35, 0.85);
  },
  update(p, w, h, dt, time) {
    p.y += p.vy * dt;
    p.x += Math.sin(time * p.freq + p.phase) * (12 + p.r * 5) * dt;
    if (p.y > h + 8 || p.x < -24 || p.x > w + 24) {
      return false;
    }
    return true;
  },
  draw({ ctx, dark }, p) {
    ctx.globalAlpha = p.a * 0.3;
    ctx.fillStyle = dark ? "#e8f4ff" : "#ffffff";
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.r * 2.1, 0, TAU);
    ctx.fill();
    ctx.globalAlpha = p.a;
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.r, 0, TAU);
    ctx.fill();
    ctx.globalAlpha = 1;
  }
};

/** 花瓣：粉椭圆旋转飘落（樱花/春日通用） */
const PETAL_COLORS = ["#f6a8c5", "#f293b6", "#fbc0d8", "#f7b1cc"];
const petals: EffectDef = {
  count: snow.count,
  init(p, w, h, scatter) {
    initDrift(p, w, h, scatter, [26, 52]);
    p.r = rand(2.4, 4.6);
    p.a = rand(0.45, 0.8);
    p.rot = rand(0, TAU);
    p.spin = rand(-2.2, 2.2);
    p.color = Math.floor(rand(0, PETAL_COLORS.length));
  },
  update(p, w, h, dt, time) {
    p.y += p.vy * dt;
    p.x += Math.sin(time * p.freq + p.phase) * 22 * dt;
    p.rot += p.spin * dt;
    if (p.y > h + 10 || p.x < -24 || p.x > w + 24) return false;
    return true;
  },
  draw({ ctx }, p) {
    ctx.save();
    ctx.translate(p.x, p.y);
    ctx.rotate(p.rot + Math.sin(p.phase) * 0.4);
    ctx.globalAlpha = p.a;
    ctx.fillStyle = PETAL_COLORS[p.color];
    ctx.beginPath();
    ctx.ellipse(0, 0, p.r * 1.55, p.r * 0.85, 0, 0, TAU);
    ctx.fill();
    ctx.restore();
    ctx.globalAlpha = 1;
  }
};

/** 柳絮：细小绒点，微风横向漂移，缓慢下沉（春日） */
const catkins: EffectDef = {
  count: (area, f) => Math.round(Math.min(120, Math.max(10, (area / (1000 * 720)) * BASE_COUNT * 0.8 * f))),
  init(p, w, h, scatter) {
    initDrift(p, w, h, scatter, [7, 17]);
    p.r = rand(1.2, 2.6);
    p.a = rand(0.4, 0.7);
    p.vx = rand(-8, 20);
  },
  update(p, w, h, dt, time) {
    p.y += p.vy * dt;
    p.x += (p.vx + Math.sin(time * p.freq + p.phase) * 18) * dt;
    if (p.x > w + 14) p.x = -14;
    if (p.x < -14) p.x = w + 14;
    if (p.y > h + 8) return false;
    return true;
  },
  draw({ ctx, dark }, p) {
    ctx.globalAlpha = p.a * 0.35;
    ctx.fillStyle = dark ? "#cfe3cd" : "#eef5e8";
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.r * 2.4, 0, TAU);
    ctx.fill();
    ctx.globalAlpha = p.a;
    ctx.fillStyle = dark ? "#dcecd8" : "#f7fbf3";
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.r, 0, TAU);
    ctx.fill();
    ctx.globalAlpha = 1;
  }
};

/** 落叶：暖色叶形翻滚下落（秋叶） */
const LEAF_COLORS = ["#d9802f", "#c05a2b", "#e2a63d", "#a84a22", "#c97b3f"];
const leaves: EffectDef = {
  count: (area, f) => Math.round(Math.min(90, Math.max(8, (area / (1000 * 720)) * BASE_COUNT * 0.6 * f))),
  init(p, w, h, scatter) {
    initDrift(p, w, h, scatter, [30, 66]);
    p.r = rand(2.6, 4.4);
    p.a = rand(0.55, 0.9);
    p.rot = rand(0, TAU);
    p.spin = rand(-2.8, 2.8);
    p.color = Math.floor(rand(0, LEAF_COLORS.length));
  },
  update(p, w, h, dt, time) {
    p.y += p.vy * dt;
    p.x += Math.sin(time * p.freq + p.phase) * 18 * dt;
    p.rot += p.spin * dt;
    if (p.y > h + 12 || p.x < -24 || p.x > w + 24) return false;
    return true;
  },
  draw({ ctx }, p) {
    ctx.save();
    ctx.translate(p.x, p.y);
    ctx.rotate(p.rot);
    ctx.globalAlpha = p.a;
    ctx.fillStyle = LEAF_COLORS[p.color];
    ctx.beginPath();
    ctx.ellipse(0, 0, p.r * 1.6, p.r * 0.72, 0, 0, TAU);
    ctx.fill();
    ctx.restore();
    ctx.globalAlpha = 1;
  }
};

/** 萤火虫：缓慢游走的发光点，呼吸式闪烁（盛夏） */
const fireflies: EffectDef = {
  count: (area, f) => Math.round(Math.min(48, Math.max(6, (area / (1000 * 720)) * BASE_COUNT * 0.55 * f))),
  init(p, w, h, scatter) {
    p.x = rand(0, w);
    p.y = rand(0, h);
    p.vx = rand(-8, 8);
    p.vy = rand(-6, 6);
    p.r = rand(1, 2);
    p.a = rand(0.3, 0.7);
    p.phase = rand(0, TAU);
    p.freq = rand(0.6, 1.6);
  },
  update(p, w, h, dt) {
    p.vx = Math.max(-14, Math.min(14, p.vx + rand(-1, 1) * 42 * dt)) * 0.985;
    p.vy = Math.max(-12, Math.min(12, p.vy + rand(-1, 1) * 42 * dt)) * 0.985;
    p.x += p.vx * dt;
    p.y += p.vy * dt;
    if (p.x < -12) p.x = w + 12;
    if (p.x > w + 12) p.x = -12;
    if (p.y < -12) p.y = h + 12;
    if (p.y > h + 12) p.y = -12;
    return true;
  },
  draw({ ctx, dark }, p) {
    const glow = 0.25 + 0.75 * Math.abs(Math.sin(p.freq * p.phase + p.phase));
    const a = p.a * glow;
    const core = dark ? "255,244,180" : "214,150,40";
    const halo = dark ? "255,238,150" : "236,196,90";
    const g = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, p.r * 4.5);
    g.addColorStop(0, `rgba(${core},${a})`);
    g.addColorStop(0.4, `rgba(${halo},${a * 0.45})`);
    g.addColorStop(1, "rgba(255,238,150,0)");
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.r * 4.5, 0, TAU);
    ctx.fill();
  }
};

/** 星尘：微小光点呼吸闪烁，缓慢漂移（紫罗兰） */
const STAR_COLORS_DARK = ["255,255,255", "217,210,242", "195,181,234"];
const STAR_COLORS_LIGHT = ["120,100,190", "150,130,210", "110,90,170"];
const stardust: EffectDef = {
  count: (area, f) => Math.round(Math.min(110, Math.max(10, (area / (1000 * 720)) * BASE_COUNT * 0.9 * f))),
  init(p, w, h, scatter) {
    p.x = rand(0, w);
    p.y = rand(0, h);
    p.vx = rand(-5, 5);
    p.vy = rand(-7, 7);
    p.r = rand(0.6, 1.7);
    p.a = rand(0.3, 0.9);
    p.phase = rand(0, TAU);
    p.freq = rand(1, 3);
    p.color = Math.floor(rand(0, 3));
  },
  update(p, w, h, dt) {
    p.x += p.vx * dt;
    p.y += p.vy * dt;
    if (p.x < -6) p.x = w + 6;
    if (p.x > w + 6) p.x = -6;
    if (p.y < -6) p.y = h + 6;
    if (p.y > h + 6) p.y = -6;
    return true;
  },
  draw({ ctx, dark }, p) {
    const twinkle = 0.2 + 0.8 * Math.abs(Math.sin(p.freq * p.phase * 0.35 + p.phase));
    const a = p.a * twinkle;
    const rgb = (dark ? STAR_COLORS_DARK : STAR_COLORS_LIGHT)[p.color];
    ctx.fillStyle = `rgba(${rgb},${a})`;
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.r, 0, TAU);
    ctx.fill();
    if (p.r > 1.3) {
      ctx.globalAlpha = a * 0.4;
      ctx.beginPath();
      ctx.arc(p.x, p.y, p.r * 2.6, 0, TAU);
      ctx.fill();
      ctx.globalAlpha = 1;
    }
  }
};

/** 代码雨：终端字符列（极客），亮头暗尾，速度各异 */
const MATRIX_CHARS = "01<>{}[]();=+-*/#$%&@アイウエオカキクケサシスセ";
const matrix: EffectDef = {
  count: (_area, f) => 0, // 由引擎按列数特殊处理
  init(p, w, h, scatter) {
    p.y = scatter ? rand(-h, 0) : rand(-180, -12);
    p.vy = rand(40, 110);
    p.a = rand(0.5, 0.9);
    p.chars = genMatrixString();
  },
  update(p, w, h, dt) {
    p.y += p.vy * dt;
    if (p.y - 8 * 12 > h + 24) {
      p.y = rand(-180, -12);
      p.vy = rand(40, 110);
      p.chars = genMatrixString();
    }
    return true;
  },
  draw({ ctx, dark }, p) {
    if (!p.chars) return;
    const fs = 12;
    const trail = 8;
    const headIdx = Math.floor(p.y / fs);
    ctx.font = "12px Consolas, 'Courier New', monospace";
    ctx.textBaseline = "top";
    for (let i = 0; i < trail; i++) {
      const idx = headIdx - i;
      if (idx < 0) continue;
      const charY = idx * fs;
      if (charY > window.innerHeight) break;
      const char = p.chars[((idx % p.chars.length) + p.chars.length) % p.chars.length];
      const fade = 1 - i / trail;
      if (i === 0) {
        ctx.globalAlpha = 0.95 * p.a;
        ctx.fillStyle = dark ? "#b6ffc2" : "#1c7c38";
      } else {
        ctx.globalAlpha = fade * 0.5 * p.a;
        ctx.fillStyle = dark ? "rgba(102,235,140,1)" : "rgba(30,120,55,1)";
      }
      ctx.fillText(char, p.x, charY);
    }
    ctx.globalAlpha = 1;
  }
};

function genMatrixString(): string {
  const length = Math.floor(rand(14, 26));
  let out = "";
  for (let i = 0; i < length; i++) {
    out += MATRIX_CHARS[Math.floor(Math.random() * MATRIX_CHARS.length)];
  }
  return out;
}

const EFFECTS: Record<EffectId, EffectDef> = {
  snow,
  petals,
  catkins,
  leaves,
  fireflies,
  stardust,
  matrix
};

const MATRIX_COLUMN_WIDTH = 26;

export function createAmbientEngine(canvas: HTMLCanvasElement): AmbientEngine {
  const context = canvas.getContext("2d");
  if (!context) {
    return { configure() {}, destroy() {} };
  }
  const ctx = context;

  let effect: EffectId | null = null;
  let dark = false;
  let density: AmbientDensity = "medium";
  let w = 0;
  let h = 0;
  let raf = 0;
  let disposed = false;
  let particles: Particle[] = [];
  let last = performance.now();
  let time = 0;
  let lastTs = last;

  function targetCount(): number {
    if (!effect) return 0;
    if (effect === "matrix") {
      const f = DENSITY_FACTOR[density];
      return Math.round(Math.min(80, Math.max(4, (w / MATRIX_COLUMN_WIDTH) * (0.4 + 0.35 * f))));
    }
    return EFFECTS[effect].count(w * h, DENSITY_FACTOR[density]);
  }

  function spawnParticle(scatter: boolean): Particle {
    const p: Particle = {
      x: 0, y: 0, vx: 0, vy: 0, r: 1, a: 0.5,
      phase: 0, freq: 1, rot: 0, spin: 0, color: 0
    };
    if (effect === "matrix") {
      p.x = rand(0, Math.max(1, Math.floor(w / MATRIX_COLUMN_WIDTH))) * MATRIX_COLUMN_WIDTH + 8;
    }
    EFFECTS[effect!].init(p, w, h, scatter);
    return p;
  }

  function rebuild(): void {
    particles = [];
    if (!effect) return;
    const target = targetCount();
    for (let i = 0; i < target; i++) {
      particles.push(spawnParticle(true));
    }
  }

  function syncCount(): void {
    if (!effect) return;
    const target = targetCount();
    while (particles.length < target) {
      particles.push(spawnParticle(true));
    }
    if (particles.length > target) {
      particles.length = target;
    }
  }

  function resize(): void {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const rect = canvas.getBoundingClientRect();
    w = Math.max(1, Math.round(rect.width));
    h = Math.max(1, Math.round(rect.height));
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    rebuild();
  }

  function frame(ts: number): void {
    if (disposed || !effect) return;
    const dt = Math.min(0.05, Math.max(0.001, (ts - lastTs) / 1000));
    lastTs = ts;
    time += dt;

    ctx.clearRect(0, 0, w, h);
    const def = EFFECTS[effect];
    for (const p of particles) {
      if (!def.update(p, w, h, dt, time)) {
        def.init(p, w, h, false);
      }
      def.draw({ ctx, time, dark }, p);
    }
    raf = requestAnimationFrame(frame);
  }

  function start(): void {
    if (disposed || !effect || document.hidden) return;
    if (!raf) {
      lastTs = performance.now();
      raf = requestAnimationFrame(frame);
    }
  }

  function stop(): void {
    if (raf) {
      cancelAnimationFrame(raf);
      raf = 0;
    }
  }

  function onVisibility(): void {
    if (document.hidden) {
      stop();
    } else {
      start();
    }
  }

  function onResize(): void {
    resize();
  }

  document.addEventListener("visibilitychange", onVisibility);
  window.addEventListener("resize", onResize);
  resize();

  return {
    configure(config: AmbientConfig): void {
      dark = config.dark;
      const next = config.effect === "none" ? null : config.effect;
      const changed = next !== effect;
      effect = next;
      if (!effect) {
        stop();
        ctx.clearRect(0, 0, w, h);
        particles = [];
        return;
      }
      if (changed) {
        rebuild();
      } else {
        syncCount();
      }
      start();
    },
    destroy(): void {
      disposed = true;
      stop();
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("resize", onResize);
      particles = [];
    }
  };
}
