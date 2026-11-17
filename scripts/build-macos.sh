#!/usr/bin/env bash
# GPUI Diff Tool — macOS ARM64 DMG 一键构建脚本
# 用法：在 Apple Silicon Mac（macOS 12+）上执行  bash scripts/build-macos.sh
set -euo pipefail

APP_NAME="GPUI Diff Tool"
EXECUTABLE="gpui-diff-tool"
BUNDLE_ID="com.example.gpuidifftool"
VERSION="0.1.0"
TARGET="aarch64-apple-darwin"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$PROJECT_DIR/build"
APP_DIR="$BUILD_DIR/$APP_NAME.app"
DMG_PATH="$BUILD_DIR/$APP_NAME-$VERSION-arm64.dmg"

log() { printf '\033[1;34m[build]\033[0m %s\n' "$*"; }
err() { printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

# ---------- 1. 环境检查 ----------
log "检查构建环境..."
[[ "$(uname -s)" == "Darwin" ]] || err "此脚本必须在 macOS 上运行（当前系统: $(uname -s)）"
xcode-select -p >/dev/null 2>&1 || err "缺少 Xcode Command Line Tools，请先执行: xcode-select --install"
command -v cargo >/dev/null 2>&1 || err "缺少 cargo，请先安装 Rust: https://rustup.rs"

HOST_ARCH="$(uname -m)"
if [[ "$HOST_ARCH" != "arm64" ]]; then
  log "当前架构为 $HOST_ARCH，将交叉编译至 $TARGET（需要对应 macOS SDK）"
fi

# ---------- 2. 编译 release ----------
log "编译 release 版本（target=$TARGET）..."
cd "$PROJECT_DIR"
rustup target add "$TARGET" 2>/dev/null || true
cargo build --release --target "$TARGET"

BINARY="$PROJECT_DIR/target/$TARGET/release/$EXECUTABLE"
[[ -f "$BINARY" ]] || err "编译产物不存在: $BINARY"

# ---------- 3. 组装 .app bundle ----------
log "创建 App Bundle: $APP_DIR"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$BINARY" "$APP_DIR/Contents/MacOS/$EXECUTABLE"
chmod +x "$APP_DIR/Contents/MacOS/$EXECUTABLE"

# 如需自定义图标，把 AppIcon.icns 放到 Contents/Resources 并取消下面两行注释：
# cp "$PROJECT_DIR/AppIcon.icns" "$APP_DIR/Contents/Resources/AppIcon.icns"
# sed -i '' 's#</dict>#<key>CFBundleIconFile</key><string>AppIcon.icns</string></dict>#' "$APP_DIR/Contents/Info.plist"

cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>zh_CN</string>
    <key>CFBundleExecutable</key><string>$EXECUTABLE</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleName</key><string>$APP_NAME</string>
    <key>CFBundleDisplayName</key><string>$APP_NAME</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# ---------- 4. 打包 DMG ----------
log "打包 DMG..."
mkdir -p "$BUILD_DIR"
rm -f "$DMG_PATH"

if command -v create-dmg >/dev/null 2>&1; then
  create-dmg \
    --volname "$APP_NAME" \
    --window-size 640 420 \
    --icon-size 96 \
    --app-drop-link 480 200 \
    "$DMG_PATH" \
    "$APP_DIR" || err "create-dmg 打包失败"
else
  log "未安装 create-dmg，改用系统自带 hdiutil（无拖拽背景美化）"
  log "如需更漂亮的 DMG：brew install create-dmg"
  hdiutil create -volname "$APP_NAME" -srcfolder "$APP_DIR" -ov -format UDZO "$DMG_PATH" \
    || err "hdiutil 打包失败"
fi

[[ -f "$DMG_PATH" ]] || err "DMG 未生成"
log "完成 → $DMG_PATH"
ls -lh "$DMG_PATH"
