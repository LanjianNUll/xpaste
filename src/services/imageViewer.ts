/**
 * 全局图片预览器：命令式调用的单例入口。
 *
 * 用法（任意组件内）：
 *   import { openImageViewer } from "@/services/imageViewer";
 *   openImageViewer(imageSrc, "文件名");
 *
 * 实现方式：把预览器的可见状态与图片源放在这里，
 * 组件 ImageViewer 挂在根组件模板上并订阅这份状态，
 * 业务代码只管调用 open，不用关心组件挂在哪里。
 */
import { ref } from "vue";

/** 是否显示全屏预览 */
export const imageViewerVisible = ref(false);
/** 当前预览的图片地址（data URL 或相对路径） */
export const imageViewerSrc = ref("");
/** 当前预览的文件名，用于工具栏显示与下载命名 */
export const imageViewerName = ref("图片");

/** 打开全屏预览 */
export function openImageViewer(src: string, name?: string) {
  if (!src) return;
  imageViewerSrc.value = src;
  imageViewerName.value = name ?? "图片";
  imageViewerVisible.value = true;
}

/** 关闭全屏预览 */
export function closeImageViewer() {
  imageViewerVisible.value = false;
}
