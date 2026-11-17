use gpui::{
    div, prelude::*, px, rgb, rgba, App, Application, Bounds, Context, Entity,
    ScrollHandle, SharedString, Window, WindowBounds, WindowOptions,
};
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
struct DiffRow {
    left_line_no: Option<usize>,
    left_text: SharedString,
    left_tag: Option<ChangeTag>,
    right_line_no: Option<usize>,
    right_text: SharedString,
    right_tag: Option<ChangeTag>,
}

struct DiffApp {
    left_path: Option<PathBuf>,
    right_path: Option<PathBuf>,
    left_content: String,
    right_content: String,
    rows: Vec<DiffRow>,
    added_count: usize,
    removed_count: usize,
    scroll_handle: ScrollHandle,
    error_message: Option<String>,
}

impl DiffApp {
    fn new(_cx: &mut Context<Self>) -> Self {
        let left_sample = "fn main() {\n    println!(\"Hello, world!\");\n    let x = 10;\n    let y = 20;\n    println!(\"Sum: {}\", x + y);\n}\n";
        let right_sample = "fn main() {\n    println!(\"Hello, GPUI!\");\n    let x = 10;\n    let z = 30;\n    println!(\"Result: {}\", x + z);\n}\n";

        let mut app = Self {
            left_path: None,
            right_path: None,
            left_content: left_sample.to_string(),
            right_content: right_sample.to_string(),
            rows: Vec::new(),
            added_count: 0,
            removed_count: 0,
            scroll_handle: ScrollHandle::new(),
            error_message: None,
        };
        app.recompute_diff();
        app
    }

    fn recompute_diff(&mut self) {
        let diff = TextDiff::from_lines(&self.left_content, &self.right_content);
        let mut rows = Vec::new();
        let mut left_no = 1usize;
        let mut right_no = 1usize;
        let mut added = 0usize;
        let mut removed = 0usize;

        // 将连续的 Delete 和 Insert 配对对齐到同一行，实现 Side-by-Side 视图
        let mut pending_deletes: Vec<(usize, String)> = Vec::new();
        let mut pending_inserts: Vec<(usize, String)> = Vec::new();

        let flush_pending = |rows: &mut Vec<DiffRow>,
                             deletes: &mut Vec<(usize, String)>,
                             inserts: &mut Vec<(usize, String)>| {
            let max_len = deletes.len().max(inserts.len());
            for i in 0..max_len {
                let (l_no, l_txt, l_tag) = if let Some((no, txt)) = deletes.get(i) {
                    (Some(*no), SharedString::from(txt.clone()), Some(ChangeTag::Delete))
                } else {
                    (None, SharedString::from(""), None)
                };

                let (r_no, r_txt, r_tag) = if let Some((no, txt)) = inserts.get(i) {
                    (Some(*no), SharedString::from(txt.clone()), Some(ChangeTag::Insert))
                } else {
                    (None, SharedString::from(""), None)
                };

                rows.push(DiffRow {
                    left_line_no: l_no,
                    left_text: l_txt,
                    left_tag: l_tag,
                    right_line_no: r_no,
                    right_text: r_txt,
                    right_tag: r_tag,
                });
            }
            deletes.clear();
            inserts.clear();
        };

        for change in diff.iter_all_changes() {
            let text = change.value().trim_end_matches(&['\r', '\n'][..]).to_string();
            match change.tag() {
                ChangeTag::Equal => {
                    flush_pending(&mut rows, &mut pending_deletes, &mut pending_inserts);
                    rows.push(DiffRow {
                        left_line_no: Some(left_no),
                        left_text: SharedString::from(text.clone()),
                        left_tag: Some(ChangeTag::Equal),
                        right_line_no: Some(right_no),
                        right_text: SharedString::from(text),
                        right_tag: Some(ChangeTag::Equal),
                    });
                    left_no += 1;
                    right_no += 1;
                }
                ChangeTag::Delete => {
                    removed += 1;
                    pending_deletes.push((left_no, text));
                    left_no += 1;
                }
                ChangeTag::Insert => {
                    added += 1;
                    pending_inserts.push((right_no, text));
                    right_no += 1;
                }
            }
        }
        flush_pending(&mut rows, &mut pending_deletes, &mut pending_inserts);

        self.rows = rows;
        self.added_count = added;
        self.removed_count = removed;
    }

