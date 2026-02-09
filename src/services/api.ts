import type { ClipboardItem } from "@/types";
import { invoke } from "@tauri-apps/api/core";

export async function fetchHistory(query = "", limit = 200): Promise<ClipboardItem[]> {
  try {
    if (query.trim().length === 0) {
      return await invoke<ClipboardItem[]>("list_history", { limit });
    }
    return await invoke<ClipboardItem[]>("search_history", { query, limit });
  } catch {
    return mockHistory();
  }
}

export async function fetchHistoryByDate(
  startTs: number,
  endTs: number,
  query = "",
  limit = 200
): Promise<ClipboardItem[]> {
  try {
    if (query.trim().length === 0) {
      return await invoke<ClipboardItem[]>("list_history_by_date", { startTs, endTs, limit });
    }
    return await invoke<ClipboardItem[]>("search_history_by_date", { query, startTs, endTs, limit });
  } catch {
    return mockHistory();
  }
}

export async function setClipboard(id: number): Promise<void> {
  try {
    await invoke<void>("set_clipboard", { id });
  } catch {
    // ignore in web context
  }
}

export async function setClipboardAndPaste(id: number): Promise<void> {
  try {
    await invoke<void>("set_clipboard_and_paste", { id });
  } catch {
    // ignore in web context
  }
}

export async function getCursorPosition(): Promise<{ x: number; y: number }> {
  try {
    const [x, y] = await invoke<[number, number]>("get_cursor_position");
    return { x, y };
  } catch {
    return { x: 0, y: 0 };
  }
}

function mockHistory(): ClipboardItem[] {
  const now = Date.now();
  return [
    {
      id: 1,
      format: "text",
      category: "text",
      text: "欢迎使用 Paste App：这里会显示剪贴板历史。",
      createdAt: now - 1000 * 60
    },
    {
      id: 2,
      format: "text",
      category: "link",
      text: "https://tauri.app",
      createdAt: now - 1000 * 120
    },
    {
      id: 3,
      format: "color",
      category: "text",
      color: "#2f80ed",
      text: "#2f80ed",
      createdAt: now - 1000 * 180
    }
  ];
}
