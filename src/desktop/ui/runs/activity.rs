use super::*;
use crate::progress::{Counts, Status};

pub(crate) fn status_color(status: Status, cx: &App) -> Hsla {
    match status {
        Status::Running | Status::Comparing => cx.theme().primary,
        Status::Passed | Status::Captured => cx.theme().success,
        Status::Failed => cx.theme().danger,
        Status::Cancelled => cx.theme().warning,
        Status::Queued => cx.theme().muted_foreground,
    }
}

impl RunPanel {
    fn outcome_heading(&self) -> String {
        let comparisons = self.manager.progress.comparison_counts(None);
        let counts = if comparisons.total > 0 {
            comparisons
        } else {
            self.manager.progress.counts(None)
        };
        match &self.manager.outcome {
            Some(RunOutcome::Cancelled) => "Run stopped".into(),
            Some(RunOutcome::Failed(_)) => "Command failed".into(),
            Some(RunOutcome::Completed) if counts.failed > 0 => {
                format!("{} failed, {} passed", counts.failed, counts.passed)
            }
            Some(RunOutcome::Completed) if counts.captured > 0 => {
                format!("{} captured, not compared", counts.captured)
            }
            Some(RunOutcome::Completed) if counts.passed > 0 => format!("{} passed", counts.passed),
            Some(RunOutcome::Completed) => {
                match self.manager.request.as_ref().map(|request| request.command) {
                    Some(RunCommand::Approve) => "Reference images updated".into(),
                    Some(RunCommand::Init) => "Workspace initialized".into(),
                    Some(RunCommand::Install) => "Chromium is ready".into(),
                    Some(RunCommand::Snapshot) => "Report created".into(),
                    Some(RunCommand::Optimize) => "Image optimization completed".into(),
                    _ => "Command completed".into(),
                }
            }
            None if self.running() => self
                .manager
                .request
                .as_ref()
                .map(|request| format!("{} in progress", request.command.action_label()))
                .unwrap_or_else(|| "Run in progress".into()),
            None if self.selection.is_empty() => "Select suites to run".into(),
            None => "Ready to run".into(),
        }
    }

