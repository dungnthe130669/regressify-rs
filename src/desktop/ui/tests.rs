use std::{path::Path, time::Duration};

use gpui_kit::component::Root;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, Entity, Focusable, TestAppContext, WindowHandle, px, size};

use super::Workbench;
use crate::desktop::session::Selection;

#[gpui_kit::test]
fn structural_actions_do_not_discard_an_invalid_field_draft(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let field = this
                .bindings
                .iter()
                .find(|binding| binding.field.key == "delay")
                .unwrap()
                .field
                .clone();
            this.change_field(&field, "not a number", window, cx);
        });
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        assert!(view.read(cx).field_errors.contains_key("delay"));
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
    })
    .unwrap();
}

#[gpui_kit::test]
async fn save_reads_pending_native_field_text_before_deferred_notifications(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("field-url", cx);
        window.press("ctrl-a", cx);
        window.input("https://example.com/pending-edit", cx);
        view.update(cx, |this, cx| {
            this.save_document(&super::SaveDocument, window, cx)
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Saved"))
    })
    .await;
    let source =
        std::fs::read_to_string(directory.path().join("visual_tests/ui-test.tests.yaml")).unwrap();
    let value: serde_json::Value = serde_yaml::from_str(&source).unwrap();
    assert_eq!(
        value["scenarios"][0]["url"],
        "https://example.com/pending-edit"
    );
    cx.update(|cx| {
        assert!(
            view.read(cx)
                .source
                .read(cx)
                .value()
                .contains("/pending-edit")
        )
    });
}

#[gpui_kit::test]
async fn run_suite_refuses_a_file_shadowed_by_another_format(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let open = this
                .session
                .workspace
                .as_ref()
                .unwrap()
                .create_suite("ui-test.tests.json")
                .unwrap();
            this.session.install_document(open).unwrap();
            this.sync_editor(window, cx);
        });
        window.render_frame(cx);
        window.click("run-suite", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .and_then(|status| status.label().map(str::to_string))
            .is_some_and(|status| status.contains("not the selected"))
    })
    .await;
    cx.update(|cx| {
        assert_eq!(view.read(cx).main_tab, 0);
        assert!(!view.read(cx).runs.read(cx).running());
    });
}

#[gpui_kit::test]
async fn multiple_suites_can_be_checked_and_empty_selection_cannot_run(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.session
                .workspace
                .as_ref()
                .unwrap()
                .create_suite("second")
                .unwrap();
            this.session.refresh().unwrap();
            this.sync_suites(cx);
            this.main_tab = 1;
        });
        window.render_frame(cx);
        window.within("run-tabs").click(1_usize, cx);
        window.click("start-command", cx);
        assert!(!view.read(cx).runs.read(cx).running());
        window.click("suite-check-ui-test", cx);
        window.click("suite-check-second", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_count(), 2);
        assert_eq!(window.find("suite-check-ui-test").checked(), Some(true));
        window.click("select-all-suites", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_count(), 0);
        window.click("start-command", cx);
        assert!(!view.read(cx).runs.read(cx).running());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn switching_workspaces_discards_old_selection_and_closing_unloads_everything(
    cx: &mut TestAppContext,
) {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, first.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
            this.load_workspace(second.path().into(), window, cx);
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Workspace opened"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            view.read(cx).session.workspace.as_ref().unwrap().root,
            second.path().canonicalize().unwrap()
        );
        assert!(view.read(cx).session.open.is_none());
        assert!(view.read(cx).session.suites.is_empty());
        assert_eq!(view.read(cx).runs.read(cx).selected_count(), 0);
        assert_eq!(
            view.read(cx).runs.read(cx).output_root().unwrap(),
            second.path().canonicalize().unwrap().join(".regressify")
        );
        view.update(cx, |this, cx| this.close_workspace(window, cx));
        assert!(view.read(cx).session.workspace.is_none());
        assert!(view.read(cx).runs.read(cx).output_root().is_err());
        assert!(view.read(cx).source.read(cx).value().is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn live_dashboard_tracks_parallel_scenarios_and_filters_failures(cx: &mut TestAppContext) {
    use crate::progress::{Event, Job, Record};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| {
                for (id, suite) in [(1, "shop"), (2, "account")] {
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 0,
                        event: Event::Queued {
                            job: Job {
                                id,
                                suite_id: suite.into(),
                                suite_name: suite.into(),
                                scenario_id: format!("home-{id}"),
                                label: format!("Home {id}"),
                                viewport: "Desktop".into(),
                                captures: vec![format!("capture-{id}")],
                            },
                        },
                    });
                }
                panel.manager.progress.apply(Record {
                    version: 1,
                    elapsed_ms: 100,
                    event: Event::Started { id: 1 },
                });
                cx.notify();
            });
        });
        window.render_frame(cx);
        assert_eq!(
            window.find(("live-status", 1_usize)).label(),
            Some("Running")
        );
        assert_eq!(
            window.find(("live-status", 2_usize)).label(),
            Some("Queued")
        );
        assert_eq!(window.find("metric-active").label(), Some("Running: 1"));
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                for event in [
                    Event::Captured {
                        id: 1,
                        error: None,
                        infrastructure: false,
                    },
                    Event::Compared {
                        suite_id: "shop".into(),
                        capture: "capture-1".into(),
                        label: "Home 1".into(),
                        passed: true,
                        error: None,
                    },
                    Event::Captured {
                        id: 2,
                        error: Some("Missing #sign-in".into()),
                        infrastructure: false,
                    },
                ] {
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 1500,
                        event,
                    });
                }
                cx.notify();
            })
        });
        window.render_frame(cx);
        assert_eq!(
            window.find(("live-status", 1_usize)).label(),
            Some("Passed")
        );
        assert_eq!(
            window.find(("live-status", 2_usize)).label(),
            Some("Failed")
        );
        window.click("activity-failed-only", cx);
        assert!(window.try_find(("live-status", 1_usize)).is_none());
        assert!(window.find(("live-status", 2_usize)).visible());
    })
    .unwrap();
}

