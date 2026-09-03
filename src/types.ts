export type ClipboardFormat = "text" | "image" | "html" | "file" | "color";
export type ClipboardCategory = "link" | "image" | "text" | "file";

export type DateRangeType = "all" | "today" | "yesterday" | "beforeYesterday" | "custom" | "customRange";

export interface DateRange {
  startTs: number;
  endTs: number;
}

export interface ClipboardItem {
  id: number;
  format: ClipboardFormat;
  category: ClipboardCategory;
  text?: string | null;
  html?: string | null;
  filePath?: string | null;
  color?: string | null;
  imageWidth?: number | null;
  imageHeight?: number | null;
  createdAt: number;
  copyCount: number;
}

export interface HistoryPage {
  items: ClipboardItem[];
  total: number;
}

export interface HistoryPageQuery {
  startTs?: number;
  endTs?: number;
  keyword?: string;
  formats?: string[];
  categories?: string[];
  page: number;
  pageSize: number;
}
