import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import "highlight.js/styles/github-dark.css";
import App from "./App.vue";
import "./styles/theme.css";
import "./styles/app.css";
import { initTheme } from "./services/theme";
import { isMacOS, isWindows } from "./services/platform";

// 把宿主平台挂到 <html> 上，样式表据此切换窗口骨架：
// macOS 用系统原生标题栏（红绿灯由窗口服务器绘制、窗口自带圆角与投影），
// 内容需要通栏鋪到窗口边缘；Windows 仍是自绘标题栏 + 卡片式留白。
document.documentElement.dataset.os = isMacOS ? "mac" : isWindows ? "win" : "other";

const app = createApp(App);
app.use(ElementPlus);
app.mount("#app");

// 主题在挂载后初始化：入口 html 中的内联脚本已提前应用一次，这里负责
// 跟随系统深浅色变化、跨窗口同步，并同步原生窗口标题栏颜色。
void initTheme();
