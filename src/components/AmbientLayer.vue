<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  loadThemeState,
  resolveAmbientEffect,
  subscribeThemeApplied,
  type AmbientDensity,
  type AmbientEffectId,
  type ThemeState
} from "@/services/theme";

import "@/styles/ambient.css";

/**
 * 背景氛围层（纯 CSS）。
 *
 * 只做两件事：
 * 1. 生成一批粒子 DOM，给每颗写一次 CSS 变量（位置 / 尺寸 / 时长 / 延迟 / 摆动幅度…）；
 * 2. 把 data-effect 挂到容器上，由 styles/ambient.css 按效果接管形状与动画。
 *
 * 运行期没有任何逐帧脚本：下落、摇摆、自转、翻转、闪烁全是 @keyframes，
 * 由合成器直接跑，切到后台窗口时浏览器会自动降频。
 * 粒子数量随窗口面积和密度档位变化，弹窗（360×500）只有主窗口的四成左右。
 */

type EffectId = Exclude<AmbientEffectId, "none">;

interface Particle {
  key: string;
  style: Record<string, string>;
  chars?: string;
}

/** 各效果的基准粒子数（按 1280×800 视口、medium 密度） */
const BASE_COUNT: Record<EffectId, number> = {
  snow: 70,
  petals: 54,
  catkins: 52,
  leaves: 28,
  fireflies: 34,
  stardust: 74,
  spark: 46,
  matrix: 22
};

const DENSITY_FACTOR: Record<AmbientDensity, number> = {
  low: 0.55,
  medium: 1,
  high: 1.6
};

/** 代码雨字符集：0/1 + 符号 + 片假名，接近终端观感 */
const MATRIX_CHARS = "01<>{}[]()=+-*/#$%&@アイウエオカキクケサシスセ";

const rnd = (min: number, max: number): number => min + Math.random() * (max - min);
const fix = (value: number, digits = 1): string => value.toFixed(digits);
const deg = (value: number): string => `${Math.round(value)}deg`;
const px = (value: number): string => `${fix(value)}px`;
const sec = (value: number): string => `${fix(value)}s`;

/** 负延迟：让粒子一开始就散布在整条轨道上，而不是同时从顶部落下 */
const negativeDelay = (max: number): string => `-${fix(rnd(0, max))}s`;

function matrixChars(): string {
  const length = Math.round(rnd(5, 13));
  let out = "";
  for (let i = 0; i < length; i += 1) {
    out += MATRIX_CHARS[Math.floor(Math.random() * MATRIX_CHARS.length)];
  }
  return out;
}

/** 樱花花瓣：慢落、大摆幅、自转 + 整片翻面 */
function petalParticle(index: number, scale: number): Particle {
  const size = rnd(9, 19) * scale;
  return {
    key: `petal-${index}`,
    style: {
      "--pf-x": `${fix(rnd(-3, 101))}%`,
      "--pf-size": px(size),
      "--pf-dur": sec(rnd(14, 27)),
      "--pf-delay": negativeDelay(27),
      "--pf-dx": `${fix(rnd(-13, 13))}vw`,
      "--pf-sway": px(rnd(9, 30)),
      "--pf-sway-dur": sec(rnd(2.6, 5.4)),
      "--pf-sway-delay": negativeDelay(5.4),
      "--pf-r0": deg(rnd(0, 360)),
      "--pf-turn-dur": sec(rnd(6, 14)),
      "--pf-turn-delay": negativeDelay(14),
      "--pf-flip-dur": sec(rnd(3.6, 7.2)),
      "--pf-flip-delay": negativeDelay(7.2),
      "--pf-o": fix(rnd(0.6, 0.95), 2)
    }
  };
}

/** 冬雪：三档景深，越大越糊 */
function snowParticle(index: number, scale: number): Particle {
  const size = rnd(3.6, 11) * scale;
  const blur = size > 5.2 ? rnd(0.5, 1.5) : rnd(0, 0.25);
  return {
    key: `snow-${index}`,
    style: {
      "--pf-x": `${fix(rnd(-2, 102))}%`,
      "--pf-size": px(size),
      "--pf-dur": sec(rnd(9, 22)),
      "--pf-delay": negativeDelay(22),
      "--pf-dx": `${fix(rnd(-7, 7))}vw`,
      "--pf-sway": px(rnd(5, 16)),
      "--pf-sway-dur": sec(rnd(2.4, 5)),
      "--pf-sway-delay": negativeDelay(5),
      "--pf-blur": px(blur),
      "--pf-o": fix(rnd(0.5, 0.98), 2)
    }
  };
}

