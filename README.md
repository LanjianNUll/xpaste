# xpaste — 产品官网

跨设备剪贴板工具 xpaste 的营销站与下载页。纯静态，无构建步骤，无外部依赖。

## 页面

| 文件 | 说明 |
| --- | --- |
| `index.html` | 产品首页：价值主张、功能、工作方式、隐私、CTA |
| `download.html` | 软件下载页：六大平台、CLI、系统要求、更新日志 |
| `docs.html` | 文档：快速开始、快捷键、命令行参考、FAQ |
| `404.html` | 自定义 404 |

## 特性

- **暗色 / 浅色**：跟随系统（`prefers-color-scheme`），可手动切换并记忆（`localStorage` 键 `xp-theme`）；首屏内联脚本避免闪烁。
- **国际化**：中 / 英双语，跟随浏览器语言，可手动切换（`xp-lang`），也支持 `?lang=en` 直接指定；文案集中在 `assets/app.js` 的 `DICT`。
- **响应式**：1120px 内容宽度，960 / 640 / 460 三个断点。
- **零依赖**：系统字体栈、内联 SVG 图标、无 CDN，国内访问不受影响。

## 本地预览

```bash
python3 -m http.server 8080
# http://localhost:8080
```

## 部署到 GitHub Pages

### 方式一：用户站点（推荐，链接最短）

1. 在 GitHub 新建仓库 `<你的用户名>.github.io`。
2. 推送本目录内容到 `main` 分支：

```bash
git remote add origin git@github.com:<用户名>/<用户名>.github.io.git
git branch -M main
git push -u origin main
```

3. 仓库 Settings → Pages → Source 选择 `Deploy from a branch`，分支 `main`、目录 `/ (root)`。
4. 等 1 分钟左右，访问 `https://<用户名>.github.io/`。

### 方式二：项目站点

仓库名任意（例如 `xpaste`），Pages 同样指向 `main` / root，访问地址为：

```
https://<用户名>.github.io/xpaste/
```

页面内所有链接与静态资源都是相对路径，因此放在子路径下也能正常工作。

> `.nojekyll` 已包含，避免 Jekyll 处理静态资源。

## 修改文案

所有双语文案都在 `assets/app.js` 顶部的 `DICT` 对象里，按 `zh` / `en` 两组、以扁平 key 组织。HTML 里用 `data-i18n`（纯文本）、`data-i18n-html`（允许内联标签）引用。

新增文案时两边都要加 key，否则切换语言会留空白。

---

文案与产品名 `xpaste` 为占位设定（跨设备剪贴板），按实际产品调整即可。
