import type { ClipboardItem, HistoryPage, HistoryPageQuery } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isEnabled, enable, disable } from "@tauri-apps/plugin-autostart";

export interface ClipboardSdk {
  fetchHistory(query?: string, limit?: number): Promise<ClipboardItem[]>;
  fetchHistoryByDate(startTs: number, endTs: number, query?: string, limit?: number): Promise<ClipboardItem[]>;
  fetchHistoryPage(query: HistoryPageQuery): Promise<HistoryPage>;
  getImage(id: number, thumbnail?: boolean): Promise<string>;
  saveText(text: string): Promise<void>;
  saveImage(imageBase64: string): Promise<void>;
  copyItem(id: number): Promise<void>;
  pasteItem(id: number): Promise<void>;
  deleteItem(id: number): Promise<void>;
  deleteItems(ids: number[]): Promise<number>;
  deleteByFormat(format: string): Promise<number>;
  deleteByCategory(category: string): Promise<number>;
  deleteByDate(startTs: number, endTs: number): Promise<number>;
  getFormatStats(): Promise<Array<[string, number]>>;
  getCategoryStats(): Promise<Array<[string, number]>>;
  clear(): Promise<void>;
  subscribe(listener: () => void): Promise<() => void>;
  hidePopup(): Promise<void>;
}

class TauriClipboardSdk implements ClipboardSdk {
  async fetchHistory(query = "", limit = 100): Promise<ClipboardItem[]> {
    if (query.trim().length === 0) {
      return await invoke<ClipboardItem[]>("list_history", { limit });
    }
    return await invoke<ClipboardItem[]>("search_history", { query, limit });
  }

  async fetchHistoryByDate(startTs: number, endTs: number, query = "", limit = 100): Promise<ClipboardItem[]> {
    if (query.trim().length === 0) {
      return await invoke<ClipboardItem[]>("list_history_by_date", { startTs, endTs, limit });
    }
    return await invoke<ClipboardItem[]>("search_history_by_date", { query, startTs, endTs, limit });
  }

  async fetchHistoryPage(query: HistoryPageQuery): Promise<HistoryPage> {
    return await invoke<HistoryPage>("get_history_page", { query });
  }

  async getImage(id: number, thumbnail = false): Promise<string> {
    return await invoke<string>("get_clipboard_image", { id, thumbnail });
  }

  async saveText(text: string): Promise<void> {
    await invoke<void>("save_clipboard_text", { text });
  }

  async saveImage(imageBase64: string): Promise<void> {
    await invoke<void>("save_clipboard_image", { imageBase64 });
  }

  async copyItem(id: number): Promise<void> {
    await invoke<void>("set_clipboard", { id });
  }

  async pasteItem(id: number): Promise<void> {
    await invoke<void>("set_clipboard_and_paste", { id });
  }

  async deleteItem(id: number): Promise<void> {
    await invoke<void>("delete_history_item", { id });
  }

  async deleteItems(ids: number[]): Promise<number> {
    return await invoke<number>("delete_history_items", { ids });
  }

  async deleteByFormat(format: string): Promise<number> {
    return await invoke<number>("delete_history_by_format", { format });
  }

  async deleteByCategory(category: string): Promise<number> {
    return await invoke<number>("delete_history_by_category", { category });
  }

  async deleteByDate(startTs: number, endTs: number): Promise<number> {
    return await invoke<number>("delete_history_by_date", { startTs, endTs });
  }

  async getFormatStats(): Promise<Array<[string, number]>> {
    return await invoke<Array<[string, number]>>("get_format_stats");
  }

  async getCategoryStats(): Promise<Array<[string, number]>> {
    return await invoke<Array<[string, number]>>("get_category_stats");
  }

  async clear(): Promise<void> {
    await invoke<void>("clear_history");
  }

  async subscribe(listener: () => void): Promise<() => void> {
    return await listen("clipboard://updated", listener);
  }

  async hidePopup(): Promise<void> {
    await invoke<void>("hide_popup");
  }
}

export const clipboardSdk: ClipboardSdk = new TauriClipboardSdk();

export const fetchHistory = clipboardSdk.fetchHistory.bind(clipboardSdk);
export const fetchHistoryByDate = clipboardSdk.fetchHistoryByDate.bind(clipboardSdk);
export const fetchHistoryPage = clipboardSdk.fetchHistoryPage.bind(clipboardSdk);
export const getClipboardImage = clipboardSdk.getImage.bind(clipboardSdk);
export const saveClipboardText = clipboardSdk.saveText.bind(clipboardSdk);
export const saveClipboardImage = clipboardSdk.saveImage.bind(clipboardSdk);
export const setClipboard = clipboardSdk.copyItem.bind(clipboardSdk);
export const setClipboardAndPaste = clipboardSdk.pasteItem.bind(clipboardSdk);
export const subscribeClipboardUpdates = clipboardSdk.subscribe.bind(clipboardSdk);
export const hidePopup = clipboardSdk.hidePopup.bind(clipboardSdk);

export async function getCursorPosition(): Promise<{ x: number; y: number }> {
  try {
    const [x, y] = await invoke<[number, number]>("get_cursor_position");
    return { x, y };
  } catch {
    return { x: 0, y: 0 };
  }
}

export async function getHotkey(): Promise<string> {
  try {
    return await invoke<string>("get_hotkey");
  } catch {
    return "Win+V";
  }
}

export async function setHotkey(hotkey: string): Promise<void> {
  try {
    await invoke<void>("set_hotkey", { hotkey });
  } catch (err) {
    throw err;
  }
}

export async function isAutostartEnabled(): Promise<boolean> {
  try {
    return await isEnabled();
  } catch {
    return false;
  }
}

export async function setAutostart(enabled: boolean): Promise<void> {
  try {
    if (enabled) {
      await enable();
    } else {
      await disable();
    }
  } catch (err) {
    throw err;
  }
}

export async function clearHistory(): Promise<void> {
  await clipboardSdk.clear();
}

export async function deleteHistoryItem(id: number): Promise<void> {
  await clipboardSdk.deleteItem(id);
}

export async function deleteHistoryItems(ids: number[]): Promise<number> {
  return await clipboardSdk.deleteItems(ids);
}

export async function deleteHistoryByFormat(format: string): Promise<number> {
  return await clipboardSdk.deleteByFormat(format);
}

export async function deleteHistoryByCategory(category: string): Promise<number> {
  return await clipboardSdk.deleteByCategory(category);
}

export async function deleteHistoryByDate(
  startTs: number,
  endTs: number
): Promise<number> {
  return await clipboardSdk.deleteByDate(startTs, endTs);
}

export async function getFormatStats(): Promise<Array<[string, number]>> {
  return await clipboardSdk.getFormatStats();
}

export async function getCategoryStats(): Promise<Array<[string, number]>> {
  return await clipboardSdk.getCategoryStats();
}
