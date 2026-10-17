use super::*;
use crate::run_history::cleanup::{self as history, CleanupPlan};

pub(super) struct CleanupPanel {
    output: PathBuf,
    suites: Vec<(String, String)>,
    selected: BTreeSet<String>,
    keep: Entity<InputState>,
    plan: Option<CleanupPlan>,
    status: String,
    error: bool,
    _subscriptions: Vec<Subscription>,
}

impl CleanupPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let keep = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .mask_pattern(gpui_kit::component::input::MaskPattern::None)
                .default_value("1");
            input.set_step(None, window, cx);
            input
        });
        let changed = cx.subscribe(&keep, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.plan = None;
                this.status.clear();
                this.error = false;
                cx.notify();
            }
        });
        let stepped = cx.subscribe_in(
            &keep,
            window,
            |this, input, event: &NumberInputEvent, window, cx| {
                let NumberInputEvent::Step(action) = event;
                let current = input.read(cx).value().parse::<usize>().unwrap_or(1);
                let next = if *action == StepAction::Increment {
                    current.saturating_add(1)
                } else {
                    current.saturating_sub(1).max(1)
                };
                input.update(cx, |input, cx| {
                    input.set_value(next.to_string(), window, cx)
                });
                this.plan = None;
                cx.notify();
            },
        );
        Self {
            output: PathBuf::new(),
            suites: Vec::new(),
            selected: BTreeSet::new(),
            keep,
            plan: None,
            status: String::new(),
            error: false,
            _subscriptions: vec![changed, stepped],
        }
    }
}

#[cfg(all(test, feature = "gui-tests"))]
mod tests {
    use super::super::tests::{editor, editor_at_size, settle_dialog};
    use gpui_kit::test::{TestAppContextExt, TestWindowExt};
    use gpui_kit::{AppContext, Focusable, TestAppContext, px, size};
    use std::{fs, path::Path, time::Duration};

    fn capture_runs(output: &Path, suite: &str) {
        use crate::run_history::{RunLease, RunState};
        for id in ["old", "latest"] {
            let mut run = RunLease::begin(&output.join(suite), id, true).unwrap();
            let png = lodepng::encode32(&[40_u8, 80, 120, 255], 1, 1).unwrap();
            fs::write(
                output
                    .join(suite)
                    .join("bitmaps_test")
                    .join(id)
                    .join("home.png"),
                png,
            )
            .unwrap();
            run.finish(RunState::Completed, false).unwrap();
        }
    }

