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
- **快捷窗口**：轻量级快速访问面板（Win+V 唤起）
- **系统托盘**：后台运行，随时可用

## ✨ 功能特性

### 核心功能

- 🎯 **实时监控** - 自动捕获所有剪贴板内容变化
- 📋 **多格式支持** - 文本、图片、HTML、文件路径等
- 🔗 **链接识别** - 自动识别 http://、https://、www. 开头的链接
- 🔍 **智能搜索** - 快速查找历史记录
- 🏷️ **自动分类** - 智能识别链接、图片、文本、文件
- 📅 **按日期筛选** - 今天/昨天/前天/自定义日期范围/全部时间查看

### 历史管理

- 📊 **历史记录** - 完整的历史管理页面，支持分页浏览
- 🔍 **多维度筛选** - 时间范围、内容类型、分类、关键字搜索
- 🗑️ **灵活删除** - 单条删除、批量删除、按类型删除、清空全部
- 📈 **统计信息** - 实时显示各类型记录数量统计
- 🔗 **链接预览** - 点击链接直接在浏览器中打开
- 🖼️ **图片预览** - 点击图片放大查看原图

### 用户体验

- ⚡ **全局快捷键** - 默认 Win+V，可自定义
- 🖱️ **一键粘贴** - 点击历史记录直接插入到当前输入框
- 🖼️ **图片缩略图** - 直观预览图片内容
- 💾 **持久化存储** - SQLite 本地数据库
- 🎨 **现代化 UI** - 基于 Element Plus 的美观界面

### 主题与皮肤

- 🌗 **浅色 / 深色模式** - 一键切换，或跟随系统深浅色自动变化
- 🎨 **内置皮肤** - 经典、春日、盛夏、秋叶、冬雪、樱花（二次元）、极客、紫罗兰、中国红、中国风、科技
- 🖼️ **专属背景** - 每套皮肤自带一幅主题背景：春日花海、盛夏海湾、秋日林道、雪后森林、樱花、远山云雾…（也可切换成纯 CSS 渐变风景）
- 🌫️ **磨砂面板** - 面板半透明，背景从界面后面透出来，文字依然清晰；深浅色各有一套曝光与雾度
- ✨ **背景动效** - 柳絮、落叶、飞雪、星尘、光点上升、代码雨等粒子随皮肤切换，密度可调
- ✏️ **自定义皮肤** - 自选主题色与深浅背景调，也可以直接用自己的图片当背景，实时预览、即时生效
- 🪟 **全窗口同步** - 主窗口与快捷窗口主题实时联动，切换即时生效
- 🚫 **无闪烁启动** - 窗口打开前先恢复上次主题，避免白色闪烁

### 自动更新

- 🔄 **启动自动检查** - 每次启动检查远端更新清单，有新版本自动下载
- 📦 **一键更新** - 下载完成后提示「立即重启安装」，安装完自动重新启动
- 📝 **带更新日志** - 更新提示里直接显示这个版本改了什么
- 🔐 **签名校验** - 更新包必须通过 minisign 签名校验才会被安装

### 窗口管理

- 🖼️ **无边框标题栏** - 主窗口与快捷窗口都不再使用系统边框，顶部直接融入皮肤背景；
  拖动、双击最大化、最小化 / 最大化 / 关闭都由应用自绘（关闭按钮与系统行为一致，隐藏到托盘）
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

### 发布新版本（自动）

版本号记录在 4 个文件里，用脚本一次改好，避免 CI 因版本不一致拒绝发版：

```bash
pnpm version:bump 2.0.8      # 同时更新 package.json / tauri.conf.json / Cargo.toml / Cargo.lock
git commit -am "chore: release v2.0.8"
git push
```

推送后 [`.github/workflows/release.yml`](.github/workflows/release.yml) 会自动：

