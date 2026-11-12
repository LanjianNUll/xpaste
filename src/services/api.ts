import type { ClipboardItem, HistoryPage, HistoryPageQuery, ItemSource } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { isEnabled, enable, disable } from "@tauri-apps/plugin-autostart";
import { defaultHotkey } from "@/services/platform";

export interface ClipboardSdk {
  fetchHistory(query?: string, limit?: number): Promise<ClipboardItem[]>;
  fetchHistoryByDate(startTs: number, endTs: number, query?: string, limit?: number): Promise<ClipboardItem[]>;
  fetchHistoryPage(query: HistoryPageQuery): Promise<HistoryPage>;
  fetchFavorites(query?: string, format?: string, limit?: number): Promise<ClipboardItem[]>;
  getImage(id: number, thumbnail?: boolean, source?: ItemSource): Promise<string>;
  saveText(text: string): Promise<void>;
  saveImage(imageBase64: string): Promise<void>;
  copyItem(id: number, source?: ItemSource): Promise<void>;
  pasteItem(id: number, source?: ItemSource): Promise<void>;
  /** 把任意文本投递到原应用（编辑器送出）。 */
  pasteText(text: string): Promise<void>;
  /** 只把任意文本写进剪贴板，不模拟按键。 */
  copyText(text: string): Promise<void>;
  /** 取一条记录的完整文本（列表返回的是 4096 字符截断版）。 */
  getItemText(id: number, source?: ItemSource): Promise<string>;
  /** 打开编辑器窗口（贴在快捷面板右侧，面板本身不动）。 */
  openEditor(): Promise<void>;
  /** 关闭编辑器窗口，焦点还给快捷面板。 */
  closeEditor(): Promise<void>;
  /** 展开/收起编辑器右侧的历史栏（窗口宽度跟着变，左上角不动）。 */
  setEditorSidebar(open: boolean): Promise<void>;
  /** 编辑器是否开着——面板失焦时据此判断该不该隐藏自己。 */
  isEditorVisible(): Promise<boolean>;
  /** 最前面的窗口是否属于本应用（用来区分"焦点去了编辑器"和"切到别的应用"）。 */
  isOwnWindowFrontmost(): Promise<boolean>;
  /** 监听编辑器打开事件（用于把焦点放回编辑区末尾）。 */
  subscribeEditorOpened(listener: () => void): Promise<() => void>;
  /** 监听「插入素材」事件：面板点了条目，编辑器把它插到光标处。 */
  subscribeEditorInsert(listener: (text: string) => void): Promise<() => void>;
  /** 把一段素材送到编辑器光标处（编辑器窗口监听 editor://insert）。 */
  insertIntoEditor(text: string): Promise<void>;
  /** 打开编辑器时把「当前选中的那一条」作为默认内容送过去。 */
  seedEditor(text: string): Promise<void>;
  /** 监听「默认内容」事件：编辑器只在草稿为空时采用，避免覆盖用户正在写的东西。 */
  subscribeEditorSeed(listener: (text: string) => void): Promise<() => void>;
  toggleFavorite(id: number): Promise<boolean>;
  removeFavorite(id: number): Promise<void>;
  deleteItem(id: number): Promise<void>;
  deleteItems(ids: number[]): Promise<number>;
  deleteByFormat(format: string): Promise<number>;
  deleteByCategory(category: string): Promise<number>;
  deleteByDate(startTs: number, endTs: number): Promise<number>;
  getFormatStats(): Promise<Array<[string, number]>>;
  getCategoryStats(): Promise<Array<[string, number]>>;
  clear(): Promise<void>;
  subscribe(listener: () => void): Promise<() => void>;
  subscribePopupShown(listener: () => void): Promise<() => void>;
  hidePopup(): Promise<void>;
  /** 当前是否已授予 macOS「辅助功能」权限（非 macOS 恒为 true）。 */
  hasAccessibility(): Promise<boolean>;
  /** 主动申请「辅助功能」权限，会触发系统授权引导，返回此刻的状态。 */
  requestAccessibility(): Promise<boolean>;
  /** 一键修复授权：清除旧记录 → 重新申请 → 打开系统设置。 */
  repairAccessibility(): Promise<{ trusted: boolean; message: string }>;
  /** 打开「系统设置 → 隐私与安全性 → 辅助功能」。 */
  openAccessibilitySettings(): Promise<void>;
  /** Dock 图标是否可见。 */
  isDockVisible(): Promise<boolean>;
  /** 切换 Dock 图标可见性，立即生效。 */
  setDockVisible(visible: boolean): Promise<void>;
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

