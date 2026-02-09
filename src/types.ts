export type ClipboardFormat = "text" | "image" | "html" | "file" | "color";
export type ClipboardCategory = "link" | "image" | "text" | "file";

export interface ClipboardItem {
  id: number;
  format: ClipboardFormat;
  category: ClipboardCategory;
  text?: string | null;
  html?: string | null;
  filePath?: string | null;
  color?: string | null;
  imageBase64?: string | null;
  imageWidth?: number | null;
  imageHeight?: number | null;
  createdAt: number;
}