/** 柳絮：几乎没有重量，落得极慢、飘得极远 */
function catkinParticle(index: number, scale: number): Particle {
  const size = rnd(6, 14) * scale;
  return {
    key: `catkin-${index}`,
    style: {
      "--pf-x": `${fix(rnd(-4, 104))}%`,
      "--pf-size": px(size),
      "--pf-dur": sec(rnd(19, 36)),
      "--pf-delay": negativeDelay(36),
      "--pf-dx": `${fix(rnd(-11, 11))}vw`,
      "--pf-sway": px(rnd(18, 58)),
      "--pf-sway-dur": sec(rnd(7, 13)),
      "--pf-sway-delay": negativeDelay(13),
      "--pf-blur": px(rnd(0.3, 1.1)),
      "--pf-o": fix(rnd(0.5, 0.85), 2)
    }
  };
}

/** 落叶：叶形翻滚，翻面幅度大 */
function leafParticle(index: number, scale: number): Particle {
  const size = rnd(11, 21) * scale;
  return {
    key: `leaf-${index}`,
    style: {
      "--pf-x": `${fix(rnd(-3, 101))}%`,
      "--pf-size": px(size),
      "--pf-dur": sec(rnd(10, 19)),
      "--pf-delay": negativeDelay(19),
      "--pf-dx": `${fix(rnd(-11, 11))}vw`,
      "--pf-sway": px(rnd(10, 28)),
      "--pf-sway-dur": sec(rnd(2.2, 4.6)),
      "--pf-sway-delay": negativeDelay(4.6),
      "--pf-r0": deg(rnd(0, 360)),
      "--pf-turn-dur": sec(rnd(5, 11)),
      "--pf-turn-delay": negativeDelay(11),
      "--pf-flip-dur": sec(rnd(3.6, 6.4)),
      "--pf-flip-delay": negativeDelay(6.4),
      "--pf-o": fix(rnd(0.6, 0.92), 2)
    }
  };
}

/** 萤火虫：椭圆游走 + 呼吸，两者周期错开 */
function fireflyParticle(index: number, scale: number): Particle {
  const size = rnd(4.5, 9) * scale;
  return {
    key: `firefly-${index}`,
    style: {
      "--pf-x": `${fix(rnd(3, 97))}%`,
      "--pf-y": `${fix(rnd(6, 94))}%`,
      "--pf-size": px(size),
      "--pf-dur": sec(rnd(9, 19)),
      "--pf-delay": negativeDelay(19),
      "--pf-ax": px(rnd(18, 56)),
      "--pf-ay": px(rnd(12, 40)),
      "--pf-sway-dur": sec(rnd(2.6, 5.6)),
      "--pf-sway-delay": negativeDelay(5.6),
      "--pf-o": fix(rnd(0.5, 0.9), 2)
    }
  };
}

/** 星尘：原地微漂 + 闪烁 */
function stardustParticle(index: number, scale: number): Particle {
  const size = rnd(3.6, 7.6) * scale;
  return {
    key: `star-${index}`,
    style: {
      "--pf-x": `${fix(rnd(1, 99))}%`,
      "--pf-y": `${fix(rnd(2, 98))}%`,
      "--pf-size": px(size),
      "--pf-dur": sec(rnd(16, 30)),
      "--pf-delay": negativeDelay(30),
      "--pf-ax": px(rnd(5, 18)),
      "--pf-ay": px(rnd(4, 14)),
      "--pf-sway-dur": sec(rnd(2.4, 6)),
      "--pf-sway-delay": negativeDelay(6),
      "--pf-o": fix(rnd(0.45, 1), 2)
    }
  };
}

/** 光点上升：自下而上，带一小段拖尾 */
function sparkParticle(index: number, scale: number): Particle {
  const size = rnd(3.2, 8) * scale;
  return {
    key: `spark-${index}`,
    style: {
      "--pf-x": `${fix(rnd(1, 99))}%`,
      "--pf-size": px(size),
      "--pf-dur": sec(rnd(9, 19)),
      "--pf-delay": negativeDelay(19),
      "--pf-dx": `${fix(rnd(-5, 5))}vw`,
      "--pf-trail": px(rnd(8, 18)),
      "--pf-o": fix(rnd(0.5, 0.9), 2)
    }
  };
}