  async fetchFavorites(query = "", format?: string, limit = 200): Promise<ClipboardItem[]> {
    return await invoke<ClipboardItem[]>("list_favorites", { query, format, limit });
  }

  async getImage(id: number, thumbnail = false, source?: ItemSource): Promise<string> {
    return await invoke<string>("get_clipboard_image", { id, thumbnail, source });
  }

  async saveText(text: string): Promise<void> {
    await invoke<void>("save_clipboard_text", { text });
  }

  async saveImage(imageBase64: string): Promise<void> {
    await invoke<void>("save_clipboard_image", { imageBase64 });
  }

  async copyItem(id: number, source?: ItemSource): Promise<void> {
    await invoke<void>("set_clipboard", { id, source });
  }

  async pasteItem(id: number, source?: ItemSource): Promise<void> {
    await invoke<void>("set_clipboard_and_paste", { id, source });
  }

  async pasteText(text: string): Promise<void> {
    await invoke<void>("paste_text", { text });
  }

  async copyText(text: string): Promise<void> {
    await invoke<void>("copy_text", { text });
  }

  async getItemText(id: number, source?: ItemSource): Promise<string> {
    return await invoke<string>("get_item_text", { id, source });
  }

  async openEditor(): Promise<void> {
    await invoke<void>("open_editor");
  }

  async closeEditor(): Promise<void> {
    await invoke<void>("close_editor");
  }

  async setEditorSidebar(open: boolean): Promise<void> {
    await invoke<void>("set_editor_sidebar", { open });
  }

  async isEditorVisible(): Promise<boolean> {
    try {
      return await invoke<boolean>("is_editor_visible");
    } catch {
      // 查询失败时按"没开"处理：宁可面板正常隐藏，也不要一直挂在屏幕上
      return false;
    }
  }

  async isOwnWindowFrontmost(): Promise<boolean> {
    try {
      return await invoke<boolean>("is_own_window_frontmost");
    } catch {
      // 查不到就按"不是自己"处理：宁可多收起一次，也别把面板挂在屏幕上
      return false;
    }
  }

  async subscribeEditorOpened(listener: () => void): Promise<() => void> {
    return await listen("editor://opened", () => listener());
  }

  async subscribeEditorInsert(listener: (text: string) => void): Promise<() => void> {
    return await listen<string>("editor://insert", (event) => listener(event.payload));
  }

  async insertIntoEditor(text: string): Promise<void> {
    // 广播（Any）而不是 emitTo：编辑器窗口标签硬编码在事件名里没有好处，
    // 而 emit 会把事件发给所有 webview，编辑器那边只订阅这一个名字。
    await emit("editor://insert", text);
  }

  async seedEditor(text: string): Promise<void> {
    await emit("editor://seed", text);
  }

  async subscribeEditorSeed(listener: (text: string) => void): Promise<() => void> {
    return await listen<string>("editor://seed", (event) => listener(event.payload));
  }

  async toggleFavorite(id: number): Promise<boolean> {
    return await invoke<boolean>("toggle_favorite", { id });
  }

