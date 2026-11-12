#!/usr/bin/env bash
# 打包带美化安装界面的 dmg。
#
# 背景：Tauri 自带的 dmg 封装脚本在 macOS 15 上会失败（create-dmg 的 grep 正则问题），
# 而且默认窗口是白底、没有背景图。这里改用 dmgbuild 直接写 .DS_Store，
# 布局数据见 src-tauri/dmg/settings.py，背景图见 src-tauri/dmg/background.png。
#
# 用法：
#   scripts/build-dmg.sh             # 重新编译并打包
#   scripts/build-dmg.sh --no-build  # 复用已有的 .app，只重打 dmg

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/src-tauri/target/release/bundle/macos/Xpaste.app"
DMG_DIR="$ROOT/src-tauri/target/release/bundle/dmg"

if ! python3 -c "import dmgbuild" >/dev/null 2>&1; then
  echo "缺少 dmgbuild，请先执行：pip3 install dmgbuild" >&2
  exit 1
fi

if [[ "${1:-}" != "--no-build" ]]; then
  echo "==> 编译应用"
  (cd "$ROOT" && npx tauri build --bundles app)
fi

if [[ ! -d "$APP" ]]; then
  echo "未找到 $APP，请先不带 --no-build 运行一次" >&2
  exit 1
fi

# 签名：优先用固定证书，TCC 的「辅助功能」授权才能跨版本保留；
# 没有证书时退回 ad-hoc（此时每次重装都要重新授权，见 scripts/setup-signing.sh 注释）。
SIGN_KEYCHAIN="$HOME/Library/Application Support/Xpaste/signing/xpaste-signing.keychain-db"
SIGN_IDENTITY="Xpaste Local Signing"
SIGN_KEYCHAIN_PASSWORD="xpaste-local"

sign_app() {
  # 证书名可能因为重复导入而变歧义，直接用 SHA-1 指纹指定身份
  local fingerprint
  fingerprint="$(security find-certificate -c "$SIGN_IDENTITY" -Z "$SIGN_KEYCHAIN" 2>/dev/null \
    | awk '/SHA-1 hash:/{print $3; exit}')"

  if [[ -n "$fingerprint" ]]; then
    local backup
    backup="$(mktemp)"
    security list-keychains -d user | sed 's/^[[:space:]]*"//; s/"$//' > "$backup"
    security unlock-keychain -p "$SIGN_KEYCHAIN_PASSWORD" "$SIGN_KEYCHAIN" >/dev/null 2>&1 || true
    # codesign 只在钥匙串搜索列表里找身份，用完立刻还原，不改动用户的长期配置
    security list-keychains -d user -s "$SIGN_KEYCHAIN" $(tr '\n' ' ' < "$backup")
    if codesign --force --deep --sign "$fingerprint" "$APP"; then
      echo "    已用固定证书签名：$SIGN_IDENTITY ($fingerprint)"
    else
      echo "    固定证书签名失败，退回 ad-hoc" >&2
      codesign --force --deep --sign - "$APP"
    fi
    security list-keychains -d user -s $(tr '\n' ' ' < "$backup")
    rm -f "$backup"
  else
    echo "==> 未配置签名证书，使用 ad-hoc（每次重装都需重新授予辅助功能）"
    codesign --force --deep --sign - "$APP"
  fi
}

echo "==> 签名"
sign_app

VERSION="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["version"])' "$ROOT/src-tauri/tauri.conf.json")"
if [[ "$(uname -m)" == "arm64" ]]; then
  SUFFIX="aarch64"
else
  SUFFIX="x64"
fi

OUT="$DMG_DIR/Xpaste_${VERSION}_${SUFFIX}.dmg"
mkdir -p "$DMG_DIR"
rm -f "$OUT"

# 先卸载可能残留的同名卷，否则挂载会落到 "Xpaste 1" 上导致背景别名失效
hdiutil detach "/Volumes/Xpaste" -force >/dev/null 2>&1 || true

echo "==> 生成 dmg（含背景、图标位置、窗口尺寸）"
PROJECT_ROOT="$ROOT" DMG_OUT="$OUT" python3 -m dmgbuild -s "$ROOT/src-tauri/dmg/settings.py" "Xpaste" "$OUT"

echo "==> 校验"
hdiutil verify "$OUT" | tail -1
shasum -a 256 "$OUT"
echo "完成：$OUT"
