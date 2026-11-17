#!/usr/bin/env bash
# GPUI Diff Tool - macOS ARM64 DMG 一键构建脚本
# 用法：在 Apple Silicon Mac（macOS 12+）上执行  bash scripts/build-macos.sh
#
# 注意：所有变量统一使用 ${VAR} 形式引用。macOS 自带的 bash 3.2 在解析
# 变量后紧跟中文时，会把多字节字符的部分字节并进变量名，在 set -u 下触发
# unbound variable。用花括号包裹可彻底避免该问题。
set -euo pipefail

APP_NAME="GPUI Diff Tool"
EXECUTABLE="gpui-diff-tool"
BUNDLE_ID="com.example.gpuidifftool"
VERSION="0.1.0"
TARGET="aarch64-apple-darwin"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${PROJECT_DIR}/build"
APP_DIR="${BUILD_DIR}/${APP_NAME}.app"
DMG_CONTENTS_DIR="${BUILD_DIR}/dmg-contents"
DMG_PATH="${BUILD_DIR}/${APP_NAME}-${VERSION}-arm64.dmg"

log() { printf '\033[1;34m[build]\033[0m %s\n' "$*"; }
err() { printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

# ---------- 1. 环境检查 ----------
log "检查构建环境..."
[[ "$(uname -s)" == "Darwin" ]] || err "此脚本必须在 macOS 上运行（当前系统: $(uname -s)）"
xcode-select -p >/dev/null 2>&1 || err "缺少 Xcode Command Line Tools，请先执行: xcode-select --install"
command -v cargo >/dev/null 2>&1 || err "缺少 cargo，请先安装 Rust: https://rustup.rs"

HOST_ARCH="$(uname -m)"
if [[ "${HOST_ARCH}" != "arm64" ]]; then
  log "当前架构为 ${HOST_ARCH} ，将交叉编译至 ${TARGET} （需要对应 macOS SDK）"
fi

# ---------- 2. 编译 release ----------
log "编译 release 版本（target=${TARGET}）..."
cd "${PROJECT_DIR}"
rustup target add "${TARGET}" 2>/dev/null || true
cargo build --release --target "${TARGET}"

BINARY="${PROJECT_DIR}/target/${TARGET}/release/${EXECUTABLE}"
[[ -f "${BINARY}" ]] || err "编译产物不存在: ${BINARY}"

# ---------- 3. 组装 .app bundle ----------
log "创建 App Bundle: ${APP_DIR}"
rm -rf "${APP_DIR}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"
cp "${BINARY}" "${APP_DIR}/Contents/MacOS/${EXECUTABLE}"
chmod +x "${APP_DIR}/Contents/MacOS/${EXECUTABLE}"

# 如需自定义图标，把 AppIcon.icns 放到 Contents/Resources 并取消下面两行注释：
# cp "${PROJECT_DIR}/AppIcon.icns" "${APP_DIR}/Contents/Resources/AppIcon.icns"
# sed -i '' 's#</dict>#<key>CFBundleIconFile</key><string>AppIcon.icns</string></dict>#' "${APP_DIR}/Contents/Info.plist"

cat > "${APP_DIR}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
    <key>CFBundleExecutable</key><string>${EXECUTABLE}</string>
    <key>CFBundleIdentifier</key><string>${BUNDLE_ID}</string>
    <key>CFBundleName</key><string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key><string>${APP_NAME}</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# An ad-hoc signature seals the assembled bundle but does not establish a trusted developer.
log "为 App Bundle 添加 ad-hoc 签名..."
codesign --force --sign - "${APP_DIR}"
codesign --verify --deep --strict --verbose=2 "${APP_DIR}"

# ---------- 4. 打包 DMG ----------
log "打包 DMG..."
mkdir -p "${BUILD_DIR}"
rm -f "${DMG_PATH}"
rm -rf "${DMG_CONTENTS_DIR}"
mkdir -p "${DMG_CONTENTS_DIR}"
cp -R "${APP_DIR}" "${DMG_CONTENTS_DIR}/"
cat > "${DMG_CONTENTS_DIR}/README.txt" <<'README'
首次打开提示

此应用使用 ad-hoc 签名，未使用 Developer ID 签名或 Apple 公证。
首次打开时，请按住 Control 键点按应用图标，选择“打开”，并在确认对话框中再次选择“打开”。

如果 macOS 仍提示应用已损坏，请先将应用拖入“应用程序”，然后在终端运行：
xattr -dr com.apple.quarantine "/Applications/GPUI Diff Tool.app"

仅对你信任的应用执行以上操作。正式分发需要 Developer ID 签名和 Apple 公证。
README

if command -v create-dmg >/dev/null 2>&1; then
  create-dmg \
    --volname "${APP_NAME}" \
    --window-size 640 420 \
    --icon-size 96 \
    --app-drop-link 480 200 \
    "${DMG_PATH}" \
    "${DMG_CONTENTS_DIR}" || err "create-dmg 打包失败"
else
  log "未安装 create-dmg，改用系统自带 hdiutil（无拖拽背景美化）"
  log "如需更漂亮的 DMG：brew install create-dmg"
  hdiutil create -volname "${APP_NAME}" -srcfolder "${DMG_CONTENTS_DIR}" -ov -format UDZO "${DMG_PATH}" \
    || err "hdiutil 打包失败"
fi

[[ -f "${DMG_PATH}" ]] || err "DMG 未生成"
log "完成 → ${DMG_PATH}"
ls -lh "${DMG_PATH}"
