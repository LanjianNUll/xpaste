export type ClipboardFormat = "text" | "image" | "html" | "file" | "color";
export type ClipboardCategory = "link" | "image" | "text" | "file" | "folder";

export type DateRangeType = "all" | "today" | "yesterday" | "beforeYesterday" | "custom" | "customRange";

export interface DateRange {
  startTs: number;
  endTs: number;
}

/** 条目数据来源：历史记录或收藏快照，决定后端按哪个表读写完整内容。 */
export type ItemSource = "history" | "favorite";

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
  /** 历史条目=是否已收藏；收藏列表条目恒为 true。 */
  favorited: boolean;
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
