# GPUI Diff Tool

一个基于 [GPUI](https://gpui.rs) 的 macOS 原生 Side-by-Side 文件 Diff 工具。

## 修复内容（相对原始 zip）

1. **`Cargo.toml`**：`gpui` 从 zed `main` 分支改为 crates.io 稳定版 `0.2.2`，这是原代码 API 对应的版本，已实测 `cargo check` 通过；移除了未使用的 `smol` 依赖。
2. **`src/main.rs`**：
   - 文件读取失败不再静默忽略，改为在顶部栏显示红色错误提示；
   - 滚动容器补 `.min_h(px(0.0))`，避免大内容时不出现滚动条。

> 完整审查意见见 `../CODE_REVIEW.md`。

## 本地开发

需要 macOS + Xcode Command Line Tools：

```bash
git clone <your-repo> gpui-diff-tool
cd gpui-diff-tool
cargo run
```

## 构建 macOS ARM64 DMG

### 方式 A：在 Apple Silicon Mac 上

```bash
# 可选：brew install create-dmg   # 获得带拖拽背景的 DMG，否则用系统 hdiutil
bash scripts/build-macos.sh
```

产物：`build/GPUI Diff Tool-0.1.0-arm64.dmg`

脚本做了四件事：环境检查 → `cargo build --release --target aarch64-apple-darwin` → 组装 `.app` bundle（含 `Info.plist`）→ 用 `create-dmg` / `hdiutil` 打包。

### 方式 B：GitHub Actions 云端构建（无需本地 Mac）

把本目录推到 GitHub 仓库，然后：

```bash
git tag v0.1.0
git push origin v0.1.0
```

或在 Actions 页面手动触发 `Build macOS ARM64 DMG`。工作流使用 `macos-14`（Apple Silicon）runner，构建完成后在 Run 页面下载 `gpui-diff-tool-arm64-dmg` artifact。

## 签名与公证（可选，用于对外分发）

本地构建未做代码签名。若要让 Gatekeeper 放行，需要在拥有 Apple Developer 账号的环境下对 `.app` 执行 `codesign --deep --force --sign "Developer ID Application: ..."`，再做 `xcrun notarytool` 公证。CI 中可通过 `apple-actions/import-codesign-certs` 等 action 注入证书实现。

## 已知限制

- 非 UTF-8 编码文件会显示替换字符（`String::from_utf8_lossy`）。
- 大文件 diff 在当前线程同步计算，可能短暂无响应；详见 review 第 4 条。
