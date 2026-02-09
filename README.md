# Xpaste

<div align="center">

一款现代化的跨平台剪贴板历史管理工具，让你的剪贴板更智能、更高效。

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)
![Vue](https://img.shields.io/badge/Vue-3.x-green.svg)

[功能特性](#功能特性) • [快速开始](#快速开始) • [使用说明](#使用说明) • [技术栈](#技术栈) • [贡献指南](#贡献指南)

</div>

## 📸 预览

- **主窗口**：完整的剪贴板历史管理界面
- **快捷窗口**：轻量级快速访问面板（Alt+V 唤起）
- **系统托盘**：后台运行，随时可用

## ✨ 功能特性

### 核心功能

- 🎯 **实时监控** - 自动捕获所有剪贴板内容变化
- 📋 **多格式支持** - 文本、图片、HTML、文件路径等
- 🔍 **智能搜索** - 快速查找历史记录
- 🏷️ **自动分类** - 智能识别链接、图片、文本、文件
- 📅 **按日期筛选** - 今天/昨天/前天/自定义日期查看

### 用户体验

- ⚡ **全局快捷键** - 默认 Alt+V，可自定义
- 🖱️ **一键粘贴** - 点击历史记录直接插入到当前输入框
- 🖼️ **图片缩略图** - 直观预览图片内容
- 💾 **持久化存储** - SQLite 本地数据库
- 🎨 **现代化 UI** - 基于 Element Plus 的美观界面

### 窗口管理

- 🪟 **跟随光标** - 快捷窗口智能定位，支持多显示器
- 🔒 **不抢焦点** - 快捷窗口不影响当前输入框
- 🎯 **自动粘贴** - 模拟 Ctrl+V，无缝插入内容
- 🌐 **后台运行** - 系统托盘模式，不占用任务栏

## 🚀 快速开始

### 前置要求

- Node.js 16+ 
- Rust 1.70+
- pnpm (推荐) 或 npm

### 安装依赖

```bash
# 安装前端依赖
pnpm install

# 安装 Tauri CLI（如果未安装）
pnpm add -D @tauri-apps/cli
```

### 开发运行

```bash
# 启动开发服务器
pnpm tauri dev
```

### 构建发布

```bash
# 构建生产版本
pnpm tauri build
```

构建完成后，可执行文件位于 `src-tauri/target/release/` 目录。

## 📖 使用说明

### 基本操作

1. **启动应用** - 应用将在后台运行，系统托盘显示图标
2. **查看历史** - 点击托盘图标打开主窗口
3. **快速访问** - 按 Alt+V 在光标位置弹出快捷窗口
4. **插入内容** - 点击任意历史记录，自动粘贴到当前输入框

### 快捷键

- **Alt+V** - 唤起快捷窗口（可自定义）
- **Ctrl+V** - 自动触发（由应用模拟）

### 自定义快捷键

1. 打开主窗口
2. 点击 "设置快捷键" 按钮
3. 输入新快捷键（格式：Alt+V, Ctrl+Shift+C 等）
4. 重启应用生效

支持的修饰键：`Ctrl`, `Alt`, `Shift`, `Win`  
支持的按键：`A-Z` 字母键

### 日期筛选

- **今天** - 显示今日的剪贴板记录
- **昨天** - 显示昨天的记录
- **前天** - 显示前天的记录
- **自定义** - 选择任意日期查看

### 托盘菜单

- **左键点击** - 显示主窗口
- **右键菜单**
  - 显示主窗口
  - 退出应用

## 🛠️ 技术栈

### 前端

- **框架**: Vue 3 + TypeScript
- **UI 库**: Element Plus
- **构建工具**: Vite
- **代码高亮**: highlight.js

### 后端

- **框架**: Tauri 2.0
- **语言**: Rust
- **数据库**: SQLite (sqlx)
- **剪贴板**: arboard
- **图片处理**: image

### 核心特性

- 全局快捷键监听 (tauri-plugin-global-shortcut)
- Windows API 集成 (光标位置、窗口定位、键盘模拟)
- 多显示器支持
- 异步运行时 (tokio)

## 📁 项目结构

```
paste_app/
├── src/                    # 前端源码
│   ├── services/          # API 服务
│   ├── styles/            # 样式文件
│   ├── App.vue            # 主窗口组件
│   ├── PopupWindow.vue    # 快捷窗口组件
│   ├── main.ts            # 主窗口入口
│   ├── popup.ts           # 快捷窗口入口
│   └── types.ts           # TypeScript 类型定义
├── src-tauri/             # Tauri 后端
│   ├── src/
│   │   ├── main.rs        # 主入口和窗口管理
│   │   ├── clipboard.rs   # 剪贴板监控
│   │   ├── db.rs          # 数据库操作
│   │   ├── models.rs      # 数据模型
│   │   └── classify.rs    # 内容分类
│   ├── capabilities/      # 权限配置
│   └── icons/             # 应用图标
├── index.html             # 主窗口 HTML
├── popup.html             # 快捷窗口 HTML
└── package.json
```

## 🤝 贡献指南

欢迎贡献代码、报告问题或提出新功能建议！

### 开发流程

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 提交 Pull Request

### 代码规范

- 前端：遵循 Vue 3 Composition API 规范
- 后端：遵循 Rust 标准代码风格
- 提交信息：使用清晰的提交信息描述更改

## 🐛 问题反馈

如果您遇到任何问题或有功能建议，请在 [Issues](https://github.com/yourusername/xpaste/issues) 页面提交。

提交 Issue 时请包含：
- 操作系统版本
- 应用版本
- 详细的问题描述
- 复现步骤
- 相关截图（如有）

## 📝 更新日志

### v0.0.1 (当前版本)

- ✨ 初始版本发布
- ✅ 剪贴板历史监控
- ✅ 多格式内容支持
- ✅ 全局快捷键
- ✅ 智能窗口定位
- ✅ 系统托盘集成
- ✅ 按日期筛选
- ✅ 可自定义快捷键

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

感谢以下开源项目：

- [Tauri](https://tauri.app/) - 构建桌面应用的最佳框架
- [Vue.js](https://vuejs.org/) - 渐进式 JavaScript 框架
- [Element Plus](https://element-plus.org/) - Vue 3 组件库
- [arboard](https://github.com/1Password/arboard) - 跨平台剪贴板库

## 🌟 Star History

如果这个项目对你有帮助，请给我们一个 ⭐️

---

<div align="center">

Made with ❤️ by the Xpaste

[报告问题](https://github.com/LanjianNUll/xpaste/issues) • [请求功能](https://github.com/LanjianNUll/xpaste/issues/new?labels=enhancement)

</div>