/** 代码雨：按列等距排布，每列一串随机字符 */
function matrixParticle(index: number, columns: number, scale: number): Particle {
  return {
    key: `matrix-${index}`,
    chars: matrixChars(),
    style: {
      "--pf-x": `${fix((((index % columns) + 0.5) / columns) * 100)}%`,
      "--pf-y": "-24vh",
      "--pf-size": px(rnd(10, 13) * scale),
      "--pf-dur": sec(rnd(7, 15)),
      "--pf-delay": negativeDelay(15),
      "--pf-dx": "0vw",
      "--pf-o": fix(rnd(0.38, 0.8), 2)
    }
  };
}

function buildParticles(
  effect: AmbientEffectId,
  density: AmbientDensity,
  width: number
): Particle[] {
  if (effect === "none") return [];

  const height = window.innerHeight;
  // 弹窗只有 360 宽，粒子按比例缩一点，否则一颗花瓣就占了半屏
  const sizeScale = Math.min(1, Math.max(0.8, width / 1280));
  const areaFactor = Math.min(1.25, Math.max(0.55, Math.sqrt((width * height) / (1280 * 800))));
  const factor = areaFactor * DENSITY_FACTOR[density];
  const list: Particle[] = [];

  if (effect === "matrix") {
    // 列宽固定 52px：弹窗约 7 列、主窗口 20 多列
    const columns = Math.max(4, Math.min(34, Math.round(width / 52)));
    const count = Math.max(4, Math.round(columns * 0.9 * DENSITY_FACTOR[density]));
    for (let i = 0; i < count; i += 1) {
      list.push(matrixParticle(i, columns, sizeScale));
    }
    return list;
  }

  const count = Math.max(6, Math.round(BASE_COUNT[effect] * factor));
  const maker: Record<Exclude<EffectId, "matrix">, (i: number, scale: number) => Particle> = {
    snow: snowParticle,
    petals: petalParticle,
    catkins: catkinParticle,
    leaves: leafParticle,
    fireflies: fireflyParticle,
    stardust: stardustParticle,
    spark: sparkParticle
  };

  for (let i = 0; i < count; i += 1) {
    list.push(maker[effect](i, sizeScale));
  }
  return list;
}

const state = ref<ThemeState>(loadThemeState());
const width = ref(window.innerWidth);

const effect = computed(() => resolveAmbientEffect(state.value));

const particles = computed(() =>
  buildParticles(effect.value, state.value.effectDensity, width.value)
);

let unsubscribe: (() => void) | null = null;
let resizeTimer: number | null = null;

function handleResize(): void {
  if (resizeTimer != null) window.clearTimeout(resizeTimer);
  // 防抖：拖动窗口时不要每帧重建整批粒子，否则动画会不停从头开始
  resizeTimer = window.setTimeout(() => {
    resizeTimer = null;
    if (Math.abs(window.innerWidth - width.value) > 8) {
      width.value = window.innerWidth;
    }
  }, 260);
}

onMounted(() => {
  unsubscribe = subscribeThemeApplied((next) => {
    state.value = next;
  });
  window.addEventListener("resize", handleResize);
});

onBeforeUnmount(() => {
  unsubscribe?.();
  window.removeEventListener("resize", handleResize);
  if (resizeTimer != null) window.clearTimeout(resizeTimer);
});
</script>

<template>
  <div class="skin-backdrop" aria-hidden="true"></div>
  <div
    class="ambient-layer"
    :data-effect="effect"
    :data-density="state.effectDensity"
    aria-hidden="true"
  >
    <div class="ambient-glow"></div>
    <div v-if="particles.length" class="ambient-field">
      <span
        v-for="particle in particles"
        :key="particle.key"
        class="ambient-particle"
        :style="particle.style"
      ><i><b>{{ particle.chars }}</b></i></span>
    </div>
  </div>
</template>

<style scoped>
/* 色晕强度随皮肤变化：有背景风景的皮肤会调低，避免与背景双层叠加过艳 */
.ambient-glow {
  background: var(--skin-glow, none);
  opacity: var(--skin-glow-opacity, 0.5);
  transition: opacity 0.3s ease;
}
</style>
