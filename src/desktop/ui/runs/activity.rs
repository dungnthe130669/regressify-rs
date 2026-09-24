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
                    Some(RunCommand::Deploy) => "Reports deployed".into(),
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
        if let Some(request) = self
            .manager
            .request
            .as_ref()
            .filter(|request| request.command == RunCommand::Deploy)
        {
            return self.render_deployment_activity(request, cx);
        }
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
        let elapsed_label = if self.manager.recovered_from_reports {
            "--:--".into()
        } else {
            format!("{:02}:{:02}", elapsed / 60_000, elapsed / 1000 % 60)
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
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .children(self.manager.request.as_ref().map(|request| {
                                    let label = if self.manager.recovered_from_reports {
                                        "Saved comparison results".into()
                                    } else {
                                        format!(
                                            "{}: {}",
                                            if self.running() {
                                                "Running"
                                            } else {
                                                "Last run"
                                            },
                                            request.command.action_label()
                                        )
                                    };
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
                                        .w_full()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(phase.to_string()),
                                ),
                        )
                        .child(
                            div()
                                .id("run-elapsed")
                                .role(Role::Status)
                                .aria_label(elapsed_label.clone())
                                .test_support()
                                .flex_shrink_0()
                                .text_sm()
                                .font_family(cx.theme().mono_font_family.clone())
                                .child(elapsed_label),
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
        if let Some(error) = &self.manager.persistence_error {
            content = content.child(
                div()
                    .id("run-persistence-error")
                    .role(Role::Status)
                    .aria_label(error.clone())
                    .test_support()
                    .px_3()
                    .py_2()
                    .bg(cx.theme().warning.opacity(0.08))
                    .text_color(cx.theme().warning)
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
                let label = self
                    .suites
                    .iter()
                    .find(|entry| entry.output_id == suite.id)
                    .map(|entry| entry.label())
                    .unwrap_or(&suite.name);
                content =
                    content.child(self.suite_header(&suite.id, label, counts, state, expanded, cx));
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
                    suite.label(),
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

    fn render_deployment_activity(
        &self,
        request: &RunRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let progress = &self.manager.progress;
        let (heading, color, icon) = match &self.manager.outcome {
            None => ("Deploying reports", cx.theme().primary, None),
            Some(RunOutcome::Completed) => (
                "Deployment complete",
                cx.theme().success,
                Some(gpui_kit::assets::IconName::CircleCheck),
            ),
            Some(RunOutcome::Failed(_)) => (
                "Deployment failed",
                cx.theme().danger,
                Some(gpui_kit::assets::IconName::CircleX),
            ),
            Some(RunOutcome::Cancelled) => (
                "Deployment stopped",
                cx.theme().warning,
                Some(gpui_kit::assets::IconName::Square),
            ),
        };
        let phase = if progress.phase.is_empty() {
            "Preparing selected reports"
        } else {
            &progress.phase
        };
        let elapsed = if self.manager.outcome.is_none() {
            self.started
                .map(|started| started.elapsed().as_millis() as u64)
                .unwrap_or(progress.elapsed_ms)
        } else {
            progress.elapsed_ms
        };
        let error = progress.error.as_deref().or(match &self.manager.outcome {
            Some(RunOutcome::Failed(error)) => Some(error.as_str()),
            _ => None,
        });
        let recent_logs = deployment_log_tail(&self.manager.logs);
        let log_count = recent_logs.len();
        div()
            .id("activity-scroll")
            .size_full()
            .overflow_y_scrollbar()
            .child(
                div()
                    .id("deployment-progress")
                    .role(Role::Group)
                    .aria_label("Report deployment")
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .w_full()
                    .min_w_0()
                    .bg(color.opacity(0.08))
                    .border_b_1()
                    .border_color(color.opacity(0.35))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .size_8()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(match icon {
                                        Some(icon) => Icon::new(icon)
                                            .size_6()
                                            .text_color(color)
                                            .into_any_element(),
                                        None => div()
                                            .id("deployment-spinner")
                                            .test_support()
                                            .child(
                                                spinner::Spinner::new()
                                                    .with_size(px(24.))
                                                    .color(color),
                                            )
                                            .into_any_element(),
                                    }),
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
                                            .id("run-outcome-heading")
                                            .role(Role::Status)
                                            .aria_label(heading)
                                            .test_support()
                                            .text_base()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(color)
                                            .child(heading),
                                    )
                                    .child(
                                        div()
                                            .id("live-phase")
                                            .role(Role::Status)
                                            .aria_label(phase.to_string())
                                            .test_support()
                                            .text_sm()
                                            .truncate()
                                            .child(phase.to_string()),
                                    ),
                            )
                            .child(
                                div()
                                    .id("deployment-elapsed")
                                    .test_support()
                                    .flex_shrink_0()
                                    .text_sm()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .child(format!(
                                        "{:02}:{:02}",
                                        elapsed / 60_000,
                                        elapsed / 1000 % 60
                                    )),
                            ),
                    )
                    .when_some(request.options.deployment.as_ref(), |body, job| {
                        let destination = format!(
                            "{} | {} | {} | {} {}",
                            job.target.name,
                            job.target.provider.label(),
                            if job.target.environment.is_empty() {
                                "Production"
                            } else {
                                &job.target.environment
                            },
                            job.suite_ids.len(),
                            if job.suite_ids.len() == 1 {
                                "suite"
                            } else {
                                "suites"
                            },
                        );
                        body.child(
                            div()
                                .id("deployment-destination")
                                .role(Role::Status)
                                .aria_label(destination.clone())
                                .test_support()
                                .text_sm()
                                .truncate()
                                .child(destination),
                        )
                    }),
            )
            .when_some(error, |body, error| {
                body.child(
                    div()
                        .id("deployment-error")
                        .role(Role::Status)
                        .aria_label(error.to_string())
                        .test_support()
                        .px_4()
                        .py_3()
                        .text_color(cx.theme().danger)
                        .child(error.to_string()),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .p_4()
                    .child(
                        Button::new("activity-view-log")
                            .small()
                            .icon(gpui_kit::assets::IconName::Terminal)
                            .label("View Log")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_all_logs(window, cx)),
                            ),
                    )
                    .when_some(
                        progress.deployment.as_ref().filter(|_| {
                            matches!(self.manager.outcome, Some(RunOutcome::Completed))
                        }),
                        |row, deployment| {
                            let url = deployment.url.clone();
                            row.child(
                                Button::new("deployment-open-site")
                                    .small()
                                    .primary()
                                    .icon(gpui_kit::assets::IconName::ExternalLink)
                                    .label("Open Site")
                                    .tooltip(format!("{}: {}", deployment.target, deployment.url))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Err(error) = open::that_detached(&url) {
                                            this.status =
                                                format!("Cannot open deployed site: {error}");
                                            cx.notify();
                                        }
                                    })),
                            )
                        },
                    ),
            )
            .when(!recent_logs.is_empty(), |body| {
                body.child(
                    div()
                        .id("deployment-live-log")
                        .role(Role::Group)
                        .aria_label("Deployment log")
                        .test_support()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .px_4()
                        .py_3()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child("Deployment log"),
                        )
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .text_xs()
                                .font_family(cx.theme().mono_font_family.clone())
                                .children(recent_logs.into_iter().enumerate().map(
                                    |(index, line)| {
                                        div()
                                            .id(SharedString::from(if index + 1 == log_count {
                                                "deployment-log-latest".to_string()
                                            } else {
                                                format!("deployment-log-line-{index}")
                                            }))
                                            .role(Role::ListItem)
                                            .aria_label(line.clone())
                                            .test_support()
                                            .w_full()
                                            .min_w_0()
                                            .truncate()
                                            .child(line)
                                    },
                                )),
                        ),
                )
            })
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

