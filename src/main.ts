import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import "highlight.js/styles/github-dark.css";
import App from "./App.vue";
import "./styles/theme.css";
import "./styles/app.css";
import { initTheme } from "./services/theme";

const app = createApp(App);
app.use(ElementPlus);
app.mount("#app");

// 主题在挂载后初始化：入口 html 中的内联脚本已提前应用一次，这里负责
// 跟随系统深浅色变化、跨窗口同步，并同步原生窗口标题栏颜色。
void initTheme();
