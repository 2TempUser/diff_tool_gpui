# GPUI Diff Tool

一个基于 [GPUI](https://gpui.rs) 的 macOS 原生 Side-by-Side 文件与文本 Diff 工具。

## 使用

- 在左右路径栏输入文件路径，或点“选择”打开文件选择器，再点“打开”载入文件。
- 点“粘贴文本”展开双栏文本框，粘贴两段内容后点“比较文本”。
- 点“清空”移除两侧文件、路径、文本和差异结果。

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

## macOS 首次启动与正式分发

构建脚本会对 `.app` 做 ad-hoc 签名并验证 bundle 完整性，但这不等于 Developer ID 签名，也不包含 Apple 公证。首次从 DMG 打开时，macOS 仍可能提示无法验证开发者；可按住 Control 键点按应用，选择“打开”并确认。

如果 macOS 仍提示应用已损坏，请先将应用拖入“应用程序”，然后在终端运行：

```bash
xattr -dr com.apple.quarantine "/Applications/GPUI Diff Tool.app"
```

仅对你信任的应用执行以上操作。要让下载后的应用无需手动放行即可通过 Gatekeeper，需要 Developer ID Application 证书签名并完成 Apple 公证。

## 已知限制

- 非 UTF-8 编码文件会显示替换字符（`String::from_utf8_lossy`）。
- 大文件 diff 在当前线程同步计算，可能短暂无响应；详见 review 第 4 条。