    fn pick_file(&mut self, is_left: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let file = rfd::AsyncFileDialog::new()
                .set_title(if is_left {
                    "选择左侧对比文件 (Original)"
                } else {
                    "选择右侧对比文件 (Modified)"
                })
                .pick_file()
                .await;

            if let Some(file_handle) = file {
                let path = file_handle.path().to_path_buf();
                let read_result = fs::read(&path).map_err(|e| e.to_string());
                let _ = this.update(cx, |app, cx| {
                    match read_result {
                        Ok(bytes) => {
                            let content = String::from_utf8_lossy(&bytes).into_owned();
                            if is_left {
                                app.left_path = Some(path);
                                app.left_content = content;
                            } else {
                                app.right_path = Some(path);
                                app.right_content = content;
                            }
                            app.error_message = None;
                            app.recompute_diff();
                        }
                        Err(e) => {
                            app.error_message = Some(format!("读取失败: {}", e));
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn reload_files(&mut self, cx: &mut Context<Self>) {
        let mut errors = Vec::new();
        if let Some(path) = &self.left_path {
            match fs::read(path) {
                Ok(bytes) => self.left_content = String::from_utf8_lossy(&bytes).into_owned(),
                Err(e) => errors.push(format!("左侧: {}", e)),
            }
        }
        if let Some(path) = &self.right_path {
            match fs::read(path) {
                Ok(bytes) => self.right_content = String::from_utf8_lossy(&bytes).into_owned(),
                Err(e) => errors.push(format!("右侧: {}", e)),
            }
        }
        self.error_message = if errors.is_empty() {
            None
        } else {
            Some(format!("重新加载失败: {}", errors.join("; ")))
        };
        self.recompute_diff();
        cx.notify();
    }

    fn swap_sides(&mut self, cx: &mut Context<Self>) {
        std::mem::swap(&mut self.left_path, &mut self.right_path);
        std::mem::swap(&mut self.left_content, &mut self.right_content);
        self.recompute_diff();
        cx.notify();
    }
}

impl Render for DiffApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let left_label = self
            .left_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "示例文件 A (点击选择文件)".to_string());

        let right_label = self
            .right_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "示例文件 B (点击选择文件)".to_string());

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .font_family("Menlo")
            // 顶部控制栏
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(52.0))
                    .px_4()
                    .bg(rgb(0x181825))
                    .border_b_1()
                    .border_color(rgb(0x313244))
                    // 左侧文件按钮组
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("btn-left")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x313244))
                                    .hover(|s| s.bg(rgb(0x45475a)))
                                    .cursor_pointer()
                                    .text_sm()
                                    .child("选择左侧文件")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pick_file(true, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xa6adc8))
                                    .max_w(px(220.0))
                                    .truncate()
                                    .child(left_label),
                            ),
                    )
                    // 中间操作与统计
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("btn-swap")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x313244))
                                    .hover(|s| s.bg(rgb(0x45475a)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("⇄ 交换")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.swap_sides(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("btn-reload")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x313244))
                                    .hover(|s| s.bg(rgb(0x45475a)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("↻ 重新加载")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reload_files(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .text_xs()
                                    .child(
                                        div()
                                            .text_color(rgb(0xa6e3a1))
                                            .child(format!("+{}", self.added_count)),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(0xf38ba8))
                                            .child(format!("-{}", self.removed_count)),
                                    ),
                            )
                            // 读取文件失败时显示错误提示
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xf38ba8))
                                    .max_w(px(240.0))
                                    .truncate()
                                    .child(self.error_message.clone().unwrap_or_default()),
                            ),
                    )
                    // 右侧文件按钮组
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xa6adc8))
                                    .max_w(px(220.0))
                                    .truncate()
                                    .child(right_label),
                            )
                            .child(
                                div()
                                    .id("btn-right")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x313244))
                                    .hover(|s| s.bg(rgb(0x45475a)))
                                    .cursor_pointer()
                                    .text_sm()
                                    .child("选择右侧文件")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pick_file(false, cx);
                                    })),
                            ),
                    ),
            )
            // 主体 Side-by-Side Diff 滚动区
            .child(
                div()
                    .id("diff-scroll-area")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .py_1()
                    .children(self.rows.iter().enumerate().map(|(idx, row)| {
                        let left_bg = match row.left_tag {
                            Some(ChangeTag::Delete) => rgba(0xf38ba828),
                            None => rgba(0x11111b66),
                            _ => rgba(0x00000000),
                        };
                        let right_bg = match row.right_tag {
                            Some(ChangeTag::Insert) => rgba(0xa6e3a128),
                            None => rgba(0x11111b66),
                            _ => rgba(0x00000000),
                        };

                        let left_prefix = match row.left_tag {
                            Some(ChangeTag::Delete) => "-",
                            _ => " ",
                        };
                        let right_prefix = match row.right_tag {
                            Some(ChangeTag::Insert) => "+",
                            _ => " ",
                        };

                        div()
                            .id(idx)
                            .flex()
                            .w_full()
                            .min_h(px(22.0))
                            .text_xs()
                            // 左半栏
                            .child(
                                div()
                                    .flex()
                                    .w_1_2()
                                    .bg(left_bg)
                                    .border_r_1()
                                    .border_color(rgb(0x313244))
                                    .px_2()
                                    .py(px(2.0))
                                    .child(
                                        div()
                                            .w(px(40.0))
                                            .text_color(rgb(0x6c7086))
                                            .child(
                                                row.left_line_no
                                                    .map(|n| n.to_string())
                                                    .unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .text_color(rgb(0xf38ba8))
                                            .child(left_prefix),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_x_hidden()
                                            .child(row.left_text.clone()),
                                    ),
                            )
                            // 右半栏
                            .child(
                                div()
                                    .flex()
                                    .w_1_2()
                                    .bg(right_bg)
                                    .px_2()
                                    .py(px(2.0))
                                    .child(
                                        div()
                                            .w(px(40.0))
                                            .text_color(rgb(0x6c7086))
                                            .child(
                                                row.right_line_no
                                                    .map(|n| n.to_string())
                                                    .unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .text_color(rgb(0xa6e3a1))
                                            .child(right_prefix),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_x_hidden()
                                            .child(row.right_text.clone()),
                                    ),
                            )
                    })),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, gpui::size(px(1200.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                let view: Entity<DiffApp> = cx.new(|cx| DiffApp::new(cx));
                view
            },
        )
        .unwrap();
    });
}