fn deployment_log_tail(
    logs: &std::collections::VecDeque<crate::desktop::runner::LogLine>,
) -> Vec<String> {
    let mut lines = logs
        .iter()
        .rev()
        .flat_map(|entry| entry.text.lines().rev())
        .filter(|line| !line.trim().is_empty())
        .take(12)
        .map(|line| {
            let mut text = line.chars().take(512).collect::<String>();
            if line.chars().nth(512).is_some() {
                text.push_str("...");
            }
            text
        })
        .collect::<Vec<_>>();
    lines.reverse();
    lines
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

#[cfg(all(test, feature = "gui-tests"))]
mod tests {
    use crate::desktop::deployment::{DeployTarget, DeploymentJob, DeploymentProvider};
    use crate::desktop::runner::{LogLine, RunCommand, RunOptions, RunOutcome, RunRequest};
    use crate::progress::{Event, Record};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, px, size};

    fn deployment_request(root: &std::path::Path) -> RunRequest {
        RunRequest {
            root: root.to_path_buf(),
            command: RunCommand::Deploy,
            options: RunOptions {
                deployment: Some(DeploymentJob {
                    target: DeployTarget {
                        id: "qa".into(),
                        name: "QA reports".into(),
                        provider: DeploymentProvider::AzureSwa,
                        credential_id: None,
                        environment: "preview".into(),
                        site_id: String::new(),
                        cloudflare: None,
                    },
                    suite_ids: vec!["ui-test".into()],
                    catalog_ids: vec!["ui-test".into()],
                    partial_confirmed: false,
                }),
                ..Default::default()
            },
        }
    }

    #[test]
    fn deployment_log_tail_keeps_recent_messages_in_order_and_bounds_rendering() {
        let logs = (0..30)
            .map(|index| LogLine {
                stderr: true,
                text: format!("\nStep {index}\n"),
            })
            .chain([LogLine {
                stderr: true,
                text: "x".repeat(10_000),
            }])
            .collect();
        let lines = super::deployment_log_tail(&logs);
        assert_eq!(lines.len(), 12);
        assert_eq!(lines[0], "Step 19");
        assert_eq!(lines[10], "Step 29");
        assert_eq!(lines[11].len(), 515);
        assert!(lines[11].ends_with("..."));
    }

    #[gpui_kit::test]
    fn chromium_install_phases_fit_and_logs_remain_accessible(cx: &mut TestAppContext) {
        let phases = [
            "Waiting for Chromium cache lock (120s elapsed; another Regressify process is preparing the browser)",
            "Connecting to Chromium download server (25s elapsed; waiting for response)",
            "Downloading Chromium: 128.0/300.0 MiB (43%), 1.25 MiB/s, 102s elapsed; no data for 55s",
            "Downloading Chromium: 128.0 MiB (total size unknown), 1.25 MiB/s, 102s elapsed; no data for 55s",
            "Extracting Chromium (120s elapsed; unpacking files)",
            "Verifying and saving Chromium installation",
            "Chromium ready",
            "Capturing scenarios",
        ];
        for (width, height) in [(1080., 720.), (1600., 1000.)] {
            let directory = tempfile::tempdir().unwrap();
            let (handle, view) = super::super::super::tests::editor_at_size(
                cx,
                directory.path(),
                size(px(width), px(height)),
            );
            cx.update_window(handle.into(), |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.main_tab = 1;
                    this.runs.update(cx, |panel, cx| {
                        panel.manager.request = Some(RunRequest {
                            root: directory.path().to_path_buf(),
                            command: RunCommand::Reference,
                            options: RunOptions::default(),
                        });
                        cx.notify();
                    });
                    cx.notify();
                });
                for dark in [false, true] {
                    super::super::super::design::apply_theme(dark, Some(window), cx);
                    for phase in phases {
                        view.update(cx, |this, cx| {
                            this.runs.update(cx, |panel, cx| {
                                panel.manager.progress.apply(Record {
                                    version: 1,
                                    elapsed_ms: 102_000,
                                    event: Event::Phase { name: phase.into() },
                                });
                                panel.manager.logs.push_back(LogLine {
                                    stderr: true,
                                    text: format!("{phase}\n"),
                                });
                                cx.notify();
                            });
                        });
                        window.render_frame(cx);
                        assert_eq!(window.find("live-phase").label(), Some(phase));
                        let bounds = window.find("live-phase").bounds();
                        let elapsed = window.find("run-elapsed").bounds();
                        let log = window.find("activity-view-log").bounds();
                        assert!(bounds.size.width > px(0.));
                        assert!(bounds.size.height > px(0.));
                        assert!(bounds.origin.x + bounds.size.width <= elapsed.origin.x);
                        assert!(elapsed.origin.x + elapsed.size.width <= px(width));
                        assert!(bounds.origin.y + bounds.size.height <= log.origin.y);
                        assert!(log.origin.y + log.size.height <= px(height));
                    }
                }
                window.click("activity-view-log", cx);
                window.render_frame(cx);
                assert_eq!(window.find("run-log-heading").label(), Some("All run logs"));
            })
            .unwrap();
        }
    }

    #[gpui_kit::test]
    fn deployment_activity_shows_phases_and_outcomes(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::super::tests::editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.main_tab = 1;
                this.runs.update(cx, |panel, cx| {
                    panel.manager.request = Some(deployment_request(directory.path()));
                    panel.manager.logs.push_back(LogLine {
                        stderr: true,
                        text: "Azure tier: Free; storage budget 250 MiB\n".into(),
                    });
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 1200,
                        event: Event::Phase {
                            name: "Uploading selected reports".into(),
                        },
                    });
                    cx.notify();
                });
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("deployment-progress").is_some());
            assert!(window.find("deployment-live-log").visible());
            assert_eq!(
                window.find("deployment-log-latest").label(),
                Some("Azure tier: Free; storage budget 250 MiB")
            );
            assert_eq!(
                window.find("run-outcome-heading").label(),
                Some("Deploying reports")
            );
            assert_eq!(
                window.find("live-phase").label(),
                Some("Uploading selected reports")
            );
            assert!(window.try_find("metric-total").is_none());
            assert!(window.try_find("deployment-open-site").is_none());
            assert!(window.find("deployment-spinner").visible());
            assert!(
                window
                    .find("deployment-destination")
                    .label()
                    .unwrap()
                    .contains("QA reports")
            );

            for phase in [
                "Validating Azure deployment target",
                "Packaging selected reports",
                "Uploading selected reports",
                "Waiting for Azure deployment",
                "Validating Netlify deployment target",
                "Packaging selected reports for Netlify",
                "Waiting for Netlify deployment",
            ] {
                view.update(cx, |this, cx| {
                    this.runs.update(cx, |panel, cx| {
                        panel.manager.progress.apply(Record {
                            version: 1,
                            elapsed_ms: 1800,
                            event: Event::Phase { name: phase.into() },
                        });
                        panel.manager.logs.push_back(LogLine {
                            stderr: true,
                            text: format!("{phase}\n"),
                        });
                        cx.notify();
                    });
                });
                window.render_frame(cx);
                assert_eq!(window.find("live-phase").label(), Some(phase));
                assert_eq!(window.find("deployment-log-latest").label(), Some(phase));
                assert!(window.find("deployment-spinner").visible());
            }
            window.click("activity-view-log", cx);
            window.render_frame(cx);
            assert_eq!(window.find("run-log-heading").label(), Some("All run logs"));
            window.click("log-activity", cx);
            window.render_frame(cx);
            assert!(window.find("deployment-progress").visible());

            for (outcome, heading) in [
                (RunOutcome::Completed, "Deployment complete"),
                (
                    RunOutcome::Failed("Upload unavailable".into()),
                    "Deployment failed",
                ),
                (RunOutcome::Cancelled, "Deployment stopped"),
            ] {
                view.update(cx, |this, cx| {
                    this.runs.update(cx, |panel, cx| {
                        panel.manager.progress.apply(Record {
                            version: 1,
                            elapsed_ms: 2400,
                            event: Event::Deployed {
                                target: "QA reports".into(),
                                url: "https://reports.example.com".into(),
                            },
                        });
                        panel.manager.outcome = Some(outcome.clone());
                        cx.notify();
                    });
                });
                window.render_frame(cx);
                assert_eq!(window.find("run-outcome-heading").label(), Some(heading));
                assert!(window.try_find("deployment-spinner").is_none());
                if let RunOutcome::Failed(error) = &outcome {
                    assert_eq!(
                        window.find("deployment-error").label(),
                        Some(error.as_str())
                    );
                }
                assert_eq!(
                    window.try_find("deployment-open-site").is_some(),
                    matches!(outcome, RunOutcome::Completed)
                );
            }
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn deployment_activity_fits_compact_and_wide_windows(cx: &mut TestAppContext) {
        for (width, height) in [(1080., 720.), (1600., 1000.)] {
            let directory = tempfile::tempdir().unwrap();
            let (handle, view) = super::super::super::tests::editor_at_size(
                cx,
                directory.path(),
                size(px(width), px(height)),
            );
            cx.update_window(handle.into(), |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.main_tab = 1;
                    this.runs.update(cx, |panel, cx| {
                        let mut request = deployment_request(directory.path());
                        let job = request.options.deployment.as_mut().unwrap();
                        job.target.name = "Long deployment destination ".repeat(4);
                        job.target.provider = DeploymentProvider::Netlify;
                        job.target.environment.clear();
                        panel.manager.request = Some(request);
                        panel.manager.progress.phase =
                            format!("Uploading selected reports {}", "details".repeat(40));
                        panel.manager.logs.extend((0..30).map(|index| LogLine {
                            stderr: true,
                            text: format!("Upload step {index}: {}\n", "details ".repeat(100)),
                        }));
                        cx.notify();
                    });
                    cx.notify();
                });
                for dark in [false, true] {
                    super::super::super::design::apply_theme(dark, Some(window), cx);
                    window.render_frame(cx);
                    let band = window.find("deployment-progress").bounds();
                    assert!(band.size.width > px(400.));
                    assert!(band.origin.x + band.size.width <= px(width));
                    assert!(band.origin.y + band.size.height <= px(height));
                    for id in [
                        "deployment-spinner",
                        "run-outcome-heading",
                        "live-phase",
                        "deployment-destination",
                        "deployment-elapsed",
                    ] {
                        let bounds = window.find(id).bounds();
                        assert!(bounds.size.width > px(0.), "{id}");
                        assert!(bounds.size.height > px(0.), "{id}");
                        assert!(bounds.origin.x >= band.origin.x, "{id}");
                        assert!(
                            bounds.origin.x + bounds.size.width <= band.origin.x + band.size.width,
                            "{id}"
                        );
                        assert!(
                            bounds.origin.y + bounds.size.height
                                <= band.origin.y + band.size.height,
                            "{id}"
                        );
                    }
                    let phase = window.find("live-phase").bounds();
                    let elapsed = window.find("deployment-elapsed").bounds();
                    assert!(phase.origin.x + phase.size.width <= elapsed.origin.x);
                    let log = window.find("activity-view-log").bounds();
                    assert!(log.origin.y >= band.origin.y + band.size.height);
                    assert!(log.origin.y + log.size.height <= px(height));
                    let live_log = window.find("deployment-live-log").bounds();
                    assert!(live_log.origin.y >= log.origin.y + log.size.height);
                    assert!(live_log.origin.y + live_log.size.height <= px(height));
                    assert!(live_log.origin.x >= band.origin.x);
                    assert!(
                        live_log.origin.x + live_log.size.width <= band.origin.x + band.size.width
                    );
                    let latest = window.find("deployment-log-latest").bounds();
                    assert!(latest.origin.x >= live_log.origin.x);
                    assert!(
                        latest.origin.x + latest.size.width
                            <= live_log.origin.x + live_log.size.width
                    );
                    assert!(
                        latest.origin.y + latest.size.height
                            <= live_log.origin.y + live_log.size.height
                    );
                }
            })
            .unwrap();
        }
    }
}
