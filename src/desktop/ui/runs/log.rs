use super::*;

pub(super) struct ScenarioLogScope {
    jobs: BTreeSet<usize>,
    title: String,
}

impl RunPanel {
    fn scenario_jobs(&self, suite: &str, scenario: &str) -> BTreeSet<usize> {
        let output = self
            .executed_suites
            .iter()
            .find(|(name, _)| name == suite)
            .map(|(_, id)| id.as_str())
            .or_else(|| {
                self.suites
                    .iter()
                    .find(|entry| entry.name == suite)
                    .map(|entry| entry.output_id.as_str())
            });
        self.manager
            .progress
            .scenarios
            .values()
            .filter(|entry| {
                entry.job.scenario_id == scenario
                    && output.map_or(entry.job.suite_name == suite, |output| {
                        output == entry.job.suite_id
                    })
            })
            .map(|entry| entry.job.id)
            .collect()
    }

    pub(in crate::desktop::ui) fn has_scenario_log(&self, suite: &str, scenario: &str) -> bool {
        !self.scenario_jobs(suite, scenario).is_empty()
    }

    pub(in crate::desktop::ui) fn show_scenario_log(
        &mut self,
        suite: &str,
        scenario: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let jobs = self.scenario_jobs(suite, scenario);
        let Some(entry) = jobs
            .first()
            .and_then(|id| self.manager.progress.scenarios.get(id))
        else {
            return;
        };
        self.log_scope = Some(ScenarioLogScope {
            title: format!(
                "{} / {}",
                self.suites
                    .iter()
                    .find(|suite| suite.output_id == entry.job.suite_id)
                    .map(|suite| suite.label())
                    .unwrap_or(&entry.job.suite_name),
                entry.job.label
            ),
            jobs,
        });
        self.tab = 3;
        self.refresh_log(window, cx);
        cx.notify();
    }

    pub(super) fn show_job_log(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.manager.progress.scenarios.get(&id) else {
            return;
        };
        self.log_scope = Some(ScenarioLogScope {
            jobs: BTreeSet::from([id]),
            title: format!(
                "{} / {}{}",
                self.suites
                    .iter()
                    .find(|suite| suite.output_id == entry.job.suite_id)
                    .map(|suite| suite.label())
                    .unwrap_or(&entry.job.suite_name),
                entry.job.label,
                if entry.job.viewport.is_empty() {
                    String::new()
                } else {
                    format!(" / {}", entry.job.viewport)
                }
            ),
        });
        self.tab = 3;
        self.refresh_log(window, cx);
        cx.notify();
    }

    pub(super) fn show_all_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.log_scope = None;
        self.tab = 3;
        self.refresh_log(window, cx);
        cx.notify();
    }

    pub(super) fn refresh_log(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = if let Some(scope) = &self.log_scope {
            let progress = &self.manager.progress;
            let mut text = String::new();
            if progress.omitted_logs > 0 {
                text.push_str(&format!(
                    "[{} older scenario log entries omitted from the run's bounded history]\n",
                    progress.omitted_logs
                ));
            }
            for entry in progress
                .logs
                .iter()
                .filter(|entry| scope.jobs.contains(&entry.job_id))
            {
                let viewport = if scope.jobs.len() > 1 {
                    progress
                        .scenarios
                        .get(&entry.job_id)
                        .map(|job| format!(" [{}]", job.job.viewport))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                text.push_str(&format!(
                    "[{:02}:{:02}.{:03}]{viewport} {}\n",
                    entry.elapsed_ms / 60_000,
                    entry.elapsed_ms / 1000 % 60,
                    entry.elapsed_ms % 1000,
                    entry.message
                ));
            }
            if text.is_empty() {
                text.push_str("No retained log entries for this scenario.\n");
            }
            text
        } else {
            let capacity = self
                .manager
                .logs
                .iter()
                .map(|line| line.text.len() + if line.stderr { "[stderr] ".len() } else { 0 })
                .sum();
            let mut text = String::with_capacity(capacity);
            for line in &self.manager.logs {
                if line.stderr {
                    text.push_str("[stderr] ");
                }
                text.push_str(&line.text);
            }
            text
        };
        if text != self.log_text {
            self.log
                .update(cx, |state, cx| state.set_value(text.clone(), window, cx));
            self.log_text = text;
        }
    }

    pub(super) fn render_log(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = self
            .log_scope
            .as_ref()
            .map(|scope| scope.title.clone())
            .unwrap_or_else(|| "All run logs".into());
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("log-activity")
                            .ghost()
                            .small()
                            .icon(IconName::ArrowLeft)
                            .tooltip("Back to Activity")
                            .accessibility_label("Back to Activity")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tab = 0;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("run-log-heading")
                            .role(Role::Heading)
                            .aria_label(title.clone())
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    )
                    .when(self.log_scope.is_some(), |bar| {
                        bar.child(
                            Button::new("log-all")
                                .ghost()
                                .small()
                                .icon(gpui_kit::assets::IconName::List)
                                .label("All run logs")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_all_logs(window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("copy-run-log")
                            .ghost()
                            .small()
                            .icon(IconName::Copy)
                            .tooltip("Copy log")
                            .accessibility_label("Copy log")
                            .disabled(self.log_text.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    this.log_text.clone(),
                                ))
                            })),
                    ),
            )
            .child(
                div()
                    .id("run-log-editor")
                    .size_full()
                    .flex_1()
                    .min_h_0()
                    .child(
                        CodeEditor::new(&self.log)
                            .aria_label(if self.log_scope.is_some() {
                                "Scenario run log"
                            } else {
                                "Run log"
                            })
                            .size_full()
                            .readonly(true),
                    ),
            )
            .into_any_element()
    }
}