1. 校验三处版本号一致，且该版本还没发过（只改代码没改版本号的提交会直接跳过）；
2. 在 `windows-latest` 上构建 Windows 安装包（NSIS `.exe`）并打包便携版 zip；
3. 创建 Release，**按两个版本之间的提交/PR 自动生成更新日志**，然后正式发布。

产物包括：

- `Xpaste_<版本>_x64-setup.exe` —— 安装包
- `Xpaste_v<版本>_portable_win_x64.zip` —— 便携版（解压即用）

也可以不用脚本，直接打 tag 触发（tag 名必须和代码里的版本号一致）：

```bash
git tag v2.0.8 && git push origin v2.0.8
```

或在 GitHub 的 **Actions → Release → Run workflow** 手动触发，勾选 `force` 可强制重发同一版本。

## 📖 使用说明

### 基本操作

1. **启动应用** - 应用将在后台运行，系统托盘显示图标
2. **查看历史** - 点击托盘图标打开主窗口
3. **快速访问** - 按 Win+V 在光标位置弹出快捷窗口
4. **插入内容** - 点击任意历史记录，自动粘贴到当前输入框

### 快捷键

- **Win+V** - 覆盖 Windows 原生剪贴板历史并唤起快捷窗口（可自定义）
- **Ctrl+V** - 自动触发（由应用模拟）

### 自定义快捷键

1. 打开主窗口
2. 点击 "设置快捷键" 按钮
3. 输入新快捷键（格式：Win+V, Ctrl+Shift+C 等）
4. 重启应用生效

支持的修饰键：`Ctrl`, `Alt`, `Shift`, `Win`  
支持的按键：`A-Z` 字母键

### 日期筛选

- **全部时间** - 显示所有历史记录
- **今天** - 显示今日的剪贴板记录
- **昨天** - 显示昨天的记录
- **前天** - 显示前天的记录
- **自定义范围** - 选择任意日期范围查看

### 内容分类

- 🔗 **链接** - http://、https://、www. 开头的网址（点击可直接打开）
- 📝 **文本** - 普通文本内容
- 🖼️ **图片** - 剪贴板中的图片（点击可放大预览）
- 📁 **文件** - 文件路径

### 历史管理页面

- **筛选功能**
  - 时间范围：全部/今天/昨天/前天/自定义范围
  - 分类筛选：链接/文本/图片/文件（支持多选）
  - 关键字搜索：实时搜索内容
  
- **批量操作**
  - 批量删除：选中多条记录一次性删除
  - 按类型删除：删除所有链接/文本/图片/文件
  - 清空全部：删除所有历史记录
  
- **统计信息**
  - 实时显示各类型的记录数量
  - 链接类型以绿色标签突出显示

### 主题与皮肤设置

1. 打开主窗口，点击工具栏的 ☀️/🌙 按钮快速切换深浅色
2. 点击"设置"打开设置对话框，顶部即为外观设置：
   - **主题模式**：浅色 / 深色 / 跟随系统
   - **皮肤**：点击宫格即换肤，卡片缩略图就是该皮肤的背景
   - **背景动效**：粒子总开关与低 / 中 / 高三档密度
   - **背景画法**：在「实景照片」与「简约渐变」之间切换
     （照片均来自 Pixabay，来源与许可见 `src/assets/skins/CREDITS.md`）
   - **自定义**：点击"＋自定义"卡片，可自选主题色、深浅背景调与背景动效；
     点"选择图片"还能直接用自己的图片当背景，图片会自动压缩到 1600px 并只保存在本机
3. 所有外观改动自动保存，快捷窗口同步生效
4. 「简约渐变」背景由纯 CSS 渐变图层绘制（见 `src/services/scenery.ts`），
   不依赖图片资源：任意分辨率都不糊，也不增加安装包体积

### 自动更新

应用内置了基于 Tauri 官方 updater 插件的自动更新：**每次启动**先拉取远端更新清单，
发现有更高版本就弹出提示并**自动开始下载**，下载完成后用户可以自己挑时间点「立即重启安装」，
安装程序装完会自动把应用重新拉起来。

