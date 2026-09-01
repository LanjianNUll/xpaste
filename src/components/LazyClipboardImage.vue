<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { getClipboardImage } from "@/services/api";

const props = defineProps<{
  itemId: number;
  alt?: string;
}>();

const imageRef = ref<HTMLImageElement | null>(null);
const imageUrl = ref("");
let observer: IntersectionObserver | null = null;
let releaseTimer: number | null = null;
let requestVersion = 0;

/** 仅在图片进入视口时读取数据库，离开视口后主动释放 Base64 内存。 */
async function loadImage() {
  if (imageUrl.value) return;
  const currentVersion = ++requestVersion;
  try {
    const data = await getClipboardImage(props.itemId, true);
    if (currentVersion === requestVersion) {
      imageUrl.value = `data:image/png;base64,${data}`;
    }
  } catch {
    imageUrl.value = "";
  }
}

function scheduleRelease() {
  if (releaseTimer != null) window.clearTimeout(releaseTimer);
  releaseTimer = window.setTimeout(() => {
    requestVersion += 1;
    imageUrl.value = "";
  }, 1000);
}

onMounted(() => {
  observer = new IntersectionObserver(([entry]) => {
    if (entry.isIntersecting) {
      if (releaseTimer != null) window.clearTimeout(releaseTimer);
      loadImage();
    } else {
      scheduleRelease();
    }
  }, { rootMargin: "80px" });
  if (imageRef.value) observer.observe(imageRef.value);
});

onBeforeUnmount(() => {
  requestVersion += 1;
  observer?.disconnect();
  if (releaseTimer != null) window.clearTimeout(releaseTimer);
  imageUrl.value = "";
});
</script>

<template>
  <img ref="imageRef" :src="imageUrl" :alt="alt ?? '剪贴板图片'" loading="lazy" />
</template>