async fn settle_dialog(cx: &mut TestAppContext, handle: WindowHandle<Root>) {
    // GPUI's dialog animation uses wall time, not the test executor's clock.
    std::thread::sleep(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn editor(cx: &mut TestAppContext, root: &Path) -> (WindowHandle<Root>, Entity<Workbench>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| super::design::apply_theme(false, None, cx));
    cx.update(super::bind_keys);
    let mut created = None;
    let handle = cx.open_window(size(px(1280.), px(820.)), |window, cx| {
        let view = cx.new(|cx| Workbench::new(window, cx));
        view.update(cx, |this, cx| {
            this.session.connect(root).unwrap();
            this.runs.update(cx, |panel, cx| {
                panel.set_workspace(root.to_path_buf(), window, cx)
            });
            let open = this
                .session
                .workspace
                .as_ref()
                .unwrap()
                .create_suite("ui-test")
                .unwrap();
            this.session.install_document(open).unwrap();
            this.sync_suites(cx);
            this.select(Selection::Scenario(0), window, cx);
        });
        created = Some(view.clone());
        Root::new(view, window, cx)
    });
    (handle, created.unwrap())
}

#[gpui_kit::test]
async fn editing_real_suite_fields_preserves_plain_css_selectors(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let source = include_str!("../../../visual_tests/color-blender.tests.yaml");
    let original: serde_json::Value = serde_yaml::from_str(source).unwrap();
    let path = Path::new("visual_tests/color-blender.tests.yaml");
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let open = this
                .session
                .workspace
                .as_ref()
                .unwrap()
                .create_document(path, crate::desktop::document::DocumentKind::Suite, source)
                .unwrap();
            this.session.install_document(open).unwrap();
            this.sync_editor(window, cx);
        });
        window.render_frame(cx);
        let binding = view
            .read(cx)
            .bindings
            .iter()
            .find(|binding| binding.field.key == "viewportNames")
            .unwrap();
        let super::Control::Textarea(input) = &binding.control else {
            panic!("Expected viewport list input");
        };
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.render_frame(cx);
        window.input("name", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("hint-viewportNames")
            .is_some_and(|hint| hint.label() == Some("Override"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).field_errors.is_empty());
        assert!(view.read(cx).diagnostic.is_none());
        window.click("field-asyncCompareLimit", cx);
        window.input("3", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("hint-asyncCompareLimit")
            .is_some_and(|hint| hint.label() == Some("Override"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).field_errors.is_empty());
        assert!(view.read(cx).diagnostic.is_none());
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"],
            original["scenarios"]
        );
        window.click("save", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Saved"))
    })
    .await;
    let saved = std::fs::read_to_string(directory.path().join(path)).unwrap();
    let edited: serde_json::Value = serde_yaml::from_str(&saved).unwrap();
    assert_eq!(edited["viewportNames"], "name");
    assert_eq!(edited["asyncCompareLimit"], 3);
    assert_eq!(edited["scenarios"], original["scenarios"]);
    assert!(saved.contains(".t-o-color-blender__midpoint-list-item[aria-label='10 midpoints']"));
    assert_eq!(
        saved.matches("# Scenario:").count(),
        source.matches("# Scenario:").count()
    );
    cx.update(|cx| assert!(!view.read(cx).session.dirty()));
}

