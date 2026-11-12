import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import EditorWindow from "./components/EditorWindow.vue";
import "./styles/theme.css";
import "./styles/app.css";
import { initTheme } from "./services/theme";

const app = createApp(EditorWindow);
app.use(ElementPlus);
app.mount("#app");

// 与其他窗口共享同一份主题状态，主窗口切换主题后即时跟随
void initTheme();