    #[gpui_kit::test]
    async fn cleanup_dialog_previews_selects_invalidates_and_deletes_only_confirmed_suites(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join(".regressify");
        capture_runs(&output, "ui-test");
        capture_runs(&output, "other");
        let (handle, view) = editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.overview = true;
                cx.notify();
            });
            window.render_frame(cx);
            window.click("overview-cleanup", cx);
        })
        .unwrap();
        settle_dialog(cx, handle).await;
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            !view.read(cx).busy
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(
                view.read(cx)
                    .cleanup
                    .read(cx)
                    .keep
                    .read(cx)
                    .value()
                    .as_ref(),
                "1"
            );
            if view.read(cx).cleanup.read(cx).selected.len() != 2 {
                window.click("cleanup-select-all", cx);
                window.render_frame(cx);
            }
            window.click("cleanup-select-all", cx);
            window.render_frame(cx);
            window.click("cleanup-suite-ui-test", cx);
            window.render_frame(cx);
            window.click("cleanup-confirm", cx);
            assert!(output.join("ui-test/bitmaps_test/old").is_dir());
            window.click("cleanup-review", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            !view.read(cx).busy
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let state = view.read(cx).cleanup.read(cx);
            assert_eq!(state.plan.as_ref().expect(&state.status).removed_runs(), 1);
            assert_eq!(
                window.find("cleanup-preview-ui-test").label(),
                Some("ui-test: keep latest; delete 1 run(s)")
            );
            assert!(output.join("ui-test/bitmaps_test/old").is_dir());
            window.within("cleanup-keep").click("increment", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(view.read(cx).cleanup.read(cx).plan.is_none());
            let input = view.read(cx).cleanup.read(cx).keep.clone();
            window.focus(&input.read(cx).focus_handle(cx), cx);
            window.press("ctrl-a", cx);
            window.input("0", cx);
            window.render_frame(cx);
            window.click("cleanup-review", cx);
            assert!(view.read(cx).cleanup.read(cx).error);
            window.focus(&input.read(cx).focus_handle(cx), cx);
            window.press("ctrl-a", cx);
            window.input("1", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("cleanup-review", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            !view.read(cx).busy
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("cleanup-confirm", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
            !view.read(cx).busy
        })
        .await;
        cx.update(|cx| {
            let state = view.read(cx).cleanup.read(cx);
            assert!(!state.error, "{}", state.status);
            assert!(state.status.contains("Removed 1"));
        });
        assert!(!output.join("ui-test/bitmaps_test/old").exists());
        assert!(output.join("ui-test/bitmaps_test/latest").is_dir());
        assert!(output.join("other/bitmaps_test/old").is_dir());
    }

    #[gpui_kit::test]
    async fn cleanup_dialog_cancel_busy_and_custom_output_preserve_data(cx: &mut TestAppContext) {
        for size in [size(px(1000.), px(640.)), size(px(1600.), px(1000.))] {
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join("custom-output");
            capture_runs(&output, "ui-test");
            let (handle, view) = editor_at_size(cx, directory.path(), size);
            cx.update_window(handle.into(), |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.open_cleanup(Some(output.clone()), Some("ui-test".into()), window, cx)
                });
            })
            .unwrap();
            settle_dialog(cx, handle).await;
            cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
                !view.read(cx).busy
            })
            .await;
            cx.update_window(handle.into(), |_, window, cx| {
                assert_eq!(view.read(cx).cleanup.read(cx).output, output);
                assert!(window.find("cleanup-review").visible());
                assert!(window.find("cleanup-confirm").visible());
                view.update(cx, |this, cx| {
                    this.busy = true;
                    cx.notify();
                });
                window.render_frame(cx);
                window.press("escape", cx);
                window.render_frame(cx);
                assert!(window.try_find("cleanup-review").is_some());
                view.update(cx, |this, cx| {
                    this.busy = false;
                    cx.notify();
                });
                window.render_frame(cx);
                window.click("cleanup-cancel", cx);
            })
            .unwrap();
            assert!(output.join("ui-test/bitmaps_test/old").is_dir());
            assert!(output.join("ui-test/bitmaps_test/latest").is_dir());
        }
    }
}

impl Workbench {
    pub(super) fn cleanup_button(&self, id: &'static str, cx: &mut Context<Self>) -> Button {
        Button::new(id)
            .ghost()
            .icon(gpui_kit::assets::IconName::Trash)
            .tooltip("Clean up old test runs")
            .accessibility_label("Clean up old test runs")
            .disabled(self.busy || self.runs.read(cx).running() || self.session.workspace.is_none())
            .on_click(cx.listener(|this, _, window, cx| this.open_cleanup(None, None, window, cx)))
    }

