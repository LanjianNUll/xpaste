#!/usr/bin/env bash
# 创建（或复用）本地代码签名身份。
#
# 为什么需要它：用 `codesign -s -`（ad-hoc）签名时，签名的 "designated requirement"
# 只有一行 cdhash。每重新编译一次，cdhash 就变，系统里那条「辅助功能」授权随之失效——
# 但设置列表里那条记录还在、开关仍然是绿的，于是出现「明明允许了还是粘贴不了」。
#
# 换成固定证书后，DR 变成 `identifier "com.paste.app" and certificate root = H"..."`，
# 重新编译也不会变，授权一次即可长期有效。
#
# 证书是自签的，只在本地用于让 TCC 认得出"同一个应用"，不具备任何分发/公证效力。
# 私钥放在独立钥匙串里，不动登录钥匙串。

set -euo pipefail

SIGN_DIR="$HOME/Library/Application Support/Xpaste/signing"
KEYCHAIN="$SIGN_DIR/xpaste-signing.keychain-db"
IDENTITY="Xpaste Local Signing"
KEYCHAIN_PASSWORD="xpaste-local"

if security find-identity -v -p codesigning "$KEYCHAIN" 2>/dev/null | grep -q "$IDENTITY"; then
  echo "签名身份已存在：$IDENTITY"
  exit 0
fi

echo "==> 生成自签名代码签名证书"
mkdir -p "$SIGN_DIR"
cd "$SIGN_DIR"

openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
  -keyout xpaste-key.pem -out xpaste-cert.pem \
  -subj "/CN=$IDENTITY/O=Xpaste Local/C=CN" \
  -addext "basicConstraints=critical,CA:false" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning" 2>/dev/null

# macOS 的 security 不认 OpenSSL 3 默认的 AES-256/PBKDF2 封装，必须用 legacy 格式
openssl pkcs12 -export -legacy -inkey xpaste-key.pem -in xpaste-cert.pem \
  -out xpaste-identity.p12 -passout "pass:$KEYCHAIN_PASSWORD" -name "$IDENTITY" 2>/dev/null

echo "==> 导入独立钥匙串"
security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN" 2>/dev/null || true
security set-keychain-settings "$KEYCHAIN"            # 不设自动锁定
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
security import xpaste-identity.p12 -k "$KEYCHAIN" -P "$KEYCHAIN_PASSWORD" -T /usr/bin/codesign >/dev/null
# 允许 codesign 在无人值守时使用私钥，否则每次编译都会弹"允许访问钥匙串"
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null 2>&1

rm -f xpaste-identity.p12

echo "完成：${IDENTITY}（${KEYCHAIN}）"
