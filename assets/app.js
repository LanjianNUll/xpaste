/* xpaste site — theme, i18n, small interactions. No dependencies. */
(function () {
  "use strict";

  var DICT = {
    zh: {
      "meta.index.title": "xpaste — 复制一次，随处粘贴",
      "meta.index.desc": "xpaste 是跨设备的剪贴板同步工具。本地优先、端到端加密，macOS / Windows / Linux / iOS / Android 全平台，自带命令行。",
      "meta.download.title": "下载 xpaste — 支持 macOS、Windows、Linux、iOS、Android",
      "meta.download.desc": "下载 xpaste 客户端与命令行工具。免费使用，端到端加密，支持六大平台。",
      "meta.docs.title": "xpaste 文档 — 快速开始、快捷键与 CLI",
      "meta.docs.desc": "xpaste 快速上手：安装、配对设备、快捷键、命令行参考与常见问题。",

      "nav.features": "功能",
      "nav.download": "下载",
      "nav.docs": "文档",
      "nav.cta": "免费下载",

      "hero.eyebrow": "v2.4 已发布 · 支持 6 个平台",
      "hero.title": "复制一次，<em>随处粘贴</em>。",
      "hero.sub": "xpaste 把剪贴板连成一片。在电脑上复制的地址，走到楼下用手机直接粘贴。本地优先、端到端加密，服务器只看得到密文。",
      "hero.cta1": "下载 xpaste",
      "hero.cta2": "看看怎么用",
      "hero.m1": "永久免费",
      "hero.m2": "端到端加密",
      "hero.m3": "离线可用",

      "mock.title": "剪贴板 · 今天",
      "mock.panel2": "已连接的设备",
      "mock.clip1": "https://xpaste.app/docs/quickstart?ref=onboarding",
      "mock.clip2": "xp cp ./build/report.pdf --to iphone",
      "mock.clip3": "周会纪要：Q4 只做三件事 —— 同步、检索、CLI。",
      "mock.chip1": "链接",
      "mock.chip2": "命令",
      "mock.chip3": "文本",
      "mock.dev1": "MacBook Pro",
      "mock.dev2": "已在 iPhone 上粘贴",
      "mock.dev3": "来自 iPad",
      "mock.t1": "刚刚",
      "mock.t2": "2 分钟",
      "mock.t3": "9 分钟",
      "mock.mac": "MacBook Pro",
      "mock.iphone": "iPhone 15",
      "mock.ipad": "iPad Air",
      "mock.pc": "Windows 台式机",
      "mock.online": "在线",
      "mock.idle": "空闲",
      "mock.sync": "已同步到 4 台设备 · 48ms",

      "feat.kicker": "能力",
      "feat.title": "够用的剪贴板，不该有学习成本。",
      "feat.sub": "没有复杂的规则和面板。复制、切换、粘贴，剩下的交给它。",
      "f1.t": "跨设备即时同步",
      "f1.d": "复制后平均 60ms 出现在其他设备，走 WebSocket 长连接，不用等轮询。",
      "f2.t": "端到端加密",
      "f2.d": "在本机加密后才上行，密钥只存在你的设备上。我们无法解密你的任何一条记录。",
      "f3.t": "历史与检索",
      "f3.d": "默认保留最近 500 条，按 ⌘⇧V 翻回去。支持按类型、设备、时间筛选。",
      "f4.t": "复制前随手改",
      "f4.d": "在历史里直接编辑再粘贴，去掉跟踪参数、引号或多余空行，原条目仍保留。",
      "f5.t": "命令行优先",
      "f5.d": "xp cp / xp paste / xp ls，SSH 进服务器也能直接把本地剪贴板递过去。",
      "f6.t": "本地优先",
      "f6.d": "断网照常复制粘贴，恢复后自动补传，冲突时按时间戳保留两份。",

      "ed.kicker": "编辑",
      "ed.head": "复制完才发现，要改一句。",
      "ed.sub": "切回原来的应用去找那句话太麻烦，粘进备忘录改完再复制更麻烦。xpaste 让你在剪贴板里直接改，改完就粘。",
      "ed.b1": "就地编辑",
      "ed.b1d": "双击任意一条历史，光标直接落在原文里，不用另开窗口。",
      "ed.b2": "常用整理一键完成",
      "ed.b2d": "去掉链接跟踪参数、外层引号、多余空行，链接和代码都不容易改坏。",
      "ed.b3": "改动不污染历史",
      "ed.b3d": "编辑结果另存为新版本，原始条目原样留着，随时能退回去。",
      "ed.b4": "手机上一样能改",
      "ed.b4d": "长按条目就能改，改完直接粘进聊天框，省掉一次来回切换。",
      "ed.panel": "剪贴板 · 刚刚复制",
      "ed.badge": "已修改",
      "ed.chip": "链接",
      "ed.label": "编辑这条内容",
      "ed.hint": "改完直接复制，原条目仍在历史里",
      "ed.a1": "去跟踪参数",
      "ed.a2": "去引号",
      "ed.a3": "压空行",
      "ed.a4": "还原",
      "ed.note": "可以直接在框里改字，也可以点上面的快捷整理。",
      "ed.copy": "复制修改版",
      "ed.copied": "已复制修改后的内容",
      "ed.copiedShort": "已复制",
      "ed.nochange": "这条已经足够干净了",
      "ed.fail": "已选中文本，按 ⌘C 复制即可",
      "ed.reset": "已还原为原始内容",

      "how.kicker": "工作方式",
      "how.title": "三跳，一条链路。",
      "how.sub": "从按下 ⌘C 到另一台设备出现内容，中间只经过一次加密上传。",
      "st1.t": "复制",
      "st1.d": "在任何 App 里照常复制。xpaste 在后台读取剪贴板变化，不拦截键盘。",
      "st2.t": "加密后上行",
      "st2.d": "本机用会话密钥加密，密文通过长连接推到同步服务，服务端只负责转发。",
      "st3.t": "在另一台粘贴",
      "st3.d": "目标设备收到密文，本地解密后写入剪贴板，直接 ⌘V 就行。",

      "sec.kicker": "隐私",
      "sec.title": "服务器经手的，只有密文。",
      "sec.sub": "端到端加密不是营销词，是架构约束：密钥在你设备上生成，同步服务拿不到明文，也无法为你恢复。",
      "sec.b1": "每条记录独立派生密钥",
      "sec.b1d": "XChaCha20-Poly1305，单条记录泄露不影响其他内容。",
      "sec.b2": "设备间密钥交换",
      "sec.b2d": "新设备通过二维码或短码加入，密钥只在设备之间交换。",
      "sec.b3": "可自托管",
      "sec.b3d": "官方服务之外，可以把同步端跑在自己的服务器或 NAS 上。",
      "sec.codehead": "同步服务收到的内容",

      "cta.title": "现在就可以开始。",
      "cta.sub": "装完两台设备，复制第一段文字，你会立刻明白它为什么值得常驻。",
      "cta.btn1": "下载 xpaste",
      "cta.btn2": "阅读文档",

      "ft.tagline": "跨设备剪贴板。本地优先，端到端加密。",
      "ft.product": "产品",
      "ft.resources": "资源",
      "ft.legal": "其他",
      "ft.download": "下载",
      "ft.features": "功能",
      "ft.docs": "文档",
      "ft.cli": "命令行",
      "ft.changelog": "更新日志",
      "ft.privacy": "隐私说明",
      "ft.status": "服务状态",
      "ft.rights": "© 2026 xpaste. 保留所有权利。",
      "ft.built": "为多设备的人而做",

      "dl.title": "下载 xpaste",
      "dl.sub": "同一个版本号，六个平台。装两台设备就能用，不需要配置服务器。",
      "dl.ver": "最新版本 2.4.0 · 2026-11-14",
      "dl.mac": "macOS",
      "dl.macd": "13 Ventura 及以上 · Apple Silicon / Intel",
      "dl.win": "Windows",
      "dl.wind": "10 (21H2) 及以上 · x64 / ARM64",
      "dl.linux": "Linux",
      "dl.linuxd": "deb / rpm / AppImage · x64",
      "dl.ios": "iOS",
      "dl.iosd": "16 及以上 · App Store",
      "dl.android": "Android",
      "dl.androidd": "10 及以上 · Google Play",
      "dl.server": "服务器",
      "dl.serverd": "无图形界面 · 仅命令行 · x64 / ARM64",
      "dl.install": "安装方式",
      "dl.get": "下载",
      "dl.store": "前往商店",
      "dl.cli": "命令行",
      "dl.clid": "不想装图形界面？CLI 覆盖全部核心能力。",
      "dl.req": "系统要求",
      "dl.log": "更新日志",
      "dl.log1t": "图片与文件直传",
      "dl.log1d": "图片、PDF 走独立通道，不再受 1MB 文本上限限制；新增 xp cp <file> 直接推到指定设备。",
      "dl.log2t": "历史检索重写",
      "dl.log2d": "本地索引改为增量构建，五万条记录下搜索延迟从 400ms 降到 30ms。",
      "dl.log3t": "Linux 正式支持",
      "dl.log3d": "提供 deb、rpm 与 AppImage，支持 Wayland 下的剪贴板监听。",

      "tbl.component": "组件",
      "tbl.min": "最低要求",
      "tbl.rec": "推荐",
      "tbl.os": "操作系统",
      "tbl.mem": "内存",
      "tbl.disk": "磁盘",

      "docs.title": "快速开始",
      "docs.sub": "三分钟从安装到第一次跨设备粘贴。",
      "qs.kicker": "第一步",
      "qs.title": "装好，然后配对。",
      "qs.s1t": "安装客户端",
      "qs.s1d": "下载对应平台的安装包，或用 Homebrew 一行装完。",
      "qs.s2t": "登录并配对",
      "qs.s2d": "在第一台设备上登录，生成配对二维码；其他设备扫码即加入同一密钥空间。",
      "qs.s3t": "开始使用",
      "qs.s3d": "照常复制，在另一台设备按 ⌘V。历史面板用 ⌘⇧V 打开。",
      "sc.title": "快捷键",
      "sc.action": "操作",
      "sc.mac": "macOS",
      "sc.win": "Windows / Linux",
      "sc.open": "打开历史面板",
      "sc.next": "切换到下一条",
      "sc.pin": "固定当前条目",
      "sc.pause": "暂停同步",
      "sc.edit": "编辑当前条目",
      "sc.copyedit": "复制修改后的版本",
      "sc.editnote": "编辑结果另存为新版本，原始条目保留在历史里，随时可以退回去。",
      "cli.title": "命令行",
      "cli.cmd": "命令",
      "cli.desc": "说明",
      "cli.c1": "把标准输入或文件推送到指定设备",
      "cli.c2": "取出最近一条，写入 stdout",
      "cli.c3": "列出历史，支持 --type 与 --device",
      "cli.c4": "查看设备与同步状态",
      "faq.title": "常见问题",
      "faq.q1": "免费版有限制吗？",
      "faq.a1": "核心同步、历史 500 条、全部平台都不收费。付费只影响历史保留时长与团队共享空间。",
      "faq.q2": "你们能看到我复制的内容吗？",
      "faq.a2": "不能。内容在本机加密后才上行，密钥不上传。代价是：忘记密钥就无法找回历史。",
      "faq.q3": "能在服务器上用它吗？",
      "faq.a3": "可以。CLI 支持无图形界面的 Linux，通过设备码登录，适合跳板机和 CI 环境。",
      "faq.q4": "支持自托管吗？",
      "faq.a4": "支持。同步端提供单文件二进制与 Docker 镜像，配置文件里指向自己的地址即可。"
    },

    en: {
      "meta.index.title": "xpaste — Copy once. Paste anywhere.",
      "meta.index.desc": "xpaste keeps your clipboard in sync across devices. Local-first, end-to-end encrypted, on macOS, Windows, Linux, iOS and Android — with a real CLI.",
      "meta.download.title": "Download xpaste — macOS, Windows, Linux, iOS, Android",
      "meta.download.desc": "Download the xpaste app and command line tool. Free, end-to-end encrypted, six platforms.",
      "meta.docs.title": "xpaste Docs — Quick start, shortcuts and CLI",
      "meta.docs.desc": "Get started with xpaste: install, pair your devices, keyboard shortcuts, CLI reference and FAQ.",

      "nav.features": "Features",
      "nav.download": "Download",
      "nav.docs": "Docs",
      "nav.cta": "Get xpaste",

      "hero.eyebrow": "v2.4 is out · six platforms",
      "hero.title": "Copy once.<br><em>Paste anywhere.</em>",
      "hero.sub": "xpaste turns every device into the same clipboard. Copy an address on your laptop, paste it on your phone downstairs. Local-first, end-to-end encrypted — the server only ever sees ciphertext.",
      "hero.cta1": "Download xpaste",
      "hero.cta2": "See how it works",
      "hero.m1": "Free forever",
      "hero.m2": "End-to-end encrypted",
      "hero.m3": "Works offline",

      "mock.title": "Clipboard · Today",
      "mock.panel2": "Connected devices",
      "mock.clip1": "https://xpaste.app/docs/quickstart?ref=onboarding",
      "mock.clip2": "xp cp ./build/report.pdf --to iphone",
      "mock.clip3": "Weekly notes: Q4 is three things — sync, search, CLI.",
      "mock.chip1": "Link",
      "mock.chip2": "Command",
      "mock.chip3": "Text",
      "mock.dev1": "MacBook Pro",
      "mock.dev2": "Pasted on iPhone",
      "mock.dev3": "From iPad",
      "mock.t1": "now",
      "mock.t2": "2 min",
      "mock.t3": "9 min",
      "mock.mac": "MacBook Pro",
      "mock.iphone": "iPhone 15",
      "mock.ipad": "iPad Air",
      "mock.pc": "Windows desktop",
      "mock.online": "Online",
      "mock.idle": "Idle",
      "mock.sync": "Synced to 4 devices · 48ms",

      "feat.kicker": "Capabilities",
      "feat.title": "A clipboard shouldn't need a manual.",
      "feat.sub": "No rules to learn, no panel to configure. Copy, switch, paste — that's the whole product.",
      "f1.t": "Instant sync",
      "f1.d": "Content lands on your other devices in about 60ms over a persistent WebSocket, not a polling loop.",
      "f2.t": "End-to-end encryption",
      "f2.d": "Encrypted on device before it leaves. Keys never leave your machines, so we can't read a single item.",
      "f3.t": "History and search",
      "f3.d": "The last 500 items by default. Hit ⌘⇧V to go back, filter by type, device or time.",
      "f4.t": "Edit before you paste",
      "f4.d": "Tweak the item in history before pasting — strip tracking params, quotes or blank lines. The original stays.",
      "f5.t": "CLI first",
      "f5.d": "xp cp, xp paste, xp ls. SSH into a server and hand it your local clipboard without leaving the shell.",
      "f6.t": "Local-first",
      "f6.d": "Copy and paste normally while offline. Everything queues and catches up, keeping both copies on conflict.",

      "ed.kicker": "Editing",
      "ed.head": "You spot the typo after copying.",
      "ed.sub": "Going back to the original app is a hassle, and pasting into a scratch note is worse. Edit the item where it lives, then paste.",
      "ed.b1": "Edit in place",
      "ed.b1d": "Double-click any history item and the caret lands in the original text — no extra window.",
      "ed.b2": "One-click cleanups",
      "ed.b2d": "Drop tracking parameters, wrapping quotes and stray blank lines without breaking links or code.",
      "ed.b3": "History stays clean",
      "ed.b3d": "Edits are saved as a new version. The original is untouched, so you can always go back.",
      "ed.b4": "Same on mobile",
      "ed.b4d": "Long-press an item to edit, then paste straight into the chat — one switch fewer.",
      "ed.panel": "Clipboard · just now",
      "ed.badge": "Edited",
      "ed.chip": "Link",
      "ed.label": "Edit this item",
      "ed.hint": "Copy when you're done — the original stays in history",
      "ed.a1": "Strip tracking",
      "ed.a2": "Drop quotes",
      "ed.a3": "Tidy lines",
      "ed.a4": "Revert",
      "ed.note": "Type in the box directly, or use a cleanup above.",
      "ed.copy": "Copy edited",
      "ed.copied": "Edited version copied",
      "ed.copiedShort": "Copied",
      "ed.nochange": "Already clean",
      "ed.fail": "Text selected — press ⌘C to copy",
      "ed.reset": "Reverted to the original",

      "how.kicker": "How it works",
      "how.title": "Three hops, one path.",
      "how.sub": "From ⌘C to the other device, exactly one encrypted upload happens in between.",
      "st1.t": "Copy",
      "st1.d": "Copy as usual in any app. xpaste watches the clipboard in the background and never hooks your keys.",
      "st2.t": "Encrypt, then upload",
      "st2.d": "Encrypted locally with a per-item key, pushed over the socket. The relay only forwards bytes.",
      "st3.t": "Paste elsewhere",
      "st3.d": "The target device decrypts locally and writes to its clipboard. Press ⌘V — nothing else to do.",

      "sec.kicker": "Privacy",
      "sec.title": "All the server handles is ciphertext.",
      "sec.sub": "End-to-end encryption here is an architectural constraint, not a slogan: keys are generated on your devices, and the sync service can't recover them for you.",
      "sec.b1": "Per-item derived keys",
      "sec.b1d": "XChaCha20-Poly1305, so one leaked item doesn't expose the rest.",
      "sec.b2": "Device-to-device key exchange",
      "sec.b2d": "A new device joins by QR or short code; keys never travel through us.",
      "sec.b3": "Self-hostable",
      "sec.b3d": "Run the sync endpoint on your own server or NAS instead of our hosted one.",
      "sec.codehead": "What the sync service receives",

      "cta.title": "Start with two devices.",
      "cta.sub": "Install it twice, copy one line of text, and you'll get why people keep it running.",
      "cta.btn1": "Download xpaste",
      "cta.btn2": "Read the docs",

      "ft.tagline": "Cross-device clipboard. Local-first, end-to-end encrypted.",
      "ft.product": "Product",
      "ft.resources": "Resources",
      "ft.legal": "More",
      "ft.download": "Download",
      "ft.features": "Features",
      "ft.docs": "Docs",
      "ft.cli": "Command line",
      "ft.changelog": "Changelog",
      "ft.privacy": "Privacy",
      "ft.status": "Status",
      "ft.rights": "© 2026 xpaste. All rights reserved.",
      "ft.built": "Built for people with too many screens",

      "dl.title": "Download xpaste",
      "dl.sub": "One version number, six platforms. Install it on two devices and you're done — no server to configure.",
      "dl.ver": "Latest 2.4.0 · Nov 14, 2026",
      "dl.mac": "macOS",
      "dl.macd": "13 Ventura or later · Apple Silicon / Intel",
      "dl.win": "Windows",
      "dl.wind": "10 (21H2) or later · x64 / ARM64",
      "dl.linux": "Linux",
      "dl.linuxd": "deb / rpm / AppImage · x64",
      "dl.ios": "iOS",
      "dl.iosd": "16 or later · App Store",
      "dl.android": "Android",
      "dl.androidd": "10 or later · Google Play",
      "dl.server": "Server",
      "dl.serverd": "Headless · CLI only · x64 / ARM64",
      "dl.install": "Install guide",
      "dl.get": "Download",
      "dl.store": "Open store",
      "dl.cli": "Command line",
      "dl.clid": "No GUI needed — the CLI covers every core capability.",
      "dl.req": "System requirements",
      "dl.log": "Changelog",
      "dl.log1t": "Image and file transfer",
      "dl.log1d": "Images and PDFs take a separate channel, no longer capped by the 1MB text limit. New: xp cp <file> --to <device>.",
      "dl.log2t": "Rewritten history search",
      "dl.log2d": "The local index is now incremental — search latency dropped from 400ms to 30ms at 50k items.",
      "dl.log3t": "Official Linux support",
      "dl.log3d": "deb, rpm and AppImage builds, with clipboard monitoring under Wayland.",

      "tbl.component": "Component",
      "tbl.min": "Minimum",
      "tbl.rec": "Recommended",
      "tbl.os": "OS",
      "tbl.mem": "Memory",
      "tbl.disk": "Disk",

      "docs.title": "Quick start",
      "docs.sub": "From install to your first cross-device paste in three minutes.",
      "qs.kicker": "Step one",
      "qs.title": "Install it, then pair.",
      "qs.s1t": "Install the app",
      "qs.s1d": "Grab the build for your platform, or install with a single Homebrew command.",
      "qs.s2t": "Sign in and pair",
      "qs.s2d": "Sign in on the first device and generate a pairing QR. Others scan it to join the same key space.",
      "qs.s3t": "Use it",
      "qs.s3d": "Copy as usual, then press ⌘V on another device. Open history with ⌘⇧V.",
      "sc.title": "Shortcuts",
      "sc.action": "Action",
      "sc.mac": "macOS",
      "sc.win": "Windows / Linux",
      "sc.open": "Open history",
      "sc.next": "Next item",
      "sc.pin": "Pin current item",
      "sc.pause": "Pause sync",
      "sc.edit": "Edit current item",
      "sc.copyedit": "Copy edited version",
      "sc.editnote": "Edits save as a new version; the original stays in history so you can always roll back.",
      "cli.title": "Command line",
      "cli.cmd": "Command",
      "cli.desc": "Description",
      "cli.c1": "Push stdin or a file to a specific device",
      "cli.c2": "Print the most recent item to stdout",
      "cli.c3": "List history, filter with --type and --device",
      "cli.c4": "Show device and sync status",
      "faq.title": "FAQ",
      "faq.q1": "Is the free plan limited?",
      "faq.a1": "Core sync, 500 history items and every platform are free. Paid plans only extend retention and add shared team spaces.",
      "faq.q2": "Can you read what I copy?",
      "faq.a2": "No. Content is encrypted before upload and keys are never sent to us. The trade-off: if you lose your key, history can't be recovered.",
      "faq.q3": "Does it work on servers?",
      "faq.a3": "Yes. The CLI runs on headless Linux and signs in with a device code, which makes it usable on jump hosts and in CI.",
      "faq.q4": "Can I self-host?",
      "faq.a4": "Yes. The sync endpoint ships as a single binary and a Docker image — point the config at your own address."
    }
  };

  var root = document.documentElement;
  var store = {
    get: function (k) {
      try { return localStorage.getItem(k); } catch (e) { return null; }
    },
    set: function (k, v) {
      try { localStorage.setItem(k, v); } catch (e) {}
    }
  };

  function currentLang() {
    var qp = null;
    try { qp = new URLSearchParams(window.location.search).get("lang"); } catch (e) {}
    if (qp === "zh" || qp === "en") return qp;
    var l = store.get("xp-lang");
    if (l === "zh" || l === "en") return l;
    var nav = (navigator.language || "zh").toLowerCase();
    return nav.indexOf("zh") === 0 ? "zh" : "en";
  }

  function applyLang(lang) {
    var dict = DICT[lang] || DICT.zh;
    root.lang = lang === "zh" ? "zh-CN" : "en";
    root.setAttribute("data-lang", lang);

    document.querySelectorAll("[data-i18n]").forEach(function (el) {
      var k = el.getAttribute("data-i18n");
      if (dict[k] != null) el.textContent = dict[k];
    });

    document.querySelectorAll("[data-i18n-html]").forEach(function (el) {
      var k = el.getAttribute("data-i18n-html");
      if (dict[k] != null) el.innerHTML = dict[k];
    });

    document.querySelectorAll("[data-i18n-attr]").forEach(function (el) {
      el.getAttribute("data-i18n-attr").split("|").forEach(function (pair) {
        var bits = pair.split(":");
        if (bits.length === 2 && dict[bits[1]] != null) el.setAttribute(bits[0], dict[bits[1]]);
      });
    });

    var page = document.body.getAttribute("data-page") || "index";
    var tk = "meta." + page + ".title";
    var dk = "meta." + page + ".desc";
    if (dict[tk]) document.title = dict[tk];
    var md = document.querySelector('meta[name="description"]');
    if (md && dict[dk]) md.setAttribute("content", dict[dk]);

    var label = document.getElementById("langLabel");
    if (label) label.textContent = lang === "zh" ? "EN" : "中文";
    var lb = document.getElementById("langBtn");
    if (lb) lb.setAttribute("aria-label", lang === "zh" ? "Switch to English" : "切换到中文");

    store.set("xp-lang", lang);
  }

  function applyTheme(theme) {
    if (theme === "light" || theme === "dark") {
      root.setAttribute("data-theme", theme);
    } else {
      root.removeAttribute("data-theme");
    }
    var tb = document.getElementById("themeBtn");
    if (tb) tb.setAttribute("aria-label", "Toggle color theme");
  }

  function effectiveTheme() {
    var t = root.getAttribute("data-theme");
    if (t === "light" || t === "dark") return t;
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  document.addEventListener("DOMContentLoaded", function () {
    applyLang(currentLang());

    var langBtn = document.getElementById("langBtn");
    if (langBtn) {
      langBtn.addEventListener("click", function () {
        applyLang(currentLang() === "zh" ? "en" : "zh");
      });
    }

    var themeBtn = document.getElementById("themeBtn");
    if (themeBtn) {
      themeBtn.addEventListener("click", function () {
        var next = effectiveTheme() === "dark" ? "light" : "dark";
        applyTheme(next);
        store.set("xp-theme", next);
      });
    }

    var nav = document.querySelector(".nav");
    if (nav) {
      var onScroll = function () {
        nav.classList.toggle("is-stuck", window.scrollY > 8);
      };
      onScroll();
      window.addEventListener("scroll", onScroll, { passive: true });
    }

    var reveals = document.querySelectorAll(".reveal");
    if (reveals.length && "IntersectionObserver" in window) {
      var io = new IntersectionObserver(function (entries) {
        entries.forEach(function (e) {
          if (e.isIntersecting) {
            e.target.classList.add("is-in");
            io.unobserve(e.target);
          }
        });
      }, { rootMargin: "0px 0px -8% 0px", threshold: 0.08 });
      reveals.forEach(function (el) { io.observe(el); });
    } else {
      reveals.forEach(function (el) { el.classList.add("is-in"); });
    }

    var edInput = document.getElementById("edInput");
    if (edInput) {
      var edCard = document.getElementById("editCard");
      var edToast = document.getElementById("edToast");
      var edCopyBtn = document.getElementById("edCopy");
      var edCopyLabel = document.getElementById("edCopyLabel");
      var original = edInput.value;
      var toastTimer = null;

      var tr = function (k, fallback) {
        var d = DICT[currentLang()] || DICT.zh;
        return d[k] != null ? d[k] : fallback;
      };

      var toast = function (msg) {
        if (!edToast) return;
        edToast.textContent = msg;
        edToast.classList.add("is-on");
        clearTimeout(toastTimer);
        toastTimer = setTimeout(function () { edToast.classList.remove("is-on"); }, 2300);
      };

      var sync = function () {
        if (edCard) edCard.classList.toggle("is-edited", edInput.value !== original);
        edInput.style.height = "auto";
        edInput.style.height = Math.min(edInput.scrollHeight + 2, 210) + "px";
      };

      var transforms = {
        params: function (t) {
          if (!/^https?:\/\//i.test(t.trim())) return t;
          try {
            var u = new URL(t.trim());
            var dead = [];
            u.searchParams.forEach(function (_v, k) {
              if (/^(utm_|fbclid$|gclid$|spm$|ref$|from$|source$|share|si$)/i.test(k)) dead.push(k);
            });
            dead.forEach(function (k) { u.searchParams.delete(k); });
            return u.toString();
          } catch (e) {
            return t;
          }
        },
        quotes: function (t) {
          return t.replace(/^\s*["'\u201c\u201d\u2018\u2019\u300c\u300d\u300e\u300f]+/, "")
                  .replace(/["'\u201c\u201d\u2018\u2019\u300c\u300d\u300e\u300f]+\s*$/, "")
                  .trim();
        },
        blank: function (t) {
          return t.replace(/[ \t]+$/gm, "").replace(/\n{3,}/g, "\n\n").trim();
        }
      };

      edInput.addEventListener("input", sync);

      Array.prototype.forEach.call(edCard.querySelectorAll("[data-act]"), function (btn) {
        btn.addEventListener("click", function () {
          var act = btn.getAttribute("data-act");
          if (act === "reset") {
            edInput.value = original;
            sync();
            toast(tr("ed.reset", "已还原为原始内容"));
            return;
          }
          var fn = transforms[act];
          if (!fn) return;
          var next = fn(edInput.value);
          if (next === edInput.value) {
            toast(tr("ed.nochange", "这条已经足够干净了"));
            return;
          }
          edInput.value = next;
          sync();
        });
      });

      var legacyCopy = function () {
        try {
          edInput.focus();
          edInput.select();
          return document.execCommand("copy");
        } catch (e) {
          return false;
        }
      };

      var copyEdited = function () {
        var done = function (ok) {
          if (ok) {
            toast(tr("ed.copied", "已复制修改后的内容"));
          } else {
            edInput.focus();
            edInput.select();
            toast(tr("ed.fail", "已选中文本，按 ⌘C 复制即可"));
          }
          if (edCopyLabel) {
            edCopyLabel.textContent = ok ? tr("ed.copiedShort", "已复制") : tr("ed.copy", "复制修改版");
            setTimeout(function () {
              edCopyLabel.textContent = tr("ed.copy", "复制修改版");
            }, 1900);
          }
        };
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(edInput.value).then(
            function () { done(true); },
            function () { done(legacyCopy()); }
          );
        } else {
          done(legacyCopy());
        }
      };

      if (edCopyBtn) edCopyBtn.addEventListener("click", copyEdited);
      edInput.addEventListener("keydown", function (e) {
        if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
          e.preventDefault();
          copyEdited();
        }
      });

      sync();
    }

    var y = document.getElementById("year");
    if (y) y.textContent = new Date().getFullYear();
  });

  window.xpaste = { applyLang: applyLang, applyTheme: applyTheme };
})();
