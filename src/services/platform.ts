/**
 * 平台判定与平台相关文案。
 *
 * Tauri 的 WebView 里 `navigator.userAgent` 会带上宿主系统信息：
 * macOS 是 `Macintosh`，Windows 是 `Windows NT`。这里只做这一点轻量判断，
 * 不引入 @tauri-apps/plugin-os，避免为一个布尔值多装一个原生插件。
 */

const userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent;

/** 排除 iPad 上的 Safari（UA 也含 Mac），避免误判成桌面 macOS。 */
export const isMacOS =
  /Macintosh|Mac OS X/i.test(userAgent) && !/iPhone|iPad|iPod/i.test(userAgent);

export const isWindows = /Windows NT/i.test(userAgent);

/**
 * 当前平台的默认全局快捷键。
 * 必须与 Rust 侧 `HotkeySpec::default_for_platform` 保持一致——
 * 这里只在拿不到后端配置时兜底（例如浏览器里预览页面）。
 */
export const defaultHotkey = isMacOS ? "Cmd+Shift+V" : isWindows ? "Win+V" : "Ctrl+Shift+V";

/** 设置面板里提示用户可用的修饰键写法。 */
export const modifierHint = isMacOS
  ? "Cmd（⌘）、Option、Control、Shift"
  : "Ctrl、Alt、Shift、Win";

/** 输入框里的示例文案。 */
export const hotkeyPlaceholder = isMacOS
  ? "例如：Cmd+Shift+V, Option+V"
  : "例如：Win+V, Ctrl+Shift+V";

/** 快捷窗口里"粘贴"这个动作对应的按键提示。 */
export const pasteKeyLabel = isMacOS ? "⌘V" : "Ctrl+V";
