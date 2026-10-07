use super::*;
use crate::desktop::logs::{LogPage, LogQuery, PAGE_RECORDS};

pub(super) struct CaptureLogView {
    pub(super) controls: Entity<super::super::log_controls::LogControls>,
    _subscription: Subscription,
    pub(super) active: bool,
    pub(super) reference: bool,
    pub(super) query: LogQuery,
    pub(super) page: Option<LogPage>,
    pub(super) editor: Entity<EditorState>,
    pub(super) loading: bool,
    pub(super) generation: u64,
    title: String,
}

impl CaptureLogView {
    pub(super) fn new(window: &mut Window, cx: &mut Context<ResultsPanel>) -> Self {
        let controls = cx.new(|cx| super::super::log_controls::LogControls::new(window, cx));
        let subscription = cx.subscribe_in(
            &controls,
            window,
            |this: &mut ResultsPanel, _, event, window, cx| {
                use super::super::log_controls::LogControlEvent;
                match event {
                    LogControlEvent::Filter(query) => {
                        this.capture_log.query = query.clone();
                        this.load_capture_log(window, cx);
                    }
                    LogControlEvent::Follow(_) => {}
                    LogControlEvent::Copy => cx.write_to_clipboard(ClipboardItem::new_string(
                        this.capture_log.editor.read(cx).value().to_string(),
                    )),
                    LogControlEvent::Export(path) => {
                        if let (Some(root), Some(page)) = (&this.root, &this.capture_log.page) {
                            let (root, sources, query, path) = (
                                root.clone(),
                                page.sources.clone(),
                                this.capture_log.query.clone(),
                                path.clone(),
                            );
                            this.capture_log.controls.update(cx, |controls, cx| {
                                controls.export(root, sources, query, path, cx)
                            });
                        }
                    }
                }
                cx.notify();
            },
        );
        Self {
            controls,
            _subscription: subscription,
            active: false,
            reference: false,
            query: Default::default(),
            page: None,
            editor: cx.new(|cx| EditorState::new(window, cx).line_number(false)),
            loading: false,
            generation: 0,
            title: "Capture Log".into(),
        }
    }
}

impl ResultsPanel {
    pub(super) fn load_capture_log(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let Some(test) = self.selected_report().and_then(|report| {
            self.capture
                .and_then(|index| report.report.tests.get(index))
        }) else {
            return;
        };
        let source = test.pair.capture_log(self.capture_log.reference);
        let title = format!(
            "{} / {}",
            if self.capture_log.reference {
                "Reference"
            } else {
                "Test"
            },
            test.pair.display.title
        );
        self.capture_log.title = title;
        self.capture_log.loading = true;
        self.capture_log
            .controls
            .update(cx, |controls, cx| controls.availability(None, false, cx));
        self.capture_log.page = None;
        self.capture_log.generation = self.capture_log.generation.wrapping_add(1);
        let generation = self.capture_log.generation;
        let query = self.capture_log.query.clone();
        self.capture_log.editor.update(cx, |state, cx| {
            state.set_value("Loading capture log...", window, cx)
        });
        cx.spawn_in(window, async move |view, cx| {
            let page = cx
                .background_executor()
                .spawn(async move { crate::desktop::logs::load_page(&root, vec![source], &query) })
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                if this.capture_log.generation != generation {
                    return;
                }
                if let Some(execution) = page
                    .records
                    .iter()
                    .find_map(|record| record.execution.as_ref())
                {
                    this.capture_log.title.push_str(&format!(
                        " / {} x {} / Attempt {}",
                        execution.viewport.width, execution.viewport.height, execution.attempt
                    ));
                }
                this.capture_log
                    .editor
                    .update(cx, |state, cx| state.set_value(page.text(), window, cx));
                this.capture_log.page = Some(page);
                this.capture_log.loading = false;
                let state = this.capture_log.page.as_ref().map(|page| page.state);
                this.capture_log
                    .controls
                    .update(cx, |controls, cx| controls.availability(state, false, cx));
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_capture_log(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("results-capture-log-view")
            .test_support()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(
                div()
                    .id("capture-log-toolbar")
                    .role(Role::Group)
                    .aria_label("Capture log commands")
                    .test_support()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(design::ToolbarFocus::new(
                        "capture-log-toolbar-focus",
                        gpui_kit::component::toolbar::Toolbar::new("capture-log-command-toolbar")
                            .with_size(gpui_kit::component::Size::Medium)
                            .h_auto()
                            .flex_wrap()
                            .child(
                                Button::new("results-log-back")
                                    .ghost()
                                    .icon(IconName::ArrowLeft)
                                    .tooltip("Back to comparison")
                                    .accessibility_label("Back to comparison")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.capture_log.active = false;
                                        cx.notify();
                                    })),
                            )
                            .content(
                                div()
                                    .id("results-log-heading")
                                    .role(Role::Heading)
                                    .aria_label(self.capture_log.title.clone())
                                    .test_support()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .child(self.capture_log.title.clone()),
                            )
                            .child(
                                Button::new("results-log-reference")
                                    .label("Reference")
                                    .selected(self.capture_log.reference)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.capture_log.reference = true;
                                        this.capture_log.query.offset = 0;
                                        this.load_capture_log(window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("results-log-test")
                                    .label("Test")
                                    .selected(!self.capture_log.reference)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.capture_log.reference = false;
                                        this.capture_log.query.offset = 0;
                                        this.load_capture_log(window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("results-log-previous")
                                    .ghost()
                                    .icon(IconName::ArrowLeft)
                                    .tooltip("Previous log page")
                                    .accessibility_label("Previous log page")
                                    .disabled(
                                        self.capture_log.loading
                                            || self.capture_log.query.offset == 0,
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.capture_log.query.offset = this
                                            .capture_log
                                            .query
                                            .offset
                                            .saturating_sub(PAGE_RECORDS);
                                        this.load_capture_log(window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("results-log-next")
                                    .ghost()
                                    .icon(IconName::ArrowRight)
                                    .tooltip("Next log page")
                                    .accessibility_label("Next log page")
                                    .disabled(
                                        self.capture_log.loading
                                            || !self
                                                .capture_log
                                                .page
                                                .as_ref()
                                                .is_some_and(LogPage::has_more),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.capture_log.query.offset += PAGE_RECORDS;
                                        this.load_capture_log(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )),
            )
            .child(self.capture_log.controls.clone())
            .child(
                div().flex_1().min_h_0().child(
                    CodeEditor::new(&self.capture_log.editor)
                        .aria_label("Capture log")
                        .readonly(true)
                        .size_full(),
                ),
            )
            .into_any_element()
    }

    #[cfg(test)]
    pub(in crate::desktop::ui) fn capture_log_text(&self, cx: &App) -> String {
        self.capture_log.editor.read(cx).value().to_string()
    }

    #[cfg(test)]
    pub(in crate::desktop::ui) fn log_controls_handle(
        &self,
    ) -> Entity<super::super::log_controls::LogControls> {
        self.capture_log.controls.clone()
    }
}