  async removeFavorite(id: number): Promise<void> {
    await invoke<void>("remove_favorite", { id });
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

  /**
   * 快捷窗口被显示（切换为可见并取得焦点）时触发。Rust 侧用广播 emit 投递，
   * 因为前端 listen 默认注册为 Any 目标，emit_to 指定标签不会命中。
   */
  async subscribePopupShown(listener: () => void): Promise<() => void> {
    return await listen("popup://shown", listener);
  }

  async hidePopup(): Promise<void> {
    await invoke<void>("hide_popup");
  }

  async hasAccessibility(): Promise<boolean> {
    return await invoke<boolean>("check_accessibility_permission");
  }

  async requestAccessibility(): Promise<boolean> {
    return await invoke<boolean>("request_accessibility_permission");
  }

  async repairAccessibility(): Promise<{ trusted: boolean; message: string }> {
    return await invoke<{ trusted: boolean; message: string }>(
      "repair_accessibility_permission"
    );
  }

  async openAccessibilitySettings(): Promise<void> {
    await invoke<void>("open_accessibility_settings");
  }

  async isDockVisible(): Promise<boolean> {
    return await invoke<boolean>("get_dock_visible");
  }

  async setDockVisible(visible: boolean): Promise<void> {
    await invoke<void>("set_dock_visible", { visible });
  }
}

export const clipboardSdk: ClipboardSdk = new TauriClipboardSdk();

export const fetchHistory = clipboardSdk.fetchHistory.bind(clipboardSdk);
export const fetchHistoryByDate = clipboardSdk.fetchHistoryByDate.bind(clipboardSdk);
export const fetchHistoryPage = clipboardSdk.fetchHistoryPage.bind(clipboardSdk);
export const fetchFavorites = clipboardSdk.fetchFavorites.bind(clipboardSdk);
export const getClipboardImage = clipboardSdk.getImage.bind(clipboardSdk);
export const saveClipboardText = clipboardSdk.saveText.bind(clipboardSdk);
export const saveClipboardImage = clipboardSdk.saveImage.bind(clipboardSdk);
export const setClipboard = clipboardSdk.copyItem.bind(clipboardSdk);
export const setClipboardAndPaste = clipboardSdk.pasteItem.bind(clipboardSdk);
export const pasteText = clipboardSdk.pasteText.bind(clipboardSdk);
export const copyText = clipboardSdk.copyText.bind(clipboardSdk);
export const getItemText = clipboardSdk.getItemText.bind(clipboardSdk);
export const openEditor = clipboardSdk.openEditor.bind(clipboardSdk);
export const closeEditor = clipboardSdk.closeEditor.bind(clipboardSdk);
export const setEditorSidebar = clipboardSdk.setEditorSidebar.bind(clipboardSdk);
export const isEditorVisible = clipboardSdk.isEditorVisible.bind(clipboardSdk);
export const isOwnWindowFrontmost = clipboardSdk.isOwnWindowFrontmost.bind(clipboardSdk);
export const subscribeEditorOpened = clipboardSdk.subscribeEditorOpened.bind(clipboardSdk);
export const subscribeEditorInsert = clipboardSdk.subscribeEditorInsert.bind(clipboardSdk);
export const insertIntoEditor = clipboardSdk.insertIntoEditor.bind(clipboardSdk);
export const seedEditor = clipboardSdk.seedEditor.bind(clipboardSdk);
export const subscribeEditorSeed = clipboardSdk.subscribeEditorSeed.bind(clipboardSdk);
export const toggleFavorite = clipboardSdk.toggleFavorite.bind(clipboardSdk);
export const removeFavorite = clipboardSdk.removeFavorite.bind(clipboardSdk);
export const subscribeClipboardUpdates = clipboardSdk.subscribe.bind(clipboardSdk);
export const subscribePopupShown = clipboardSdk.subscribePopupShown.bind(clipboardSdk);
export const hidePopup = clipboardSdk.hidePopup.bind(clipboardSdk);

/**
 * 快捷窗口场景下选中条目：仅当快捷窗口当前可见时才写入剪贴板并模拟 Ctrl+V。
 * 若窗口已隐藏（例如从主窗口调用），则退化为仅写入剪贴板，避免误粘贴到其他应用。
 */
export async function pasteHistoryItem(id: number, source?: ItemSource): Promise<void> {
  let visible = true;
  try {
    const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    visible = await getCurrentWebviewWindow().isVisible();
  } catch {
    visible = true;
  }
  if (visible) {
    await clipboardSdk.pasteItem(id, source);
  } else {
    await clipboardSdk.copyItem(id, source);
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

export async function getHotkey(): Promise<string> {
  try {
    return await invoke<string>("get_hotkey");
  } catch {
    // 拿不到后端配置时（例如浏览器里预览）回落到当前平台的默认值
    return defaultHotkey;
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

/** 是否已授予「辅助功能」权限。查询失败时按"无权限"处理，交给界面引导授权。 */
export async function hasAccessibilityPermission(): Promise<boolean> {
  try {
    return await clipboardSdk.hasAccessibility();
  } catch {
    return false;
  }
}

/** 触发系统授权引导并返回当前状态。 */
export async function requestAccessibilityPermission(): Promise<boolean> {
  try {
    return await clipboardSdk.requestAccessibility();
  } catch {
    return false;
  }
}

/**
 * 一键修复「辅助功能」授权。
 *
 * 内部会清掉 TCC 里绑着旧签名的失效记录，再用当前二进制重新申请一次。
 * 只靠让用户去系统设置里拨开关是修不好的：那条记录永远匹配不上新签名。
 */
export async function repairAccessibilityPermission(): Promise<{
  trusted: boolean;
  message: string;
}> {
  return await clipboardSdk.repairAccessibility();
}

/** 跳到「系统设置 → 隐私与安全性 → 辅助功能」。 */
export async function openAccessibilitySettings(): Promise<void> {
  await clipboardSdk.openAccessibilitySettings();
}

export async function isDockVisible(): Promise<boolean> {
  try {
    return await clipboardSdk.isDockVisible();
  } catch {
    // 非 macOS 或调用失败时按可见处理，避免界面开关与实际状态错位
    return true;
  }
}

export async function setDockVisible(visible: boolean): Promise<void> {
  await clipboardSdk.setDockVisible(visible);
}