    pub(super) fn render_activity(&self, cx: &mut Context<Self>) -> AnyElement {
        let progress = &self.manager.progress;
        let counts = progress.counts(None);
        let comparisons = progress.comparison_counts(None);
        let results = if comparisons.total > 0 {
            comparisons
        } else {
            counts
        };
        let has_run = self.manager.request.is_some() || !progress.scenarios.is_empty();
        let phase = if has_run {
            if progress.phase.is_empty() {
                "Starting engine"
            } else {
                &progress.phase
            }
        } else {
            "Ready"
        };
        let elapsed = if self.running() {
            self.started
                .map(|started| started.elapsed().as_millis() as u64)
                .unwrap_or(0)
        } else {
            progress.elapsed_ms
        };
        let selected_scenarios = self.selected_scenario_count();
        let mut content =
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .w_full()
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .children(self.manager.request.as_ref().map(|request| {
                                    let label = format!(
                                        "{}: {}",
                                        if self.running() {
                                            "Running"
                                        } else {
                                            "Last run"
                                        },
                                        request.command.action_label()
                                    );
                                    div()
                                        .id("run-command-context")
                                        .role(Role::Status)
                                        .aria_label(label.clone())
                                        .test_support()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(label)
                                }))
                                .child(
                                    div()
                                        .id("run-outcome-heading")
                                        .role(Role::Status)
                                        .aria_label(self.outcome_heading())
                                        .test_support()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(match &self.manager.outcome {
                                            Some(RunOutcome::Failed(_)) => cx.theme().danger,
                                            Some(RunOutcome::Cancelled) => cx.theme().warning,
                                            Some(_) if counts.failed > 0 => cx.theme().danger,
                                            _ => cx.theme().foreground,
                                        })
                                        .child(self.outcome_heading()),
                                )
                                .child(
                                    div()
                                        .id("live-phase")
                                        .role(Role::Status)
                                        .aria_label(phase.to_string())
                                        .test_support()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(phase.to_string()),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_family(cx.theme().mono_font_family.clone())
                                .child(format!(
                                    "{:02}:{:02}",
                                    elapsed / 60_000,
                                    elapsed / 1000 % 60
                                )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .gap_2()
                        .when(!has_run && !self.selection.is_empty(), |row| {
                            row.child(
                                Button::new("activity-select-suites")
                                    .small()
                                    .icon(gpui_kit::assets::IconName::ListChecks)
                                    .label("Select suites")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.tab = 1;
                                        cx.notify();
                                    })),
                            )
                        })
                        .when(has_run, |row| {
                            row.child(
                                Button::new("activity-view-log")
                                    .ghost()
                                    .small()
                                    .icon(gpui_kit::assets::IconName::Terminal)
                                    .label("View Log")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.show_all_logs(window, cx)
                                    })),
                            )
                        })
                        .when(
                            matches!(self.manager.outcome, Some(RunOutcome::Completed))
                                && (counts.captured > 0
                                    || self.manager.request.as_ref().is_some_and(|request| {
                                        request.command == RunCommand::Approve
                                    })),
                            |row| {
                                row.child(
                                    Button::new("activity-compare-run")
                                        .small()
                                        .icon(gpui_kit::assets::IconName::Images)
                                        .label("Compare This Run")
                                        .disabled(self.disabled)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.compare_completed_run(false, cx)
                                        })),
                                )
                            },
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .border_y_1()
                        .border_color(cx.theme().border)
                        .py_2()
                        .gap_4()
                        .child(self.metric(
                            "total",
                            if comparisons.total > 0 {
                                "Screenshot comparisons"
                            } else {
                                "Scenarios / viewports"
                            },
                            if has_run {
                                results.total
                            } else {
                                selected_scenarios
                            },
                            cx.theme().foreground,
                        ))
                        .child(self.metric("active", "Running", counts.running, cx.theme().primary))
                        .child(self.metric("passed", "Passed", results.passed, cx.theme().success))
                        .child(self.metric("failed", "Failed", results.failed, cx.theme().danger)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} / {} scenario jobs finished",
                                    counts.complete(),
                                    counts.total
                                ))
                                .child(format!(
                                    "{} queued  |  {} captured  |  {} cancelled",
                                    counts.queued, counts.captured, counts.cancelled
                                )),
                        )
                        .child(
                            div()
                                .h(px(5.))
                                .w_full()
                                .bg(cx.theme().muted)
                                .rounded_sm()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(counts.fraction()))
                                        .bg(if counts.failed > 0 {
                                            cx.theme().danger
                                        } else if counts.cancelled > 0 {
                                            cx.theme().warning
                                        } else if counts.captured > 0 && counts.passed == 0 {
                                            cx.theme().info
                                        } else {
                                            cx.theme().primary
                                        })
                                        .rounded_sm(),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(div().font_weight(FontWeight::MEDIUM).child(if has_run {
                            "Suite activity"
                        } else {
                            "Selected suites"
                        }))
                        .child(
                            Checkbox::new("activity-failed-only")
                                .label("Failures only")
                                .checked(self.failed_only)
                                .disabled(!has_run)
                                .on_change(cx.listener(|this, checked, _, cx| {
                                    this.failed_only = *checked;
                                    cx.notify();
                                })),
                        ),
                );
        if let Some(error) = &progress.error {
            content = content.child(
                div()
                    .px_3()
                    .py_2()
                    .bg(cx.theme().danger.opacity(0.08))
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        }
        if has_run {
            for suite in progress.suites.values() {
                let counts = progress.counts(Some(&suite.id));
                if self.failed_only && counts.failed == 0 {
                    continue;
                }
                let expanded = !self.collapsed.contains(&suite.id);
                let state = suite_status(counts, suite.finished || progress.finished);
                content = content.child(self.suite_header(
                    &suite.id,
                    &suite.name,
                    counts,
                    state,
                    expanded,
                    cx,
                ));
                if expanded {
                    for scenario in progress
                        .scenarios
                        .values()
                        .filter(|scenario| scenario.job.suite_id == suite.id)
                    {
                        if self.failed_only && scenario.status != Status::Failed {
                            continue;
                        }
                        let duration = scenario.started_ms.map(|start| {
                            scenario
                                .finished_ms
                                .unwrap_or(elapsed)
                                .saturating_sub(start)
                        });
                        let status = scenario.status;
                        let job_id = scenario.job.id;
                        content = content.child(
                            Button::new(("live-row", scenario.job.id))
                                .ghost()
                                .w_full()
                                .h_auto()
                                .justify_start()
                                .accessibility_label(format!(
                                    "View log: {}{}",
                                    scenario.job.label,
                                    if scenario.job.viewport.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" ({})", scenario.job.viewport)
                                    }
                                ))
                                .tooltip("View scenario run log")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.show_job_log(job_id, window, cx)
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w_full()
                                        .min_w_0()
                                        .text_left()
                                        .gap_1()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .pb_3()
                                        .pl_4()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .size_2()
                                                        .rounded_full()
                                                        .bg(status_color(status, cx)),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .truncate()
                                                        .child(scenario.job.label.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(100.))
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .truncate()
                                                        .child(scenario.job.viewport.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(64.))
                                                        .text_right()
                                                        .font_family(
                                                            cx.theme().mono_font_family.clone(),
                                                        )
                                                        .text_xs()
                                                        .child(
                                                            duration
                                                                .map(|duration| {
                                                                    format!(
                                                                        "{:.1}s",
                                                                        duration as f64 / 1000.0
                                                                    )
                                                                })
                                                                .unwrap_or_else(|| "-".into()),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .id(("live-status", scenario.job.id))
                                                        .role(Role::Status)
                                                        .aria_label(status.label())
                                                        .test_support()
                                                        .w(px(90.))
                                                        .text_right()
                                                        .text_xs()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(status_color(status, cx))
                                                        .child(status.label()),
                                                ),
                                        )
                                        .children(scenario.message.as_ref().map(|message| {
                                            div()
                                                .pl_5()
                                                .text_xs()
                                                .text_color(if scenario.infrastructure {
                                                    cx.theme().danger
                                                } else {
                                                    status_color(status, cx)
                                                })
                                                .child(if scenario.infrastructure {
                                                    format!("Execution error: {message}")
                                                } else {
                                                    message.clone()
                                                })
                                        })),
                                ),
                        );
                    }
                }
            }
        } else {
            for suite in self
                .suites
                .iter()
                .filter(|suite| self.selection.contains(&suite.name))
            {
                content = content.child(self.suite_header(
                    &suite.output_id,
                    &suite.name,
                    Counts {
                        total: self.selection.scenario_count(suite),
                        queued: self.selection.scenario_count(suite),
                        ..Default::default()
                    },
                    Status::Queued,
                    !self.collapsed.contains(&suite.output_id),
                    cx,
                ));
                if !self.collapsed.contains(&suite.output_id) {
                    for scenario in suite
                        .scenarios
                        .iter()
                        .filter(|scenario| self.scenario_selected(&suite.name, &scenario.id))
                    {
                        content = content.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .pl_5()
                                .pb_2()
                                .child(
                                    Icon::new(gpui_kit::assets::IconName::Circle)
                                        .small()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .child(scenario.label.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Ready"),
                                ),
                        );
                    }
                }
            }
            if self.selection.is_empty() {
                content = content.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .py_10()
                        .child(
                            Icon::new(gpui_kit::assets::IconName::ListChecks)
                                .size_8()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child("No suites selected"),
                        )
                        .child(
                            Button::new("activity-select-suites")
                                .label("Select suites")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.tab = 1;
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        div()
            .id("activity-scroll")
            .size_full()
            .overflow_y_scrollbar()
            .child(content)
            .into_any_element()
    }

    fn metric(
        &self,
        id: &'static str,
        label: &'static str,
        value: usize,
        color: Hsla,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w(px(100.))
            .flex_1()
            .child(
                div()
                    .id(SharedString::from(format!("metric-{id}")))
                    .role(Role::Status)
                    .aria_label(format!("{label}: {value}"))
                    .test_support()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(value.to_string()),
            )
            .child(div().text_xs().child(label))
            .into_any_element()
    }

    fn suite_header(
        &self,
        id: &str,
        name: &str,
        counts: Counts,
        status: Status,
        expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = id.to_string();
        div()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .py_2()
            .bg(cx.theme().muted.opacity(0.5))
            .child(
                Button::new(SharedString::from(format!("toggle-suite-{id}")))
                    .ghost()
                    .xsmall()
                    .icon(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .tooltip("Expand or collapse suite")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.collapsed.remove(&id) {
                            this.collapsed.insert(id.clone());
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} / {}", counts.complete(), counts.total)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(status_color(status, cx))
                    .child(status.label()),
            )
            .into_any_element()
    }
}

fn suite_status(counts: Counts, finished: bool) -> Status {
    if counts.running > 0 {
        Status::Running
    } else if counts.queued > 0 && !finished {
        Status::Queued
    } else if counts.failed > 0 {
        Status::Failed
    } else if counts.cancelled > 0 {
        Status::Cancelled
    } else if counts.passed > 0 || counts.total == 0 && finished {
        Status::Passed
    } else {
        Status::Captured
    }
}
