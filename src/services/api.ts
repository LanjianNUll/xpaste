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

export async function setClipboard(id: number): Promise<void> {
  try {
    await invoke<void>("set_clipboard", { id });
  } catch {
    // ignore in web context
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
