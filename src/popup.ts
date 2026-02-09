import { createApp } from "vue";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import PopupWindow from "./PopupWindow.vue";
import "./styles/app.css";

const app = createApp(PopupWindow);
app.use(ElementPlus);
app.mount("#app");