更新清单是**发布流程自动生成**的 `latest.json`，挂在每个 Release 里（不用手工维护）：

```
https://github.com/LanjianNUll/xpaste/releases/latest/download/latest.json
```

内容大致长这样 —— 版本号、下载地址、更新日志、签名四样齐全：

```json
{
  "version": "2.0.9",
  "notes": "## 🎉 Xpaste v2.0.9\n\n- 这次改了什么……",
  "pub_date": "2026-10-12T02:10:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "…minisign 签名…",
      "url": "https://github.com/LanjianNUll/xpaste/releases/download/v2.0.9/Xpaste_2.0.9_x64-setup.nsis.zip"
    }
  }
}
```

更新日志（`notes`）的来源按优先级：`release-notes/<版本>.md`（手写，推荐）
→ 没有该文件时按两个 tag 之间的提交记录自动生成。

> **便携版说明**：自动更新走的是安装包（NSIS），所以只对用
> `Xpaste_x.y.z_x64-setup.exe` 安装的版本生效；用便携版 zip 的话，
> 更新时直接下载新版 zip 覆盖即可。

#### 维护者：首次启用前要做的两件事

更新包必须签名，客户端校验通过才会安装，所以需要一对 minisign 密钥：

```bash
# 1. 生成密钥对（私钥会写到 ~/.tauri/xpaste.key，请自己收好、不要提交到仓库）
pnpm tauri signer generate -w ~/.tauri/xpaste.key
```

2. 把命令输出的**公钥**填进 `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`
   （替换掉 `REPLACE_WITH_YOUR_UPDATER_PUBLIC_KEY` 占位符）

3. 到 GitHub 仓库 **Settings → Secrets and variables → Actions** 添加两个 Secret：

   | Secret 名 | 值 |
   | --- | --- |
   | `TAURI_SIGNING_PRIVATE_KEY` | `~/.tauri/xpaste.key` 文件的内容 |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 生成密钥时设置的密码（没设密码就留空/不添加） |

配置好之后，发版流程会自动多产出 `*.nsis.zip`、`*.sig` 与 `latest.json`。
如果这两项没配好，发版流程**不会失败**，只是跳过更新包并在运行摘要里给出警告
（客户端收不到更新提示）。



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
│   ├── services/          # API 服务（theme.ts 主题服务 / scenery.ts 皮肤背景生成 / ambient.ts 粒子动效）
│   ├── components/        # 组件（含 ThemeSettings 外观设置面板、AmbientLayer 背景层）
│   ├── assets/skins/      # 皮肤背景照片（CREDITS.md 记录来源与许可）
│   ├── styles/            # 样式文件（theme.css 主题令牌 / app.css 布局）
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
├── .github/workflows/     # CI：release.yml 负责构建 Windows 包并自动发版
├── scripts/               # 辅助脚本（bump-version.mjs 统一改版本号）
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

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

感谢以下开源项目：

- [Tauri](https://tauri.app/) - 构建桌面应用的最佳框架
- [Vue.js](https://vuejs.org/) - 渐进式 JavaScript 框架
- [Element Plus](https://element-plus.org/) - Vue 3 组件库
- [arboard](https://github.com/1Password/arboard) - 跨平台剪贴板库

皮肤背景照片来自 [Pixabay](https://pixabay.com/)（Pixabay 内容许可，可商用、无需署名），
逐张来源见 [`src/assets/skins/CREDITS.md`](src/assets/skins/CREDITS.md)。

## 🌟 Star History

如果这个项目对你有帮助，请给我们一个 ⭐️

---

<div align="center">

Made with ❤️ by the Xpaste

[报告问题](https://github.com/LanjianNUll/xpaste/issues) • [请求功能](https://github.com/LanjianNUll/xpaste/issues/new?labels=enhancement)

</div>