#[gpui_kit::test]
async fn typing_a_guided_field_and_clicking_save_updates_the_file(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("field-url").visible());
        window.click("field-url", cx);
        window.press("ctrl-a", cx);
        window.input("https://shop.example/checkout", cx);
        assert_eq!(
            window.find("field-url").value(),
            Some("https://shop.example/checkout")
        );
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Unsaved changes"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).session.dirty());
        window.click("save", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Saved"))
    })
    .await;
    let source =
        std::fs::read_to_string(directory.path().join("visual_tests/ui-test.tests.yaml")).unwrap();
    let value: serde_json::Value = serde_yaml::from_str(&source).unwrap();
    assert_eq!(
        value["scenarios"][0]["url"],
        "https://shop.example/checkout"
    );
    cx.update(|cx| assert!(!view.read(cx).session.dirty()));
}

#[gpui_kit::test]
fn duplicate_and_undo_buttons_update_the_actual_editor(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(window.find("field-url").bounds().size.width > px(100.));
        window.click("undo", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn invalid_source_exposes_a_diagnostic_and_save_does_not_write(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let path = directory.path().join("visual_tests/ui-test.tests.yaml");
    let original = std::fs::read_to_string(&path).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("editor-tabs").click(1_usize, cx);
        let focus = view.read(cx).source.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input("scenarios: not-a-list", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("diagnostic").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("diagnostic").visible());
        window.click("save", cx);
        assert!(view.read(cx).session.dirty());
        assert!(!view.read(cx).busy);
    })
    .unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

#[gpui_kit::test]
async fn unsaved_dialog_cancel_and_discard_preserve_their_contracts(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        view.update(cx, |this, cx| {
            this.guard(super::files::Pending::Reload, window, cx)
        });
        window.render_frame(cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("guard-cancel", cx);
        assert!(view.read(cx).session.dirty());
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    })
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.guard(super::files::Pending::Reload, window, cx)
        });
        window.render_frame(cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("guard-discard", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Document loaded"))
    })
    .await;
    cx.update(|cx| {
        assert!(!view.read(cx).session.dirty());
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    });
}

#[gpui_kit::test]
async fn new_suite_dialog_creates_a_real_file(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.guard(
                super::files::Pending::Name(super::files::Operation::New),
                window,
                cx,
            )
        });
        window.render_frame(cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("file-name", cx);
        window.press("ctrl-a", cx);
        window.input("checkout.tests.json", cx);
        assert_eq!(
            window.find("file-name").value(),
            Some("checkout.tests.json")
        );
        window.click("file-confirm", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("File saved"))
    })
    .await;
    let source =
        std::fs::read_to_string(directory.path().join("visual_tests/checkout.tests.json")).unwrap();
    let suite: crate::types::TestSuite = serde_json::from_str(&source).unwrap();
    assert_eq!(suite.scenarios.len(), 1);
    cx.update(|cx| assert_eq!(view.read(cx).session.entries.len(), 2));
}

