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
        self.log_query.offset = 0;
        self.archive_key = None;
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
        self.log_query.offset = 0;
        self.archive_key = None;
        self.tab = 3;
        self.refresh_log(window, cx);
        cx.notify();
    }

    pub(super) fn show_all_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.log_scope = None;
        self.log_query.offset = 0;
        self.archive_key = None;
        self.tab = 3;
        self.refresh_log(window, cx);
        cx.notify();
    }

    pub(super) fn refresh_log(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.running()
            && self.manager.request.is_some()
            && (self.tab == 3
                || self
                    .manager
                    .request
                    .as_ref()
                    .is_some_and(|request| request.command == RunCommand::Deploy))
        {
            self.refresh_archive(window, cx);
            return;
        }
        self.archive_key = None;
        self.archive_page = None;
        self.archive_loading = false;
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

    #[cfg(test)]
    pub(in crate::desktop::ui) fn rendered_log(&self) -> &str {
        &self.log_text
    }

    fn refresh_archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(request) = self.manager.request.clone() else {
            return;
        };
        let jobs = self
            .manager
            .progress
            .scenarios
            .values()
            .filter(|entry| {
                self.log_scope
                    .as_ref()
                    .is_none_or(|scope| scope.jobs.contains(&entry.job.id))
            })
            .map(|entry| entry.job.clone())
            .collect::<Vec<_>>();
        let query = self.log_query.clone();
        let key = serde_json::to_string(&(&request, &jobs, &query)).unwrap_or_default();
        if self.archive_key.as_ref() == Some(&key) {
            return;
        }
        self.archive_key = Some(key.clone());
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.archive_page = None;
        self.archive_loading = true;
        self.log_text = "Loading archived log...".into();
        self.log.update(cx, |state, cx| {
            state.set_value(self.log_text.clone(), window, cx)
        });
        cx.spawn_in(window, async move |view, cx| {
            let page = cx
                .background_executor()
                .spawn(async move { crate::desktop::logs::load_run_page(&request, &jobs, &query) })
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                if this.archive_key.as_ref() != Some(&key) || this.archive_generation != generation
                {
                    return;
                }
                this.archive_loading = false;
                this.log_text = page.text();
                this.log.update(cx, |state, cx| {
                    state.set_value(this.log_text.clone(), window, cx)
                });
                this.archive_page = Some(page);
                cx.notify();
            });
        })
        .detach();
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
                            .tooltip(if self.archive_page.is_some() {
                                "Copy displayed log page"
                            } else {
                                "Copy log"
                            })
                            .accessibility_label(if self.archive_page.is_some() {
                                "Copy displayed log page"
                            } else {
                                "Copy log"
                            })
                            .disabled(self.log_text.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    this.log_text.clone(),
                                ))
                            })),
                    )
                    .when_some(self.archive_page.as_ref(), |bar, page| {
                        bar.child(
                            Button::new("log-previous-page")
                                .ghost()
                                .small()
                                .icon(IconName::ArrowLeft)
                                .tooltip("Previous log page")
                                .accessibility_label("Previous log page")
                                .disabled(page.offset == 0)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.log_query.offset = this
                                        .log_query
                                        .offset
                                        .saturating_sub(crate::desktop::logs::PAGE_RECORDS);
                                    this.archive_key = None;
                                    this.refresh_log(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(div().text_xs().child(format!("{} records", page.matched)))
                        .child(
                            Button::new("log-next-page")
                                .ghost()
                                .small()
                                .icon(IconName::ArrowRight)
                                .tooltip("Next log page")
                                .accessibility_label("Next log page")
                                .disabled(!page.has_more())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.log_query.offset += crate::desktop::logs::PAGE_RECORDS;
                                    this.archive_key = None;
                                    this.refresh_log(window, cx);
                                    cx.notify();
                                })),
                        )
                    }),
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
