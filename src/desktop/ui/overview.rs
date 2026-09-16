use super::*;

impl Workbench {
    pub(super) fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let root = self
            .session
            .workspace
            .as_ref()
            .map(|workspace| workspace.root.display().to_string())
            .unwrap_or_else(|| "No workspace open".into());
        let counts = self.runs.read(cx).progress().counts(None);
        div()
            .flex()
            .items_center()
            .gap_4()
            .px_5()
            .h(px(66.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .size(px(32.))
                    .rounded_md()
                    .bg(cx.theme().primary)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(gpui_kit::assets::IconName::Layers)
                            .size_5()
                            .text_color(cx.theme().primary_foreground),
                    ),
            )
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_lg()
                    .child("Regressify"),
            )
            .child(div().w(px(1.)).h_6().bg(cx.theme().border))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("ACTIVE WORKSPACE"),
                    )
                    .child(div().truncate().text_sm().child(root)),
            )
            .when(self.runs.read(cx).running(), |header| {
                header.child(
                    Button::new("view-running")
                        .ghost()
                        .small()
                        .icon(gpui_kit::assets::IconName::Activity)
                        .label(format!(
                            "{} running / {} finished",
                            counts.running,
                            counts.complete()
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.main_tab = 1;
                            cx.notify();
                        })),
                )
            })
            .child(self.file_menu())
            .child(
                Button::new("open-workspace")
                    .ghost()
                    .icon(IconName::FolderOpen)
                    .label(if self.session.workspace.is_some() {
                        "Switch workspace"
                    } else {
                        "Open workspace"
                    })
                    .disabled(self.busy || self.runs.read(cx).running())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.choose_workspace(&OpenWorkspace, window, cx)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_overview(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(workspace) = &self.session.workspace else {
            return div()
                .flex()
                .flex_col()
                .size_full()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    Icon::new(gpui_kit::assets::IconName::Folders)
                        .size(px(48.))
                        .text_color(cx.theme().primary),
                )
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("No workspace open"),
                )
                .child(
                    Button::new("overview-open")
                        .primary()
                        .icon(IconName::FolderOpen)
                        .label("Open workspace")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.choose_workspace(&OpenWorkspace, window, cx)
                        })),
                )
                .into_any_element();
        };
        let name = workspace
            .root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let scenarios = self
            .session
            .suites
            .iter()
            .map(|suite| suite.scenarios.len())
            .sum::<usize>();
        let invalid = self
            .session
            .suites
            .iter()
            .filter(|suite| suite.error.is_some())
            .count();
        let mut page = div()
            .flex()
            .flex_col()
            .gap_5()
            .p_6()
            .w_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("WORKSPACE"),
                            )
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(name),
                            ),
                    )
                    .child(
                        Button::new("overview-new-suite")
                            .primary()
                            .icon(IconName::Plus)
                            .label("New suite")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.guard(files::Pending::Name(files::Operation::New), window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_6()
                    .border_y_1()
                    .border_color(cx.theme().border)
                    .py_4()
                    .child(self.overview_metric(
                        "Test suites",
                        self.session.suites.len(),
                        cx.theme().foreground,
                    ))
                    .child(self.overview_metric("Scenarios", scenarios, cx.theme().foreground))
                    .child(self.overview_metric(
                        "Selected",
                        self.runs.read(cx).selected_count(),
                        cx.theme().primary,
                    ))
                    .child(self.overview_metric(
                        "Config issues",
                        invalid,
                        if invalid == 0 {
                            cx.theme().muted_foreground
                        } else {
                            cx.theme().danger
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Test suites"))
                    .child(
                        Button::new("overview-run-selected")
                            .primary()
                            .icon(gpui_kit::assets::IconName::Play)
                            .label(format!(
                                "Run selected ({})",
                                self.runs.read(cx).selected_count()
                            ))
                            .disabled(
                                self.busy
                                    || self.runs.read(cx).selected_count() == 0
                                    || self.runs.read(cx).running(),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.main_tab = 1;
                                this.runs.update(cx, |panel, cx| {
                                    panel.choose(
                                        crate::desktop::runner::RunCommand::Test,
                                        window,
                                        cx,
                                    )
                                });
                                this.guard(files::Pending::Run, window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .bg(cx.theme().muted)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(px(24.)))
                    .child(div().flex_1().child("SUITE"))
                    .child(div().w(px(90.)).child("SCENARIOS"))
                    .child(div().w(px(100.)).child("STATUS"))
                    .child(div().w(px(80.))),
            );
        let query = self.search.read(cx).value().to_lowercase();
        for (index, suite) in self.session.suites.iter().enumerate() {
            if !query.is_empty()
                && !suite.name.to_lowercase().contains(&query)
                && !suite
                    .scenarios
                    .iter()
                    .any(|scenario| scenario.label.to_lowercase().contains(&query))
            {
                continue;
            }
            let entry = suite.entry.clone();
            let name = suite.name.clone();
            let counts = self.runs.read(cx).progress().counts(Some(&suite.output_id));
            let status = if suite.error.is_some() {
                "Invalid"
            } else if counts.running > 0 {
                "Running"
            } else if counts.failed > 0 {
                "Failed"
            } else if counts.passed > 0 {
                "Passed"
            } else {
                "Ready"
            };
            let color = match status {
                "Invalid" | "Failed" => cx.theme().danger,
                "Running" => cx.theme().primary,
                "Passed" => cx.theme().success,
                _ => cx.theme().muted_foreground,
            };
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .pb_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_3()
                            .child(
                                Checkbox::new(("overview-check", index))
                                    .checked(self.runs.read(cx).selected(&name))
                                    .disabled(
                                        self.busy
                                            || self.runs.read(cx).running()
                                            || suite.error.is_some(),
                                    )
                                    .on_change(cx.listener(move |this, checked, _, cx| {
                                        this.runs.update(cx, |panel, cx| {
                                            panel.toggle_suite(&name, *checked, cx)
                                        })
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(suite.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .truncate()
                                            .child(suite.entry.label()),
                                    ),
                            )
                            .child(div().w(px(90.)).child(suite.scenarios.len().to_string()))
                            .child(div().w(px(100.)).text_xs().text_color(color).child(status))
                            .child(
                                Button::new(("overview-edit", index))
                                    .ghost()
                                    .small()
                                    .icon(gpui_kit::assets::IconName::Pencil)
                                    .label("Edit")
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.load_document(entry.clone(), window, cx)
                                    })),
                            ),
                    )
                    .children(suite.error.as_ref().map(|error| {
                        div()
                            .px_3()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(error.clone())
                    })),
            );
        }
        if let Some(error) = &self.session.suite_error {
            page = page.child(div().text_color(cx.theme().danger).child(error.clone()));
        }
        if self.session.suites.is_empty() {
            page = page.child(
                div()
                    .py_8()
                    .text_color(cx.theme().muted_foreground)
                    .child("No test suites"),
            );
        }
        div()
            .id("workspace-overview-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(page)
            .into_any_element()
    }

    fn overview_metric(&self, label: &'static str, count: usize, color: Hsla) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(100.))
            .gap_1()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(count.to_string()),
            )
            .child(div().text_xs().child(label))
            .into_any_element()
    }
}
