"""dmgbuild 打包设置：安装窗口的 Finder 布局。

用法（一般由 scripts/build-dmg.sh 调用）：
    DMG_OUT=/path/to/out.dmg python3 -m dmgbuild -s src-tauri/dmg/settings.py Xpaste /path/to/out.dmg
"""

import os

# dmgbuild 是 exec 执行本文件的，没有 __file__，仓库根目录由调用方通过环境变量传入
ROOT = os.environ.get("PROJECT_ROOT") or os.getcwd()
BUNDLE = os.path.join(ROOT, "src-tauri", "target", "release", "bundle")

app_path = os.path.join(BUNDLE, "macos", "Xpaste.app")

# 输出文件名由调用方通过环境变量指定；直接运行本文件时落在 bundle/dmg 下
filename = os.environ.get("DMG_OUT", os.path.join(BUNDLE, "dmg", "Xpaste.dmg"))
volume_name = "Xpaste"

# 背景图与 @2x 会自动合成 HiDPI tiff（Retina 下不模糊）
background = os.path.join(ROOT, "src-tauri", "dmg", "background.png")
icon = os.path.join(ROOT, "src-tauri", "icons", "icon.icns")

files = [app_path]
symlinks = {"Applications": "/Applications"}

# 图标中心坐标，原点为窗口内容区左上角，单位是点
icon_locations = {
    "Xpaste.app": (165, 216),
    "Applications": (495, 216),
}

# 窗口尺寸 = 内容区 660x420 + 28pt 标题栏，背景图正好铺满内容区
window_rect = ((330, 180), (660, 448))

default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
show_icon_preview = False
show_item_info = False
label_pos = "bottom"

icon_size = 112
text_size = 13

format = "UDZO"
filesystem = "HFS+"