#[gpui_kit::test]
async fn item_removal_requires_confirmation_and_is_undoable(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("remove-item", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("remove-confirm", cx);
        assert!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{}",
            view.read(cx).status
        );
        window.click("undo", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn baseline_commands_require_confirmation_before_execution(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
            this.runs.update(cx, |panel, cx| {
                panel.choose(crate::desktop::runner::RunCommand::Approve, window, cx)
            });
        });
        window.render_frame(cx);
        window.click("start-command", cx);
        assert!(window.find("run-confirm").visible());
        assert!(!view.read(cx).runs.read(cx).running());
        window.click("run-confirm-cancel", cx);
        assert!(window.try_find("run-confirm").is_none());
        assert!(!view.read(cx).runs.read(cx).running());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn run_options_change_with_command_and_suite_shortcut_keeps_editor(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("run-suite", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Run scope: ui-test"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(view.read(cx).main_tab, 1);
        assert!(window.find("start-command").visible());
        window.within("run-tabs").click(2_usize, cx);
        assert!(window.try_find("run-field-asyncCaptureLimit").is_some());
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.choose(crate::desktop::runner::RunCommand::Snapshot, window, cx)
            })
        });
        window.render_frame(cx);
        assert!(window.try_find("run-field-asyncCaptureLimit").is_none());
        assert!(window.try_find("run-field-heading").is_some());
        window.within("workbench-tabs").click(0_usize, cx);
        assert!(window.find("field-url").visible());
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn results_filter_errors_and_load_image_comparison_modes(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join(".regressify");
    let run = output.join("shop/bitmaps_test/run-1");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::create_dir_all(output.join("shop/html_report")).unwrap();
    std::fs::create_dir_all(output.join("shop/bitmaps_reference")).unwrap();
    for path in [
        run.join("home.png"),
        run.join("home_diff.png"),
        output.join("shop/bitmaps_reference/home.png"),
    ] {
        lodepng::encode32_file(path, &[0_u8, 128, 255, 255], 1, 1).unwrap();
    }
    let pair = serde_json::json!({"label": "Home", "reference": "../bitmaps_reference/home.png", "test": "../bitmaps_test/run-1/home.png", "diffImage": "../bitmaps_test/run-1/home_diff.png"});
    let report = serde_json::json!({"id": "shop", "tests": [
        {"status": "pass", "pair": pair},
        {"status": "fail", "pair": {"label": "Checkout", "engineErrorMsg": "Missing selector #pay"}}
    ]});
    std::fs::write(
        run.join("report.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    let (handle, _) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("workbench-tabs").click(2_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find(("image-details", 1_usize)).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.find(("image-details", 1_usize)).label(),
            Some("Current 1 x 1")
        );
        window.click("zoom-in", cx);
        assert_eq!(window.find("zoom-value").label(), Some("125%"));
        window.within("image-modes").click(3_usize, cx);
        assert!(window.find(("image-details", 0_usize)).visible());
        assert!(window.find(("image-details", 1_usize)).visible());
        window.within("result-filter").click(1_usize, cx);
        assert!(window.try_find(("capture", 0_usize)).is_none());
        window.click(("capture", 1_usize), cx);
        assert_eq!(
            window.find("capture-error").label(),
            Some("Missing selector #pay")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
async fn missing_output_has_a_recoverable_empty_results_view(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, _) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("workbench-tabs").click(2_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("results-status")
            .is_some_and(|status| status.label() == Some("0 reports, 0 warnings"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("refresh-results").visible());
        window.within("workbench-tabs").click(0_usize, cx);
        assert!(window.find("field-url").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn inherited_values_explicit_false_and_reset_have_distinct_ui_states(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("regressify.yaml"),
        "bypassCsp: true\n",
    )
    .unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("hint-bypassCsp").label(),
            Some("Workspace: true")
        );
        view.update(cx, |this, cx| {
            let field = this
                .bindings
                .iter()
                .find(|binding| binding.field.key == "bypassCsp")
                .unwrap()
                .field
                .clone();
            this.change_field(&field, "false", window, cx);
        });
        window.render_frame(cx);
        assert_eq!(window.find("hint-bypassCsp").label(), Some("Override"));
        assert_eq!(view.read(cx).effective["bypassCsp"], false);
        view.update(cx, |this, cx| {
            this.edit(|session| session.set_field("bypassCsp", None), window, cx)
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("hint-bypassCsp").label(),
            Some("Workspace: true")
        );
        assert_eq!(view.read(cx).effective["bypassCsp"], true);
        assert!(
            view.read(cx).session.value().unwrap()["scenarios"][0]
                .get("bypassCsp")
                .is_none()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
async fn new_suite_keyboard_shortcut_opens_the_file_dialog(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, _) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-n"
            } else {
                "ctrl-n"
            },
            cx,
        );
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("file-name").is_some()
    })
    .await;
    assert!(
        !directory
            .path()
            .join("visual_tests/new-suite.tests.yaml")
            .exists()
    );
}
