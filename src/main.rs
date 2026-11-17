use gpui::{
    div, prelude::*, px, rgb, rgba, App, Application, Bounds, Context, Entity,
    ScrollHandle, SharedString, Window, WindowBounds, WindowOptions,
};
use gpui_component::{
    input::{Input, InputState},
    Root,
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
    left_path_input: Entity<InputState>,
    right_path_input: Entity<InputState>,
    left_text_input: Entity<InputState>,
    right_text_input: Entity<InputState>,
    show_text_inputs: bool,
    rows: Vec<DiffRow>,
    added_count: usize,
    removed_count: usize,
    scroll_handle: ScrollHandle,
    error_message: Option<String>,
}

impl DiffApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let left_sample = "fn main() {\n    println!(\"Hello, world!\");\n    let x = 10;\n    let y = 20;\n    println!(\"Sum: {}\", x + y);\n}\n";
        let right_sample = "fn main() {\n    println!(\"Hello, GPUI!\");\n    let x = 10;\n    let z = 30;\n    println!(\"Result: {}\", x + z);\n}\n";

        let mut app = Self {
            left_path: None,
            right_path: None,
            left_content: left_sample.to_string(),
            right_content: right_sample.to_string(),
            left_path_input: cx
                .new(|cx| InputState::new(window, cx).placeholder("输入左侧文件路径")),
            right_path_input: cx
                .new(|cx| InputState::new(window, cx).placeholder("输入右侧文件路径")),
            left_text_input: cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line()
                    .rows(4)
                    .placeholder("粘贴原始文本")
            }),
            right_text_input: cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line()
                    .rows(4)
                    .placeholder("粘贴修改后的文本")
            }),
            show_text_inputs: false,
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
                let _ = this.update(cx, |app, cx| {
                    app.load_file(path, is_left, cx);
                });
            }
        })
        .detach();
    }

    fn open_typed_path(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let value = if is_left {
            self.left_path_input.read(cx).value().to_string()
        } else {
            self.right_path_input.read(cx).value().to_string()
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            self.error_message = Some("请输入文件路径".to_string());
            cx.notify();
            return;
        }
        self.load_file(PathBuf::from(value), is_left, cx);
    }

    fn load_file(&mut self, path: PathBuf, is_left: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let read_result = fs::read(&path).map_err(|error| error.to_string());
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
                    Err(error) => {
                        app.error_message = Some(format!("读取文件失败: {}", error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn compare_pasted_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.left_content = self.left_text_input.read(cx).value().to_string();
        self.right_content = self.right_text_input.read(cx).value().to_string();
        self.left_path = None;
        self.right_path = None;
        self.left_path_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.right_path_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.error_message = None;
        self.recompute_diff();
        cx.notify();
    }

    fn clear_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.left_path = None;
        self.right_path = None;
        self.left_content.clear();
        self.right_content.clear();
        self.error_message = None;

        self.left_path_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.right_path_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.left_text_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.right_text_input
            .update(cx, |state, cx| state.set_value("", window, cx));

        self.recompute_diff();
        cx.notify();
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
            .unwrap_or_else(|| {
                if self.left_content.is_empty() {
                    "未载入".to_string()
                } else {
                    "文本内容".to_string()
                }
            });

        let right_label = self
            .right_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                if self.right_content.is_empty() {
                    "未载入".to_string()
                } else {
                    "文本内容".to_string()
                }
            });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0xf5f5f7))
            .text_color(rgb(0x1d1d1f))
            .font_family("SF Pro")
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(48.0))
                    .px_5()
                    .bg(rgb(0xf5f5f7))
                    .border_b_1()
                    .border_color(rgb(0xdcdce0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).child("文件对比"))
                            .child(div().text_xs().text_color(rgb(0x86868b)).child("左右并排"))
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("btn-toggle-text-input")
                                    .px_2()
                                    .py(px(5.0))
                                    .rounded_lg()
                                    .bg(rgb(0xe8e8ed))
                                    .hover(|s| s.bg(rgb(0xdedee3)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child(if self.show_text_inputs {
                                        "收起文本"
                                    } else {
                                        "粘贴文本"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_text_inputs = !this.show_text_inputs;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("btn-swap")
                                    .px_2()
                                    .py(px(5.0))
                                    .rounded_lg()
                                    .bg(rgb(0xe8e8ed))
                                    .hover(|s| s.bg(rgb(0xdedee3)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("⇄ 交换两侧")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.swap_sides(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("btn-reload")
                                    .px_2()
                                    .py(px(5.0))
                                    .rounded_lg()
                                    .bg(rgb(0xe8e8ed))
                                    .hover(|s| s.bg(rgb(0xdedee3)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("↻ 重新载入")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reload_files(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("btn-clear")
                                    .px_2()
                                    .py(px(5.0))
                                    .rounded_lg()
                                    .bg(rgb(0xffe9e7))
                                    .hover(|s| s.bg(rgb(0xffd9d5)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(rgb(0xc9342f))
                                    .child("清空")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.clear_all(window, cx);
                                    })),
                            )
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .child(
                                div()
                                    .px_2()
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(rgb(0xe5f5e9))
                                    .text_color(rgb(0x248a3d))
                                    .child(format!("+ {}", self.added_count)),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(rgb(0xffe9e7))
                                    .text_color(rgb(0xc9342f))
                                    .child(format!("- {}", self.removed_count)),
                            ),
                    ),
            )
            .when(self.show_text_inputs, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .px_5()
                        .py_3()
                        .bg(rgb(0xf0f0f2))
                        .border_b_1()
                        .border_color(rgb(0xdcdce0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x636366))
                                        .child("粘贴两侧文本，然后比较"),
                                )
                                .child(
                                    div()
                                        .id("btn-compare-text")
                                        .px_3()
                                        .py(px(6.0))
                                        .rounded_lg()
                                        .bg(rgb(0x007aff))
                                        .hover(|s| s.bg(rgb(0x0066d6)))
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(rgb(0xffffff))
                                        .child("比较文本")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.compare_pasted_text(window, cx);
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(
                                    div()
                                        .flex_1()
                                        .child(Input::new(&self.left_text_input).h(px(112.0))),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .child(Input::new(&self.right_text_input).h(px(112.0))),
                                ),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .h(px(64.0))
                    .px_5()
                    .bg(rgb(0xf0f0f2))
                    .border_b_1()
                    .border_color(rgb(0xdcdce0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w(px(560.0))
                            .child(
                                div()
                                    .px_2()
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(rgb(0xe5e5ea))
                                    .text_xs()
                                    .text_color(rgb(0x636366))
                                    .child("A"),
                            )
                            .child(
                                Input::new(&self.left_path_input)
                                    .w(px(370.0))
                                    .h(px(34.0)),
                            )
                            .child(
                                div()
                                    .id("btn-left")
                                    .px_2()
                                    .py(px(6.0))
                                    .rounded_lg()
                                    .bg(rgb(0xe5e5ea))
                                    .hover(|s| s.bg(rgb(0xd8d8dd)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("选择")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pick_file(true, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("btn-open-left-path")
                                    .px_3()
                                    .py(px(6.0))
                                    .rounded_lg()
                                    .bg(rgb(0x007aff))
                                    .hover(|s| s.bg(rgb(0x0066d6)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .child("打开")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_typed_path(true, cx);
                                    })),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w(px(560.0))
                            .child(
                                div()
                                    .px_2()
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(rgb(0xe5e5ea))
                                    .text_xs()
                                    .text_color(rgb(0x636366))
                                    .child("B"),
                            )
                            .child(
                                Input::new(&self.right_path_input)
                                    .w(px(370.0))
                                    .h(px(34.0)),
                            )
                            .child(
                                div()
                                    .id("btn-right")
                                    .px_2()
                                    .py(px(6.0))
                                    .rounded_lg()
                                    .bg(rgb(0xe5e5ea))
                                    .hover(|s| s.bg(rgb(0xd8d8dd)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .child("选择")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pick_file(false, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("btn-open-right-path")
                                    .px_3()
                                    .py(px(6.0))
                                    .rounded_lg()
                                    .bg(rgb(0x007aff))
                                    .hover(|s| s.bg(rgb(0x0066d6)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .child("打开")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_typed_path(false, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(38.0))
                    .bg(rgb(0xffffff))
                    .border_b_1()
                    .border_color(rgb(0xe5e5e8))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_1_2()
                            .h_full()
                            .px_5()
                            .border_r_1()
                            .border_color(rgb(0xe5e5e8))
                            .child(
                                div()
                                    .w(px(7.0))
                                    .h(px(7.0))
                                    .rounded_full()
                                    .bg(rgb(0x8e8e93)),
                            )
                            .child(div().text_xs().text_color(rgb(0x636366)).child("原始文件"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x1d1d1f))
                                    .max_w(px(420.0))
                                    .truncate()
                                    .child(left_label),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_1_2()
                            .h_full()
                            .px_5()
                            .child(
                                div()
                                    .w(px(7.0))
                                    .h(px(7.0))
                                    .rounded_full()
                                    .bg(rgb(0x007aff)),
                            )
                            .child(div().text_xs().text_color(rgb(0x636366)).child("修改后"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x1d1d1f))
                                    .max_w(px(420.0))
                                    .truncate()
                                    .child(right_label),
                            ),
                    ),
            )
            .child(
                div()
                    .id("diff-scroll-area")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .bg(rgb(0xffffff))
                    .children(self.rows.iter().enumerate().map(|(idx, row)| {
                        let left_bg = match row.left_tag {
                            Some(ChangeTag::Delete) => rgba(0xff3b3018),
                            None => rgba(0x8e8e9308),
                            _ => rgba(0x00000000),
                        };
                        let right_bg = match row.right_tag {
                            Some(ChangeTag::Insert) => rgba(0x34c75918),
                            None => rgba(0x8e8e9308),
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
                            .child(
                                div()
                                    .flex()
                                    .w_1_2()
                                    .bg(left_bg)
                                    .border_r_1()
                                    .border_color(rgb(0xe5e5e8))
                                    .px_2()
                                    .py(px(2.0))
                                    .child(
                                        div()
                                            .w(px(40.0))
                                            .text_color(rgb(0x8e8e93))
                                            .child(
                                                row.left_line_no
                                                    .map(|n| n.to_string())
                                                    .unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .text_color(rgb(0xc9342f))
                                            .child(left_prefix),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_x_hidden()
                                            .font_family("SF Mono")
                                            .child(row.left_text.clone()),
                                    ),
                            )
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
                                            .text_color(rgb(0x8e8e93))
                                            .child(
                                                row.right_line_no
                                                    .map(|n| n.to_string())
                                                    .unwrap_or_default(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .text_color(rgb(0x248a3d))
                                            .child(right_prefix),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_x_hidden()
                                            .font_family("SF Mono")
                                            .child(row.right_text.clone()),
                                    ),
                            )
                    })),
            )
            .when(self.error_message.is_some(), |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .h(px(32.0))
                        .px_5()
                        .bg(rgb(0xffe9e7))
                        .text_xs()
                        .text_color(rgb(0xc9342f))
                        .child(self.error_message.clone().unwrap_or_default()),
                )
            })
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);
        let bounds = Bounds::centered(None, gpui::size(px(1200.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view: Entity<DiffApp> = cx.new(|cx| DiffApp::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .unwrap();
    });
}