    pub(super) fn open_cleanup(
        &mut self,
        output: Option<PathBuf>,
        suite: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.runs.read(cx).running() || self.session.workspace.is_none() {
            return;
        }
        let output = match output
            .map(Ok)
            .unwrap_or_else(|| self.runs.read(cx).output_root())
        {
            Ok(output) => output,
            Err(error) => {
                self.status = format!("{error:#}");
                cx.notify();
                return;
            }
        };
        let selected_names = self.runs.read(cx).selected_names();
        let initial: BTreeSet<_> =
            suite
                .map(|suite| BTreeSet::from([suite]))
                .unwrap_or_else(|| {
                    self.session
                        .suites
                        .iter()
                        .filter(|suite| {
                            suite
                                .entry
                                .suite_name()
                                .is_some_and(|name| selected_names.contains(&name))
                        })
                        .map(|suite| suite.output_id.clone())
                        .collect()
                });
        let labels: BTreeMap<_, _> = self
            .session
            .suites
            .iter()
            .map(|suite| (suite.output_id.clone(), suite.label().to_owned()))
            .collect();
        self.cleanup.update(cx, |panel, cx| {
            panel.output = output.clone();
            panel.suites.clear();
            panel.selected.clear();
            panel.plan = None;
            panel.status = "Loading test runs".into();
            panel.error = false;
            panel
                .keep
                .update(cx, |input, cx| input.set_value("1", window, cx));
            cx.notify();
        });
        let view = cx.entity();
        let panel = self.cleanup.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = panel.read(cx);
            let busy = view.read(cx).busy;
            let mut body = div()
                .id("cleanup-dialog")
                .test_support()
                .flex()
                .flex_col()
                .gap_3()
                .min_w_0()
                .child(div().text_sm().child("Output folder"))
                .child(
                    div()
                        .id("cleanup-output")
                        .test_support()
                        .text_sm()
                        .child(display_path(&state.output)),
                )
                .child(
                    Form::new().child(
                        FormField::new().label("Completed runs to keep").child(
                            div()
                                .id("cleanup-keep")
                                .test_support()
                                .w(px(140.))
                                .child(NumberInput::new(&state.keep).disabled(busy)),
                        ),
                    ),
                )
                .child(
                    Checkbox::new("cleanup-select-all")
                        .label("All test suites")
                        .checked(
                            !state.suites.is_empty() && state.selected.len() == state.suites.len(),
                        )
                        .disabled(busy)
                        .on_change(window.listener_for(&view, |this, checked, _, cx| {
                            if this.busy {
                                return;
                            }
                            this.cleanup.update(cx, |panel, cx| {
                                panel.selected = if *checked {
                                    panel.suites.iter().map(|(id, _)| id.clone()).collect()
                                } else {
                                    BTreeSet::new()
                                };
                                panel.plan = None;
                                panel.status.clear();
                                cx.notify();
                            });
                        })),
                )
                .child(
                    div()
                        .id("cleanup-suites")
                        .max_h(px(200.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(state.suites.iter().map(|(id, label)| {
                            let suite_id = id.clone();
                            div().min_w_0().child(
                                Checkbox::new(SharedString::from(format!("cleanup-suite-{id}")))
                                    .label(label.clone())
                                    .checked(state.selected.contains(id))
                                    .disabled(busy)
                                    .on_change(window.listener_for(
                                        &view,
                                        move |this, checked, _, cx| {
                                            if this.busy {
                                                return;
                                            }
                                            this.cleanup.update(cx, |panel, cx| {
                                                if *checked {
                                                    panel.selected.insert(suite_id.clone());
                                                } else {
                                                    panel.selected.remove(&suite_id);
                                                }
                                                panel.plan = None;
                                                panel.status.clear();
                                                cx.notify();
                                            });
                                        },
                                    )),
                            )
                        })),
                );
            if let Some(plan) = &state.plan {
                body = body.child(
                    div()
                        .id("cleanup-preview")
                        .test_support()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(format!(
                            "{} old run(s), {} estimated space",
                            plan.removed_runs(),
                            crate::helpers::format_size(plan.bytes)
                        ))
                        .children(plan.suites.iter().map(|suite| {
                            let summary = format!(
                                "{}: keep {}; delete {} run(s)",
                                suite.id,
                                if suite.kept_runs.is_empty() {
                                    "none".into()
                                } else {
                                    suite.kept_runs.join(", ")
                                },
                                suite.removed_runs.len(),
                            );
                            div()
                                .id(SharedString::from(format!("cleanup-preview-{}", suite.id)))
                                .role(Role::Status)
                                .aria_label(summary.clone())
                                .test_support()
                                .text_sm()
                                .child(summary)
                        }))
                        .children(plan.warnings.iter().map(|warning| {
                            div()
                                .text_sm()
                                .text_color(cx.theme().warning)
                                .child(warning.clone())
                        }))
                        .child(
                            div()
                                .text_sm()
                                .child("Reference baselines and browser state will be preserved."),
                        ),
                );
            }
            if !state.status.is_empty() {
                body = body.child(
                    div()
                        .id("cleanup-status")
                        .role(if state.error {
                            Role::Alert
                        } else {
                            Role::Status
                        })
                        .aria_label(state.status.clone())
                        .test_support()
                        .text_sm()
                        .text_color(if state.error {
                            cx.theme().danger
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(state.status.clone()),
                );
            }
            dialog
                .title("Clean up test runs")
                .w(px(560.))
                .close_button(!busy)
                .keyboard(!busy)
                .overlay_closable(!busy)
                .on_ok(|_, _, _| false)
                .child(body.max_h(px(420.)).overflow_y_scrollbar())
                .footer(
                    gpui_kit::component::dialog::DialogFooter::new()
                        .child(
                            Button::new("cleanup-cancel")
                                .label("Close")
                                .disabled(busy)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("cleanup-review")
                                .label("Preview")
                                .disabled(busy || state.selected.is_empty())
                                .on_click(window.listener_for(&view, |this, _, window, cx| {
                                    this.preview_cleanup(window, cx)
                                })),
                        )
                        .child(
                            Button::new("cleanup-confirm")
                                .danger()
                                .icon(gpui_kit::assets::IconName::Trash)
                                .label("Delete old runs")
                                .disabled(busy || state.plan.is_none())
                                .on_click(window.listener_for(&view, |this, _, window, cx| {
                                    this.confirm_cleanup(window, cx)
                                })),
                        ),
                )
        });
        self.task(
            "Loading cleanup suites",
            move || Ok(history::output_suites(&output)),
            move |this, result, _, cx| {
                this.cleanup.update(cx, |panel, cx| {
                    match result {
                        Ok(suites) => {
                            panel.suites = suites
                                .into_iter()
                                .map(|id| {
                                    let label = labels
                                        .get(&id)
                                        .filter(|label| *label != &id)
                                        .map(|label| format!("{label} ({id})"))
                                        .unwrap_or_else(|| id.clone());
                                    (id, label)
                                })
                                .collect();
                            panel.selected = panel
                                .suites
                                .iter()
                                .filter(|(id, _)| initial.is_empty() || initial.contains(id))
                                .map(|(id, _)| id.clone())
                                .collect();
                            panel.status = if panel.suites.is_empty() {
                                "No test runs in this output folder".into()
                            } else {
                                String::new()
                            };
                        }
                        Err(error) => {
                            panel.status = format!("{error:#}");
                            panel.error = true;
                        }
                    }
                    cx.notify();
                });
            },
            window,
            cx,
        );
    }

    fn preview_cleanup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        let state = self.cleanup.read(cx);
        let keep = state
            .keep
            .read(cx)
            .value()
            .parse::<usize>()
            .ok()
            .filter(|keep| *keep > 0);
        let Some(keep) = keep else {
            self.cleanup.update(cx, |panel, cx| {
                panel.plan = None;
                panel.error = true;
                panel.status = "Keep count must be a whole number of at least 1".into();
                cx.notify();
            });
            return;
        };
        let output = state.output.clone();
        let suites = state.selected.iter().cloned().collect::<Vec<_>>();
        self.task(
            "Previewing old test runs",
            move || Ok(history::preview(&output, &suites, keep)),
            |this, result, _, cx| {
                this.cleanup.update(cx, |panel, cx| {
                    match result {
                        Ok(plan) => {
                            panel.plan = Some(plan);
                            panel.status.clear();
                            panel.error = false;
                        }
                        Err(error) => {
                            panel.plan = None;
                            panel.status = format!("{error:#}");
                            panel.error = true;
                        }
                    }
                    cx.notify();
                })
            },
            window,
            cx,
        );
    }

    fn confirm_cleanup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        let Some(plan) = self.cleanup.read(cx).plan.clone() else {
            return;
        };
        let count = plan.removed_runs();
        let output = plan.output.clone();
        self.task(
            "Deleting old test runs",
            move || Ok(history::execute(&plan)),
            move |this, result, window, cx| {
                this.cleanup.update(cx, |panel, cx| {
                    panel.plan = None;
                    match result {
                        Ok(()) => {
                            panel.status = format!("Removed {count} old test run(s)");
                            panel.error = false;
                        }
                        Err(error) => {
                            panel.status = format!("{error:#}");
                            panel.error = true;
                        }
                    }
                    cx.notify();
                });
                this.runs
                    .update(cx, |panel, cx| panel.refresh_artifacts_if_idle(cx));
                this.results.update(cx, |panel, cx| {
                    panel.show_run(output, String::new(), window, cx)
                });
            },
            window,
            cx,
        );
    }
}
