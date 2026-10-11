import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import PopupWindow from "./PopupWindow.vue";
import "./styles/theme.css";
import "./styles/app.css";
import { initTheme } from "./services/theme";

const app = createApp(PopupWindow);
app.use(ElementPlus);
app.mount("#app");

// 快捷窗口与主窗口共享 localStorage 中的主题状态；
// 主窗口切换主题时通过 theme://changed 事件即时同步到这里。
void initTheme();
