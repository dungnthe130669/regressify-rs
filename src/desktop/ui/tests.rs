use std::{path::Path, time::Duration};

use gpui_kit::component::Root;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, Entity, Focusable, TestAppContext, WindowHandle, px, size};

use super::Workbench;
use crate::desktop::session::Selection;

#[gpui_kit::test]
async fn update_dialog_preserves_the_draft_and_shows_build_status(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        window.click("software-updates", cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("update-status").visible());
        assert!(window.find("update-check").visible());
        assert!(view.read(cx).unsaved());
        assert!(!view.read(cx).updates.read(cx).install_on_close);
        window.click("update-close", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
async fn update_install_reuses_the_unsaved_cancel_guard(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        view.update(cx, |this, cx| {
            this.guard(super::files::Pending::Update, window, cx)
        });
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("guard-save").visible());
        window.click("guard-cancel", cx);
        assert!(view.read(cx).unsaved());
        assert!(!view.read(cx).updates.read(cx).install_on_close);
    })
    .unwrap();
}

#[gpui_kit::test]
fn switching_editor_views_flushes_pending_text_and_keeps_invalid_source_editable(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("field-url", cx);
        window.press("ctrl-a", cx);
        window.input("https://example.com/latest-draft", cx);
        window.within("editor-tabs").click(1_usize, cx);
        assert!(
            view.read(cx)
                .source
                .read(cx)
                .value()
                .contains("https://example.com/latest-draft")
        );
        let source = view.read(cx).source.clone();
        window.focus(&source.read(cx).focus_handle(cx), cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input("scenarios:\n- url: https://example.com/from-source", cx);
        window.within("editor-tabs").click(0_usize, cx);
        view.update(cx, |this, cx| {
            this.select(Selection::Scenario(0), window, cx)
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("field-url").value(),
            Some("https://example.com/from-source")
        );
        window.within("editor-tabs").click(1_usize, cx);
        window.focus(&source.read(cx).focus_handle(cx), cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input("scenarios: [}", cx);
        let draft = view.read(cx).source.read(cx).value();
        window.within("editor-tabs").click(0_usize, cx);
        assert_eq!(view.read(cx).tab, 1);
        assert_eq!(view.read(cx).source.read(cx).value(), draft);
        assert!(view.read(cx).diagnostic.is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn file_action_availability_matches_document_and_workspace_state(cx: &mut TestAppContext) {
    use super::files::Operation;
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).file_operation_enabled(Operation::Delete, cx));
        view.update(cx, |this, cx| {
            this.edit(
                crate::desktop::session::Session::duplicate_selected,
                window,
                cx,
            )
        });
        assert!(!view.read(cx).file_operation_enabled(Operation::Delete, cx));
        assert!(view.read(cx).file_operation_enabled(Operation::SaveAs, cx));
        view.update(cx, |this, cx| {
            this.session.discard().unwrap();
            this.close_workspace(window, cx);
        });
        assert!(!view.read(cx).file_operation_enabled(Operation::New, cx));
        assert!(!view.read(cx).file_operation_enabled(Operation::Reload, cx));
        assert!(view.read(cx).file_operation_enabled(Operation::Theme, cx));
    })
    .unwrap();
}

#[gpui_kit::test]
async fn command_palette_preserves_the_unsaved_guard_when_opening_another_dialog(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        window.click("open-commands", cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| window.input("new suite", cx))
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("command:New suite").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
        .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("guard-cancel").visible());
        assert!(view.read(cx).session.dirty());
        window.click("guard-cancel", cx);
    })
    .unwrap();
    assert!(
        !directory
            .path()
            .join("visual_tests/new-suite.tests.yaml")
            .exists()
    );
}

#[gpui_kit::test]
async fn command_palette_filters_and_changes_theme_without_losing_the_draft(
    cx: &mut TestAppContext,
) {
    use crate::desktop::workspace::ThemePreference;
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-shift-p"
            } else {
                "ctrl-shift-p"
            },
            cx,
        );
    })
    .unwrap();
    let original = cx.update(|cx| {
        view.read(cx)
            .session
            .open
            .as_ref()
            .unwrap()
            .document
            .source()
            .to_string()
    });
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.input("dark theme", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("command:Dark theme").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("command-list").visible());
        assert!(window.try_find("command:Light theme").is_none());
        window.press("enter", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        view.read(cx).preferences.theme_preference() == ThemePreference::Dark
            && !view.read(cx).preferences_saving
    })
    .await;
    cx.update(|cx| {
        assert!(view.read(cx).session.dirty());
        assert_eq!(
            view.read(cx)
                .session
                .open
                .as_ref()
                .unwrap()
                .document
                .source(),
            original
        );
    });
}

#[gpui_kit::test]
async fn command_palette_keeps_disabled_actions_inert_and_handles_empty_search(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("open-commands", cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.input("save document", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("command:Save document").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("enter", cx);
        assert!(window.find("command-list").visible());
        assert!(!view.read(cx).busy);
        assert!(!view.read(cx).session.dirty());
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        window.input("does not match a command", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("commands-empty").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("command-list").is_none());
        assert!(window.find("field-url").visible());
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn preference_write_failures_do_not_lock_the_editor_and_can_be_retried(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let invalid_path = directory.path().join("settings-directory");
    std::fs::create_dir(&invalid_path).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.preferences_path = Some(invalid_path);
            this.save_preferences(window, cx);
            assert!(!this.busy);
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        !view.read(cx).preferences_saving
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).status.starts_with("Cannot save preferences:"));
        assert!(!view.read(cx).busy);
        assert!(!view.read(cx).session.dirty());
        view.update(cx, |this, cx| {
            this.preferences_path = Some(directory.path().join("recovered-settings.json"));
            this.save_preferences(window, cx);
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        !view.read(cx).preferences_saving
    })
    .await;
    assert!(directory.path().join("recovered-settings.json").is_file());
    cx.update(|cx| assert_eq!(view.read(cx).status, "Preferences saved"));
}

#[gpui_kit::test]
async fn display_preferences_are_nonblocking_and_latest_changes_are_persisted(
    cx: &mut TestAppContext,
) {
    use crate::desktop::workspace::{Preferences, ThemePreference};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let original = cx.update(|cx| {
        view.read(cx)
            .session
            .open
            .as_ref()
            .unwrap()
            .document
            .source()
            .to_string()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("editor-tabs").click(1_usize, cx);
        window.click("source-larger", cx);
        assert!(!view.read(cx).busy);
        window.click("source-larger", cx);
        window.click("source-larger", cx);
        window.click("source-wrap", cx);
        window.click("source-line-numbers", cx);
        view.update(cx, |this, cx| {
            this.change_theme(
                &super::design::ChooseTheme {
                    preference: ThemePreference::Dark,
                },
                window,
                cx,
            )
        });
        assert!(!view.read(cx).busy);
        assert_eq!(window.find("source-text-size").label(), Some("17 px"));
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        !view.read(cx).preferences_saving
    })
    .await;
    let preferences =
        Preferences::load(&directory.path().join("desktop-preferences.json")).unwrap();
    assert_eq!(preferences.source_font_size, 17);
    assert!(!preferences.source_wrap);
    assert!(!preferences.source_line_numbers);
    assert_eq!(preferences.theme_preference(), ThemePreference::Dark);
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            view.read(cx)
                .session
                .open
                .as_ref()
                .unwrap()
                .document
                .source(),
            original
        );
        assert!(!view.read(cx).session.dirty());
        window.click("source-reset-size", cx);
        assert_eq!(window.find("source-text-size").label(), Some("14 px"));
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        !view.read(cx).preferences_saving
    })
    .await;
    assert_eq!(
        Preferences::load(&directory.path().join("desktop-preferences.json"))
            .unwrap()
            .source_font_size,
        14
    );
}

#[gpui_kit::test]
async fn theme_choices_persist_without_changing_drafts_and_explicit_modes_ignore_system_changes(
    cx: &mut TestAppContext,
) {
    use crate::desktop::workspace::{Preferences, ThemePreference};
    use gpui_kit::component::ActiveTheme;
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        assert!(view.read(cx).session.dirty());
    })
    .unwrap();
    let original = cx.update(|cx| {
        view.read(cx)
            .session
            .open
            .as_ref()
            .unwrap()
            .document
            .source()
            .to_string()
    });
    for preference in [
        ThemePreference::Dark,
        ThemePreference::Light,
        ThemePreference::System,
    ] {
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.change_theme(&super::design::ChooseTheme { preference }, window, cx)
            });
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).preferences_saving
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            let expected = match preference {
                ThemePreference::System => "Appearance: System",
                ThemePreference::Light => "Appearance: Light",
                ThemePreference::Dark => "Appearance: Dark",
            };
            window.render_frame(cx);
            assert_eq!(window.find("theme-picker").label(), Some(expected));
            assert_eq!(
                view.read(cx)
                    .session
                    .open
                    .as_ref()
                    .unwrap()
                    .document
                    .source(),
                original
            );
            assert!(view.read(cx).session.dirty());
            assert!(matches!(
                cx.theme().scrollbar_mode,
                gpui_kit::component::scroll::ScrollbarMode::Always
            ));
            if preference != ThemePreference::System {
                assert_eq!(
                    cx.theme().mode.is_dark(),
                    preference == ThemePreference::Dark
                );
            }
            view.update(cx, |this, cx| {
                this.system_appearance_changed(gpui_kit::WindowAppearance::Dark, window, cx)
            });
            assert_eq!(
                cx.theme().mode.is_dark(),
                preference != ThemePreference::Light
            );
            view.update(cx, |this, cx| {
                this.system_appearance_changed(gpui_kit::WindowAppearance::Light, window, cx)
            });
            assert_eq!(
                cx.theme().mode.is_dark(),
                preference == ThemePreference::Dark
            );
        })
        .unwrap();
        assert_eq!(
            Preferences::load(&directory.path().join("desktop-preferences.json"))
                .unwrap()
                .theme_preference(),
            preference
        );
    }
}

#[gpui_kit::test]
fn workspace_header_displays_a_readable_path_without_changing_its_root(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let root = &view.read(cx).session.workspace.as_ref().unwrap().root;
        assert_eq!(root, &directory.path().canonicalize().unwrap());
        let expected = crate::desktop::workspace::display_path(root);
        assert_eq!(
            window.find("workspace-path").label(),
            Some(expected.as_str())
        );
        assert!(!expected.contains('\\'));
        assert!(!expected.starts_with("//?/"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn source_editor_registers_schema_completion(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update(|cx| {
        assert!(
            view.read(cx)
                .source
                .read(cx)
                .lsp()
                .completion_provider
                .is_some()
        );
        assert_eq!(
            view.read(cx).source.read(cx).language_name().as_ref(),
            "yaml"
        );
        assert_eq!(
            view.read(cx).resolved.read(cx).language_name().as_ref(),
            "json"
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let open = this
                .session
                .workspace
                .as_ref()
                .unwrap()
                .create_suite("json-completion.tests.json")
                .unwrap();
            this.session.install_document(open).unwrap();
            this.sync_editor(window, cx);
            assert_eq!(this.source.read(cx).language_name().as_ref(), "json");
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn source_completion_can_be_accepted_and_saved_without_duplicate_text(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("editor-tabs").click(1_usize, cx);
        view.read(cx).source.clone().update(cx, |state, cx| {
            state.set_value("scenarios:\n  - url: /\n    ", window, cx);
            state.set_selected_range(state.text().len()..state.text().len(), cx);
            window.focus(&state.focus_handle(cx), cx);
        });
        window.render_frame(cx);
        window.input("del", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        let menu = view.read(cx).source.read(cx).completion_menu_state();
        menu.open && menu.items.iter().any(|item| item.label == "delay")
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("source-completion-status").label(),
            Some("1 suggestion")
        );
        window.press("enter", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        view.read(cx).source.read(cx).value().contains("delay:")
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.input("250", cx);
        assert_eq!(
            view.read(cx).source.read(cx).value().as_ref(),
            "scenarios:\n  - url: /\n    delay: 250"
        );
        window.press("ctrl-s", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        !view.read(cx).busy && !view.read(cx).session.dirty()
    })
    .await;
    let value: serde_json::Value = serde_yaml::from_str(
        &std::fs::read_to_string(directory.path().join("visual_tests/ui-test.tests.yaml")).unwrap(),
    )
    .unwrap();
    assert_eq!(value["scenarios"][0]["delay"], 250);
    cx.update_window(handle.into(), |_, window, cx| {
        view.read(cx).source.clone().update(cx, |state, cx| {
            state.set_value("scenarios: []\n", window, cx);
            state.set_selected_range(state.text().len()..state.text().len(), cx);
            window.focus(&state.focus_handle(cx), cx);
        });
        window.render_frame(cx);
        window.press("ctrl-space", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        let menu = view.read(cx).source.read(cx).completion_menu_state();
        menu.open && menu.items.iter().any(|item| item.label == "browser")
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let cursor = view.read(cx).source.read(cx).cursor();
        window.press("down", cx);
        window.press("up", cx);
        assert_eq!(view.read(cx).source.read(cx).cursor(), cursor);
        window.press("escape", cx);
        assert!(!view.read(cx).source.read(cx).completion_menu_state().open);
    })
    .unwrap();
}

#[gpui_kit::test]
async fn source_tools_preserve_text_and_jump_to_syntax_errors(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("editor-tabs").click(1_usize, cx);
        let original = view.read(cx).source.read(cx).value();
        window.click("source-wrap", cx);
        assert!(!view.read(cx).source_wrap);
        window.click("source-larger", cx);
        assert_eq!(window.find("source-text-size").label(), Some("15 px"));
        window.click("source-smaller", cx);
        window.click("source-find", cx);
        assert!(view.read(cx).source.read(cx).search_session().open);
        view.read(cx)
            .source
            .clone()
            .update(cx, |state, cx| state.close_search(cx));
        window.render_frame(cx);
        window.click("source-replace", cx);
        assert!(view.read(cx).source.read(cx).search_session().replace_mode);
        assert_eq!(view.read(cx).source.read(cx).value(), original);
        assert!(!view.read(cx).session.dirty());
        view.read(cx).source.clone().update(cx, |state, cx| {
            state.close_search(cx);
            window.focus(&state.focus_handle(cx), cx);
        });
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input("scenarios:\n  - url: /\n    actions: [}", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("source-issue").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("source-issue", cx);
        assert_eq!(view.read(cx).source.read(cx).cursor_position().line, 2);
        assert_eq!(
            view.read(cx).source.read(cx).diagnostics().unwrap().len(),
            1
        );
        window.press("ctrl-a", cx);
        window.input("scenarios: []", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        view.read(cx).diagnostic.is_none()
    })
    .await;
    cx.update(|cx| {
        assert!(
            view.read(cx)
                .source
                .read(cx)
                .diagnostics()
                .unwrap()
                .is_empty()
        )
    });
}

#[gpui_kit::test]
async fn guided_viewport_menu_opens_after_filtering_and_accepts_pointer_selection(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("common")).unwrap();
    std::fs::write(
        directory.path().join("common/_viewports.yaml"),
        "- label: phone\n  width: 390\n  height: 844\n- label: desktop\n  width: 1280\n  height: 720\n",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("regressify.yaml"),
        "viewportNames: [desktop]\n",
    )
    .unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("ctrl-shift-f", cx);
        window.input("viewportNames", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("choices-viewportNames").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("choices-viewportNames", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.within("popup-menu").try_find(0_usize).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.within("popup-menu").find(0_usize).label(),
            Some("phone (390 x 844)")
        );
        window.within("popup-menu").click(0_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        view.read(cx).session.value().unwrap()["scenarios"][0]["viewportNames"]
            == serde_json::json!(["desktop", "phone"])
    })
    .await;
}

#[gpui_kit::test]
async fn guided_choices_keep_inherited_viewports_and_draft_dependencies_consistent(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("common")).unwrap();
    std::fs::write(directory.path().join("common/_viewports.yaml"), "- label: phone\n  width: 390\n  height: 844\n- label: desktop\n  width: 1280\n  height: 720\n").unwrap();
    std::fs::write(
        directory.path().join("regressify.yaml"),
        "viewportNames: [desktop]\n",
    )
    .unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.field_search
                .update(cx, |state, cx| state.set_value("viewportNames", window, cx))
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("choices-viewportNames").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.toggle_list_choice(
                &super::choices::ToggleListChoice {
                    key: "viewportNames".into(),
                    value: "phone".into(),
                },
                window,
                cx,
            );
            assert_eq!(
                this.session.value().unwrap()["scenarios"][0]["viewportNames"],
                serde_json::json!(["desktop", "phone"])
            );
            this.toggle_list_choice(
                &super::choices::ToggleListChoice {
                    key: "viewportNames".into(),
                    value: "desktop".into(),
                },
                window,
                cx,
            );
            this.toggle_list_choice(
                &super::choices::ToggleListChoice {
                    key: "viewportNames".into(),
                    value: "phone".into(),
                },
                window,
                cx,
            );
            assert_eq!(
                this.session.value().unwrap()["scenarios"][0]["viewportNames"],
                serde_json::json!(["phone"])
            );
            this.edit(
                |session| {
                    session.add_scenario()?;
                    session.set_field("label", Some(serde_json::json!("Log in first")))?;
                    session.select(Selection::Scenario(0))
                },
                window,
                cx,
            );
            this.field_search
                .update(cx, |state, cx| state.set_value("needs", window, cx));
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("choices-needs").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.toggle_list_choice(
                &super::choices::ToggleListChoice {
                    key: "needs".into(),
                    value: "log-in-first".into(),
                },
                window,
                cx,
            );
            assert_eq!(
                this.session.value().unwrap()["scenarios"][0]["needs"],
                serde_json::json!(["log-in-first"])
            );
            this.toggle_list_choice(
                &super::choices::ToggleListChoice {
                    key: "needs".into(),
                    value: "log-in-first".into(),
                },
                window,
                cx,
            );
            assert_eq!(
                this.session.value().unwrap()["scenarios"][0]["needs"],
                serde_json::json!([])
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn scenario_sequence_supports_reordering_duplication_and_direct_editing(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.edit(
                |session| {
                    session.set_field(
                        "actions",
                        Some(serde_json::json!([{"click": "#continue"}, {"wait": 500}])),
                    )
                },
                window,
                cx,
            );
            this.field_search
                .update(cx, |state, cx| state.set_value("actions", window, cx));
        });
        window.render_frame(cx);
        assert_eq!(window.find("sequence-count").label(), Some("2 actions"));
        window.click(("sequence-down", 0_usize), cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["actions"][0]["wait"],
            500
        );
        window.click("undo", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["actions"][0]["click"],
            "#continue"
        );
        window.click(("sequence-up", 0_usize), cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["actions"][0]["click"],
            "#continue"
        );
        window.click(("sequence-copy", 0_usize), cx);
        assert_eq!(window.find("sequence-count").label(), Some("3 actions"));
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
        window.click(("sequence-step", 1_usize), cx);
        assert_eq!(
            view.read(cx).session.selection,
            Selection::Action {
                scenario: 0,
                index: 1
            }
        );
        assert_eq!(window.find("field-click").value(), Some("#continue"));
    })
    .unwrap();
}

#[gpui_kit::test]
async fn sequence_removal_is_confirmed_and_pending_invalid_fields_block_reordering(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.edit(
                |session| {
                    session.set_field(
                        "actions",
                        Some(serde_json::json!([{"click": "#continue"}, {"wait": 500}])),
                    )
                },
                window,
                cx,
            );
            this.field_search
                .update(cx, |state, cx| state.set_value("actions", window, cx));
            let binding = this
                .bindings
                .iter()
                .find(|binding| binding.field.key == "delay")
                .unwrap();
            if let super::Control::Input(input) = &binding.control {
                input.update(cx, |state, cx| state.set_value("invalid draft", window, cx));
            }
        });
        window.render_frame(cx);
        window.click(("sequence-down", 0_usize), cx);
        assert!(view.read(cx).field_errors.contains_key("delay"));
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["actions"][0]["click"],
            "#continue"
        );
        view.update(cx, |this, cx| {
            let field = this
                .bindings
                .iter()
                .find(|binding| binding.field.key == "delay")
                .unwrap()
                .field
                .clone();
            this.reset_field(&field, window, cx);
        });
        window.render_frame(cx);
        window.click(("sequence-remove", 0_usize), cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["actions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("remove-confirm", cx)
    })
    .unwrap();
    cx.update(|cx| {
        let value = view.read(cx).session.value().unwrap();
        assert_eq!(
            value["scenarios"][0]["actions"].as_array().unwrap().len(),
            1
        );
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
    });
}

#[gpui_kit::test]
async fn problem_links_focus_drafts_and_resets_preserve_other_invalid_fields(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    for (key, draft) in [
        ("delay", "invalid delay"),
        ("misMatchThreshold", "invalid threshold"),
    ] {
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.field_search
                    .update(cx, |state, cx| state.set_value(key, window, cx))
            });
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
            window
                .try_find(format!("field-{key}"))
                .is_some_and(|field| field.visible())
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click(format!("field-{key}"), cx);
            window.input(draft, cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(2), |_, cx| {
            view.read(cx).field_errors.contains_key(key)
        })
        .await;
    }
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("overrides-only", cx);
        window.click("problem-delay", cx);
        assert!(!view.read(cx).overrides_only);
        assert_eq!(window.find("field-delay").value(), Some("invalid delay"));
        let binding = view
            .read(cx)
            .bindings
            .iter()
            .find(|binding| binding.field.key == "delay")
            .unwrap();
        let super::Control::Input(input) = &binding.control else {
            panic!("Expected numeric input");
        };
        assert!(input.read(cx).focus_handle(cx).is_focused(window));
        window.click("reset-delay", cx);
        assert!(!view.read(cx).field_errors.contains_key("delay"));
        assert!(view.read(cx).field_errors.contains_key("misMatchThreshold"));
        window.click("problem-misMatchThreshold", cx);
        assert_eq!(
            window.find("field-misMatchThreshold").value(),
            Some("invalid threshold")
        );
        window.click("reset-misMatchThreshold", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(view.read(cx).field_errors.is_empty());
        assert_eq!(window.find("field-misMatchThreshold").value(), Some(""));
        let value = view.read(cx).session.value().unwrap();
        assert!(value["scenarios"][0].get("delay").is_none());
        assert!(value["scenarios"][0].get("misMatchThreshold").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn navigation_and_duplication_flush_pending_field_notifications(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("field-url", cx);
        window.press("ctrl-a", cx);
        window.input("https://example.com/pending-navigation", cx);
        window.click("breadcrumb-document", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Root);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["url"],
            "https://example.com/pending-navigation"
        );
        view.update(cx, |this, cx| {
            this.select(Selection::Scenario(0), window, cx)
        });
        window.render_frame(cx);
        window.click("field-label", cx);
        window.press("ctrl-a", cx);
        window.input("Pending scenario label", cx);
        window.click("duplicate-item", cx);
        let value = view.read(cx).session.value().unwrap();
        assert_eq!(value["scenarios"].as_array().unwrap().len(), 2);
        assert_eq!(value["scenarios"][0]["label"], "Pending scenario label");
        assert_eq!(
            value["scenarios"][1]["url"],
            "https://example.com/pending-navigation"
        );
    })
    .unwrap();
}

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
        window.click("suite-run-reference", cx);
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
        window.click("run-test", cx);
        assert!(!view.read(cx).runs.read(cx).running());
        window.click("suite-check-ui-test", cx);
        window.click("suite-check-second", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_count(), 2);
        assert_eq!(window.find("suite-check-ui-test").checked(), Some(true));
        window.click("select-all-suites", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_count(), 0);
        window.click("run-test", cx);
        assert!(!view.read(cx).runs.read(cx).running());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn duplicate_workspace_open_activates_owner_without_discarding_current_draft(
    cx: &mut TestAppContext,
) {
    use crate::desktop::instance::{DesktopInstance, OpenInstance};
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let locks = first.path().join("instances");
    let owner = match DesktopInstance::open(&locks, second.path()).unwrap() {
        OpenInstance::Owned(owner) => owner,
        OpenInstance::Activated => panic!("Expected fixture ownership"),
    };
    let (handle, view) = editor(cx, first.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
            this.load_workspace(second.path().into(), window, cx);
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("status").is_some_and(|status| {
            status.label() == Some("Switched to the existing workspace window")
        })
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(owner.take_activation());
        assert_eq!(
            view.read(cx).session.workspace.as_ref().unwrap().root,
            first.path().canonicalize().unwrap()
        );
        assert!(view.read(cx).session.dirty());
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(1));
        assert_eq!(view.read(cx).runs.read(cx).selected_count(), 1);
        assert!(window.try_find("guard-save").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn reopening_current_workspace_keeps_its_unsaved_document(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        view.update(cx, |this, cx| {
            this.load_workspace(directory.path().join("."), window, cx)
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("status").is_some_and(|status| {
            status.label() == Some("Workspace is already open in this window")
        })
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).session.dirty());
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(1));
        assert!(window.try_find("guard-save").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn switching_workspaces_discards_old_selection_and_closing_unloads_everything(
    cx: &mut TestAppContext,
) {
    use crate::desktop::instance::{DesktopInstance, OpenInstance};
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let locks = first.path().join("instances");
    let (handle, view) = editor(cx, first.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.workspace_instance = match DesktopInstance::open(&locks, first.path()).unwrap() {
                OpenInstance::Owned(instance) => Some(instance),
                OpenInstance::Activated => panic!("Expected first fixture ownership"),
            };
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
        assert!(matches!(
            DesktopInstance::open(&locks, first.path()).unwrap(),
            OpenInstance::Owned(_)
        ));
        assert!(
            view.read(cx)
                .workspace_instance
                .as_ref()
                .unwrap()
                .owns_workspace(second.path())
                .unwrap()
        );
        view.update(cx, |this, cx| this.close_workspace(window, cx));
        assert!(view.read(cx).session.workspace.is_none());
        assert!(view.read(cx).runs.read(cx).output_root().is_err());
        assert!(view.read(cx).source.read(cx).value().is_empty());
        assert!(matches!(
            DesktopInstance::open(&locks, second.path()).unwrap(),
            OpenInstance::Owned(_)
        ));
    })
    .unwrap();
}

#[gpui_kit::test]
fn sidebar_scenario_log_combines_viewports_without_other_scenarios(cx: &mut TestAppContext) {
    use crate::progress::{Event, Job, Record};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let scenario = this.session.suites[0].scenarios[0].clone();
            let mut suites = this.session.suites.clone();
            suites[0].output_id = "saved-output".into();
            this.runs.update(cx, |panel, cx| {
                panel.update_catalog(suites, None, cx);
                for (id, viewport) in [(1, "Desktop"), (2, "Mobile")] {
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 0,
                        event: Event::Queued {
                            job: Job {
                                id,
                                suite_id: "saved-output".into(),
                                suite_name: "ui-test".into(),
                                scenario_id: scenario.id.clone(),
                                label: scenario.label.clone(),
                                viewport: viewport.into(),
                                captures: vec![format!("capture-{id}")],
                            },
                        },
                    });
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 10,
                        event: Event::Log {
                            id,
                            message: format!("Captured {viewport}"),
                        },
                    });
                }
                panel.manager.progress.apply(Record {
                    version: 1,
                    elapsed_ms: 0,
                    event: Event::Queued {
                        job: Job {
                            id: 3,
                            suite_id: "ui-test".into(),
                            suite_name: "ui-test".into(),
                            scenario_id: scenario.id.clone(),
                            label: "Another suite's output".into(),
                            viewport: String::new(),
                            captures: vec!["another-output".into()],
                        },
                    },
                });
                panel.manager.progress.apply(Record {
                    version: 1,
                    elapsed_ms: 10,
                    event: Event::Log {
                        id: 3,
                        message: "Other suite message".into(),
                    },
                });
                cx.notify();
            });
        });
        window.render_frame(cx);
        window.click(("sidebar-scenario-log", 0_usize), cx);
        assert_eq!(view.read(cx).main_tab, 1);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
        window.click("copy-run-log", cx);
        let text = cx.read_from_clipboard().unwrap().text().unwrap();
        assert!(text.contains("[Desktop] Captured Desktop"));
        assert!(text.contains("[Mobile] Captured Mobile"));
        assert!(!text.contains("Other suite message"));
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn clicking_scenario_activity_opens_only_its_attributed_log(cx: &mut TestAppContext) {
    use crate::progress::{Event, Job, Record};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| {
                for (id, label, message) in [
                    (1, "Checkout", "Click #place-order"),
                    (2, "Account", "Focus #login"),
                ] {
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 0,
                        event: Event::Queued {
                            job: Job {
                                id,
                                suite_id: "ui-test".into(),
                                suite_name: "ui-test".into(),
                                scenario_id: label.into(),
                                label: label.into(),
                                viewport: "Desktop".into(),
                                captures: vec![label.into()],
                            },
                        },
                    });
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 1234,
                        event: Event::Log {
                            id,
                            message: message.into(),
                        },
                    });
                }
                cx.notify();
            });
        });
        window.render_frame(cx);
        window.click(("live-row", 1_usize), cx);
        assert_eq!(
            window.find("run-log-heading").label(),
            Some("ui-test / Checkout / Desktop")
        );
        window.click("copy-run-log", cx);
        let text = cx.read_from_clipboard().unwrap().text().unwrap();
        assert!(text.contains("[00:01.234] Click #place-order"));
        assert!(!text.contains("#login"));
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.manager.progress.apply(Record {
                    version: 1,
                    elapsed_ms: 2000,
                    event: Event::Log {
                        id: 1,
                        message: "Screenshot saved".into(),
                    },
                });
                panel.show_scenario_log("ui-test", "Checkout", window, cx);
            })
        });
        window.render_frame(cx);
        window.click("copy-run-log", cx);
        assert!(
            cx.read_from_clipboard()
                .unwrap()
                .text()
                .unwrap()
                .contains("Screenshot saved")
        );
        window.click("log-activity", cx);
        window.click(("live-row", 2_usize), cx);
        window.click("copy-run-log", cx);
        let text = cx.read_from_clipboard().unwrap().text().unwrap();
        assert!(text.contains("#login"));
        assert!(!text.contains("#place-order"));
        window.click("log-all", cx);
        assert_eq!(window.find("run-log-heading").label(), Some("All run logs"));
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn live_dashboard_tracks_parallel_scenarios_and_filters_failures(cx: &mut TestAppContext) {
    use crate::desktop::runner::{RunCommand, RunOptions, RunOutcome, RunRequest};
    use crate::progress::{Event, Job, Record};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| {
                panel.manager.request = Some(RunRequest {
                    root: directory.path().canonicalize().unwrap(),
                    command: RunCommand::Test,
                    options: RunOptions::default(),
                });
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
                        scenario_id: None,
                        passed: true,
                        error: None,
                    },
                    Event::Captured {
                        id: 2,
                        error: Some("Missing #sign-in".into()),
                        infrastructure: false,
                    },
                    Event::Compared {
                        suite_id: "account".into(),
                        capture: "capture-2".into(),
                        label: "Home 2".into(),
                        scenario_id: None,
                        passed: false,
                        error: Some("Missing #sign-in".into()),
                    },
                ] {
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 1500,
                        event,
                    });
                }
                panel.manager.outcome = Some(RunOutcome::Completed);
                panel.choose(RunCommand::Reference, window, cx);
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
        assert_eq!(
            window.find("run-outcome-heading").label(),
            Some("1 failed, 1 passed")
        );
        assert_eq!(
            window.find("run-command-context").label(),
            Some("Last run: Run Test")
        );
        assert!(window.find("activity-view-results").visible());
        window.click("activity-failed-only", cx);
        assert!(window.try_find(("live-status", 1_usize)).is_none());
        assert!(window.find(("live-status", 2_usize)).visible());
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.manager.outcome = Some(RunOutcome::Cancelled);
                cx.notify();
            });
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("run-outcome-heading").label(),
            Some("Run stopped")
        );
        assert!(window.try_find("activity-view-results").is_none());
        assert!(window.find("activity-view-log").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn report_approval_pins_the_viewed_run_and_rejects_changed_output_identity(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let scenario = cx.update(|cx| view.read(cx).session.suites[0].scenarios[0].clone());
    let run = directory
        .path()
        .join(".regressify/ui-test/bitmaps_test/reviewed-run");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::write(run.join("report.json"), serde_json::to_vec(&serde_json::json!({"id": "ui-test", "tests": [
        {"status": "fail", "pair": {"label": "Checkout", "scenarioId": scenario.id, "scenarioLabel": "Checkout", "test": "../bitmaps_test/reviewed-run/checkout.png"}}
    ]})).unwrap()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("workbench-tabs").click(2_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("capture-heading")
            .is_some_and(|heading| heading.label() == Some("Checkout"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("approve-report", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("run-confirm").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.find("run-confirm-target").label(),
            Some("Run: reviewed-run")
        );
        assert_eq!(window.find("run-confirm").label(), Some("Approve Results"));
        assert_eq!(
            window.find("run-scope").label(),
            Some(format!("Scenarios: ui-test / {}", scenario.label).as_str())
        );
        assert!(
            view.read(cx)
                .runs
                .read(cx)
                .scenario_selected("ui-test", &scenario.id)
        );
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        assert!(!view.read(cx).runs.read(cx).running());
        let path = directory.path().join("visual_tests/ui-test.tests.yaml");
        std::fs::write(
            &path,
            format!(
                "id: changed-output\n{}",
                std::fs::read_to_string(&path).unwrap()
            ),
        )
        .unwrap();
        window.click("run-confirm", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("status").is_some_and(|status| {
            status
                .label()
                .is_some_and(|label| label.contains("report's suite output changed"))
        })
    })
    .await;
    cx.update(|cx| assert!(!view.read(cx).runs.read(cx).running()));
}

#[gpui_kit::test]
async fn overview_opens_multiple_saved_reports_with_independent_run_ids(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, _, cx| {
        view.update(cx, |this, cx| {
            this.session
                .workspace
                .as_ref()
                .unwrap()
                .create_suite("second-suite")
                .unwrap();
            this.session.refresh().unwrap();
            this.sync_suites(cx);
            this.overview = true;
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
        })
    })
    .unwrap();
    for (suite, run_id) in [("ui-test", "first-run"), ("second-suite", "second-run")] {
        let output = directory.path().join(".regressify").join(suite);
        let run = output.join("bitmaps_test").join(run_id);
        std::fs::create_dir_all(&run).unwrap();
        std::fs::create_dir_all(output.join("bitmaps_reference")).unwrap();
        let png = lodepng::encode32(&[255_u8; 4], 1, 1).unwrap();
        std::fs::write(run.join("home.png"), &png).unwrap();
        std::fs::write(output.join("bitmaps_reference/home.png"), &png).unwrap();
        std::fs::write(
            run.join("report.json"),
            serde_json::to_vec(
                &serde_json::json!({"id":suite,"tests":[{"status":"pass","pair":{"label":suite}}]}),
            )
            .unwrap(),
        )
        .unwrap();
    }
    cx.update(|cx| {
        view.update(cx, |this, cx| {
            this.runs
                .update(cx, |panel, cx| panel.refresh_artifacts(cx))
        })
    });
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("overview-open-report").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("overview-open-report", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("results-status")
            .is_some_and(|status| status.label() == Some("2 reports, 0 warnings"))
    })
    .await;
    cx.update(|cx| assert!(view.read(cx).runs.read(cx).manager.request.is_none()));
}

#[gpui_kit::test]
async fn selecting_suite_discovers_baseline_and_opens_saved_report_without_running(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    for (suite, label) in [
        ("ui-test", "Saved capture"),
        ("other", "Other suite capture"),
    ] {
        let output = directory.path().join(".regressify").join(suite);
        let run = output.join("bitmaps_test/existing-run");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::create_dir_all(output.join("bitmaps_reference")).unwrap();
        let png = lodepng::encode32(&[255_u8; 4], 1, 1).unwrap();
        std::fs::write(run.join("home.png"), &png).unwrap();
        std::fs::write(output.join("bitmaps_reference/home.png"), &png).unwrap();
        std::fs::write(
            run.join("report.json"),
            serde_json::to_vec(
                &serde_json::json!({"id":suite,"tests":[{"status":"pass","pair":{"label":label}}]}),
            )
            .unwrap(),
        )
        .unwrap();
    }
    let report_path = directory
        .path()
        .join(".regressify/ui-test/bitmaps_test/existing-run/report.json");
    let before = std::fs::read(&report_path).unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("suite-open-report").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).can_test_open_suite(cx));
        assert!(view.read(cx).runs.read(cx).manager.request.is_none());
        assert_eq!(
            window.find("suite-artifact-status").label(),
            Some("1 reference image / 1 saved test run")
        );
        window.click("suite-open-report", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("capture-heading")
            .is_some_and(|heading| heading.label() == Some("Saved capture"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.find("results-status").label(),
            Some("1 report, 0 warnings")
        );
        assert!(!view.read(cx).runs.read(cx).running());
        assert!(view.read(cx).runs.read(cx).manager.request.is_none());
    })
    .unwrap();
    assert_eq!(std::fs::read(report_path).unwrap(), before);
}

#[gpui_kit::test]
fn report_totals_and_access_survive_execution_errors(cx: &mut TestAppContext) {
    use crate::desktop::runner::{RunCommand, RunOptions, RunOutcome, RunRequest};
    use crate::progress::{Event, Job, Record};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| {
                panel.manager.request = Some(RunRequest {
                    root: directory.path().canonicalize().unwrap(),
                    command: RunCommand::Test,
                    options: RunOptions {
                        run_id: "mixed-run".into(),
                        ..Default::default()
                    },
                });
                for id in 1..=12 {
                    let capture = format!("capture-{id}");
                    let scenario_id = format!("scenario-{}", (id - 1) / 3);
                    for event in [
                        Event::Queued {
                            job: Job {
                                id,
                                suite_id: "ui-test".into(),
                                suite_name: "ui-test".into(),
                                scenario_id: scenario_id.clone(),
                                label: scenario_id.clone(),
                                viewport: format!("viewport-{}", (id - 1) % 3),
                                captures: vec![capture.clone()],
                            },
                        },
                        Event::Captured {
                            id,
                            error: Some("Browser cleanup failed".into()),
                            infrastructure: true,
                        },
                        Event::Compared {
                            suite_id: "ui-test".into(),
                            capture,
                            label: scenario_id.clone(),
                            scenario_id: Some(scenario_id),
                            passed: id <= 9,
                            error: (id > 9).then(|| "Missing selector".into()),
                        },
                    ] {
                        panel.manager.progress.apply(Record {
                            version: 1,
                            elapsed_ms: 100,
                            event,
                        });
                    }
                }
                panel
                    .manager
                    .progress
                    .end(false, Some("Browser cleanup failed".into()));
                panel.manager.outcome = Some(RunOutcome::Failed("Browser cleanup failed".into()));
                cx.notify();
            });
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("run-outcome-heading").label(),
            Some("Command failed")
        );
        assert_eq!(
            window.find("metric-total").label(),
            Some("Screenshot comparisons: 12")
        );
        assert_eq!(window.find("metric-passed").label(), Some("Passed: 9"));
        assert_eq!(window.find("metric-failed").label(), Some("Failed: 3"));
        assert_eq!(
            window.find(("live-status", 1_usize)).label(),
            Some("Passed")
        );
        assert_eq!(
            window.find("activity-view-results").label(),
            Some("Open Report")
        );
        assert!(window.find("activity-view-results").visible());
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.manager.outcome = Some(RunOutcome::Completed);
                cx.notify();
            })
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("run-outcome-heading").label(),
            Some("3 failed, 9 passed")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn open_report_compares_capture_only_runs_without_changing_their_scope(cx: &mut TestAppContext) {
    use crate::desktop::runner::{RunCommand, RunOptions, RunRequest};
    let directory = tempfile::tempdir().unwrap();
    let (_, view) = editor(cx, directory.path());
    cx.update(|cx| {
        let mut suites = view.read(cx).session.suites.clone();
        suites[0].output_id = "ui-output".into();
        let id = suites[0].scenarios[0].id.clone();
        let request = RunRequest {
            root: directory.path().canonicalize().unwrap(),
            command: RunCommand::Test,
            options: RunOptions {
                regressify_dir: ".executed-output".into(),
                run_id: "captured-run".into(),
                suites: vec!["ui-test".into()],
                scenarios: vec![crate::cli::args::ScenarioFilter {
                    suite: "ui-test".into(),
                    scenario: id.clone(),
                }],
                skip_compare: true,
                mis_match_threshold: 0.25,
                ..Default::default()
            },
        };
        let intent = super::runs::RunIntent::compare_run(
            request.clone(),
            &[("ui-test".into(), "ui-output".into())],
            true,
        )
        .unwrap();
        assert!(intent.review_after_run);
        let comparison = intent.resolve(&suites).unwrap();
        assert_eq!(comparison.command, RunCommand::Compare);
        assert_eq!(comparison.root, request.root);
        assert_eq!(comparison.options.run_id, "captured-run");
        assert_eq!(comparison.options.regressify_dir, ".executed-output");
        assert_eq!(comparison.options.suites, vec!["ui-output"]);
        assert_eq!(comparison.options.scenarios[0].scenario, id);
        assert_eq!(comparison.options.scenarios[0].suite, "ui-output");
        assert_eq!(comparison.options.mis_match_threshold, 0.25);
        suites[0].output_id = "changed-output".into();
        assert!(intent.resolve(&suites).is_err());
    });
}

#[gpui_kit::test]
async fn view_results_opens_the_executed_output_and_run_instead_of_current_settings(
    cx: &mut TestAppContext,
) {
    use crate::desktop::runner::{RunCommand, RunOptions, RunOutcome, RunRequest};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let suite = directory.path().join(".custom-output/ui-test");
    std::fs::create_dir_all(suite.join("html_report")).unwrap();
    std::fs::create_dir_all(suite.join("bitmaps_reference")).unwrap();
    lodepng::encode32_file(
        suite.join("bitmaps_reference/capture.png"),
        &[0_u8, 128, 255, 255],
        1,
        1,
    )
    .unwrap();
    for (run, label) in [
        ("old-run", "Executed capture"),
        ("newer-run", "Different capture"),
    ] {
        let path = directory
            .path()
            .join(".custom-output/ui-test/bitmaps_test")
            .join(run);
        std::fs::create_dir_all(&path).unwrap();
        lodepng::encode32_file(path.join("capture.png"), &[0_u8, 128, 255, 255], 1, 1).unwrap();
        std::fs::write(path.join("report.json"), serde_json::to_vec(&serde_json::json!({"id": "ui-test", "tests": [{"status": "pass", "pair": {
            "label": label, "reference": "../bitmaps_reference/capture.png", "test": format!("../bitmaps_test/{run}/capture.png")
        }}]})).unwrap()).unwrap();
    }
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| {
                panel.manager.request = Some(RunRequest {
                    root: directory.path().canonicalize().unwrap(),
                    command: RunCommand::Test,
                    options: RunOptions {
                        regressify_dir: ".custom-output".into(),
                        run_id: "old-run".into(),
                        ..Default::default()
                    },
                });
                panel.manager.outcome = Some(RunOutcome::Completed);
                cx.notify();
            });
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("activity-view-results").label(),
            Some("Open Report")
        );
        window.within("run-tabs").click(3_usize, cx);
        assert!(window.find("activity-view-results").visible());
        window.click("activity-view-results", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("capture-heading")
            .is_some_and(|heading| heading.label() == Some("Executed capture"))
            && window.try_find(("image-details", 0_usize)).is_some()
            && window.try_find(("image-details", 1_usize)).is_some()
    })
    .await;
    cx.update(|cx| assert_eq!(view.read(cx).main_tab, 2));
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find(("image-details", 0_usize)).visible());
        assert!(window.find(("image-details", 1_usize)).visible());
        view.update(cx, |this, cx| {
            this.results.update(cx, |panel, cx| {
                panel.show_run(
                    directory.path().join(".custom-output"),
                    "not-compared-yet".into(),
                    window,
                    cx,
                )
            })
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("results-status")
            .is_some_and(|status| status.label() == Some("0 reports, 0 warnings"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.find("capture-heading").label(), Some("Comparison"))
    })
    .unwrap();
}

#[gpui_kit::test]
async fn followup_approve_and_compare_use_the_last_matching_run_but_capture_stays_fresh(
    cx: &mut TestAppContext,
) {
    use crate::desktop::runner::{RunCommand, RunOptions, RunOutcome, RunRequest};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor_with_reference(cx, directory.path()).await;
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.select_all(true, cx);
                panel.manager.request = Some(RunRequest {
                    root: directory.path().canonicalize().unwrap(),
                    command: RunCommand::Test,
                    options: RunOptions {
                        run_id: "executed-run".into(),
                        suites: vec!["ui-test".into()],
                        ..Default::default()
                    },
                });
                panel.manager.outcome = Some(RunOutcome::Completed);
                for command in [RunCommand::Approve, RunCommand::Compare] {
                    panel.choose(command, window, cx);
                    assert_eq!(
                        panel.prepare_intent(cx).unwrap().request.options.run_id,
                        "executed-run"
                    );
                }
                for command in [RunCommand::Test, RunCommand::Reference] {
                    panel.choose(command, window, cx);
                    assert!(
                        panel
                            .prepare_intent(cx)
                            .unwrap()
                            .request
                            .options
                            .run_id
                            .is_empty()
                    );
                }
            })
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn scenario_editor_actions_keep_drafts_and_do_not_change_checkbox_scope_implicitly(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(1));
        window.click(("sidebar-scenario", 0_usize), cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(1));
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        window.click("scenario-run-reference", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("run-confirm").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(
            window
                .find("run-scope")
                .label()
                .unwrap()
                .starts_with("Scenarios: ui-test /")
        );
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        assert!(view.read(cx).session.dirty());
        window.click("run-confirm", cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("guard-save").label(), Some("Save and continue"));
        window.click("guard-cancel", cx);
        assert!(!view.read(cx).runs.read(cx).running());
        assert!(view.read(cx).session.dirty());
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(1));
    })
    .unwrap();
}

#[gpui_kit::test]
fn scenario_checklists_select_only_checked_scenarios_and_request_reference_directly(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let mut suites = this.session.suites.clone();
            suites[0].scenarios[0].id = "home".into();
            suites[0].scenarios[0].label = "Home".into();
            suites[0]
                .scenarios
                .push(crate::desktop::workspace::ScenarioSummary {
                    id: "checkout".into(),
                    label: "Checkout".into(),
                    url: "about:blank".into(),
                });
            this.runs
                .update(cx, |panel, cx| panel.update_catalog(suites, None, cx));
            this.main_tab = 1;
        });
        window.render_frame(cx);
        window.within("run-tabs").click(1_usize, cx);
        window.click("suite-expand-ui-test", cx);
        window.click("scenario-check-ui-test:checkout", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        assert!(
            !view
                .read(cx)
                .runs
                .read(cx)
                .scenario_selected("ui-test", "home")
        );
        assert!(!view.read(cx).runs.read(cx).all_selected("ui-test"));
        window.click("suite-check-ui-test", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 2);
        window.click("suite-check-ui-test", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 0);
        window.click("scenario-check-ui-test:checkout", cx);
        window.click("scenario-ref-ui-test:checkout", cx);
        assert!(window.find("run-confirm").visible());
        assert_eq!(
            window.find("run-scope").label(),
            Some("Scenarios: ui-test / Checkout")
        );
        assert_eq!(
            window.find("metric-total").label(),
            Some("Scenarios / viewports: 1")
        );
        assert!(!view.read(cx).runs.read(cx).running());
    })
    .unwrap();
}

#[gpui_kit::test]
fn scenario_run_intents_freeze_ids_and_revalidate_saved_metadata(cx: &mut TestAppContext) {
    use crate::desktop::runner::RunCommand;
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let suites = this.session.suites.clone();
            this.runs.update(cx, |panel, cx| {
                let id = suites[0].scenarios[0].id.clone();
                panel.set_scenario("ui-test", &id, cx).unwrap();
                panel.choose(RunCommand::Reference, window, cx);
                let intent = panel.prepare_intent(cx).unwrap();
                assert_eq!(intent.request.options.scenarios.len(), 1);
                assert_eq!(intent.request.options.scenarios[0].scenario, id);
                assert_eq!(intent.request.options.scenarios[0].suite, "ui-test");
                let mut fresh = suites.clone();
                fresh[0].output_id = "saved-output".into();
                let mut approval = intent.clone();
                approval.request.command = RunCommand::Approve;
                let request = approval.resolve(&fresh).unwrap();
                assert_eq!(request.options.suites, vec!["saved-output"]);
                assert_eq!(request.options.scenarios[0].suite, "saved-output");
                assert_eq!(
                    intent.resolve(&fresh).unwrap().options.scenarios[0].suite,
                    "ui-test"
                );
                panel.select_all(false, cx);
                assert_eq!(
                    intent.resolve(&fresh).unwrap().options.scenarios[0].scenario,
                    id
                );
                fresh[0].scenarios[0].id = "renamed".into();
                assert!(intent.resolve(&fresh).is_err());
                assert!(!panel.running());
            });
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn run_intents_freeze_command_options_and_scope_but_resolve_fresh_output_ids(
    cx: &mut TestAppContext,
) {
    use crate::desktop::runner::RunCommand;
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.select_all(true, cx);
                panel.choose(RunCommand::Approve, window, cx);
            });
            let intent = this
                .runs
                .update(cx, |panel, cx| panel.prepare_intent(cx))
                .unwrap();
            this.runs.update(cx, |panel, cx| {
                panel.choose(RunCommand::Reference, window, cx);
                panel.select_all(false, cx);
            });
            let mut fresh = this.session.suites.clone();
            fresh[0].output_id = "new-output-id".into();
            let request = intent.resolve(&fresh).unwrap();
            assert_eq!(request.command, RunCommand::Approve);
            assert_eq!(request.options.suites, vec!["new-output-id"]);
            assert_eq!(intent.suite_names, vec!["ui-test"]);
            fresh[0].name = "unrelated-suite".into();
            assert!(intent.resolve(&fresh).is_err());
            assert!(!this.runs.read(cx).running());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn run_preflight_rejects_a_selected_suite_changed_on_disk(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor_with_reference(cx, directory.path()).await;
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
            let intent = this
                .runs
                .update(cx, |panel, cx| panel.prepare_intent(cx))
                .unwrap();
            std::fs::write(
                directory.path().join("visual_tests/ui-test.tests.yaml"),
                "scenarios: [",
            )
            .unwrap();
            this.start_selected_run(intent, window, cx);
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| window.try_find("status").is_some_and(|status| status.label().is_some_and(|label| label.starts_with("Selected suites changed or contain errors; review the selection before running")))).await;
    cx.update(|cx| {
        assert_eq!(view.read(cx).runs.read(cx).selected_count(), 0);
        assert!(!view.read(cx).runs.read(cx).running());
        assert!(view.read(cx).session.suites[0].error.is_some());
    });
}

#[gpui_kit::test]
async fn navigator_search_lists_suites_but_only_shows_scenarios_from_the_open_suite(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.session.workspace.as_ref().unwrap().create_document(
                Path::new("visual_tests/checkout.tests.yaml"),
                crate::desktop::document::DocumentKind::Suite,
                "scenarios:\n  - label: Payment confirmation\n    url: https://example.com/receipt\n  - label: Address details\n    url: https://example.com/address\n",
            ).unwrap();
            this.session.refresh().unwrap();
            this.sync_suites(cx);
        });
        window.render_frame(cx);
        window.click("navigator-search", cx);
        window.input("confirmation", cx);
    }).unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("suite-count")
            .is_some_and(|count| count.label() == Some("1 suite"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("suite-pane").visible());
        assert!(window.find("scenario-pane").visible());
        assert_eq!(window.find("scenario-suite").label(), Some("ui-test"));
        assert_eq!(
            window.find(("scenario", 0_usize)).label(),
            Some("Scenario 1")
        );
        assert!(
            window
                .try_find("scenario:visual_tests/checkout.tests.yaml:0")
                .is_none()
        );
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.path == Path::new("visual_tests/checkout.tests.yaml"))
            .unwrap();
        window.click(("file", index), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("scenario-suite")
            .is_some_and(|suite| suite.label() == Some("checkout"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("scenario-count").label(), Some("2 of 2"));
        assert_eq!(
            window.find(("scenario", 0_usize)).label(),
            Some("Payment confirmation")
        );
        window.click(("scenario", 0_usize), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("field-url")
            .is_some_and(|field| field.value() == Some("https://example.com/receipt"))
    })
    .await;
    cx.update(|cx| {
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
        assert_eq!(
            view.read(cx).session.open.as_ref().unwrap().entry.path,
            Path::new("visual_tests/checkout.tests.yaml")
        );
        assert!(!view.read(cx).session.dirty());
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("scenario-search", cx);
        window.input("address", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("scenario-count")
            .is_some_and(|count| count.label() == Some("1 of 2"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find(("scenario", 0_usize)).is_none());
        assert_eq!(
            window.find(("scenario", 1_usize)).label(),
            Some("Address details")
        );
        window.click(("scenario", 1_usize), cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(1));
        assert_eq!(
            window.find("field-url").value(),
            Some("https://example.com/address")
        );
        assert_eq!(
            window.find("navigator-search").value(),
            Some("confirmation")
        );
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn scenario_pane_switching_respects_drafts_and_workspace_files_remain_accessible(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("regressify.yaml"), "{}\n").unwrap();
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
        });
        window.render_frame(cx);
        window.click("field-url", cx);
        window.press("ctrl-a", cx);
        window.input("https://example.com/draft", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |_, cx| {
        view.read(cx).session.dirty()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("second"))
            .unwrap();
        window.click(("file", index), cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("scenario-suite").label(), Some("ui-test"));
        window.click("guard-cancel", cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.find("field-url").value(),
            Some("https://example.com/draft")
        );
        window.click("workspace-files-toggle", cx);
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.kind == crate::desktop::document::DocumentKind::Workspace)
            .unwrap();
        window.click(("file", index), cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("guard-discard", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("scenario-empty")
            .is_some_and(|empty| empty.label() == Some("Select a test suite"))
    })
    .await;
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).session.open.as_ref().unwrap().entry.kind,
            crate::desktop::document::DocumentKind::Workspace
        )
    });
}

#[gpui_kit::test]
async fn scenario_quick_navigation_keeps_its_destination_after_saving(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        view.update(cx, |this, cx| {
            let open = this
                .session
                .workspace
                .as_ref()
                .unwrap()
                .create_suite("second")
                .unwrap();
            this.open_location(open.entry, Selection::Scenario(0), window, cx);
        });
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).session.dirty());
        window.click("guard-save", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Document loaded"))
    })
    .await;
    cx.update(|cx| {
        assert_eq!(
            view.read(cx)
                .session
                .open
                .as_ref()
                .unwrap()
                .entry
                .suite_name()
                .as_deref(),
            Some("second")
        );
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
        assert!(!view.read(cx).session.dirty());
    });
    let original: serde_json::Value = serde_yaml::from_str(
        &std::fs::read_to_string(directory.path().join("visual_tests/ui-test.tests.yaml")).unwrap(),
    )
    .unwrap();
    assert_eq!(original["scenarios"].as_array().unwrap().len(), 2);
}

#[gpui_kit::test]
async fn navigator_reports_empty_results_and_has_a_keyboard_entry(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, _) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-p"
            } else {
                "ctrl-p"
            },
            cx,
        );
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window
            .try_find("navigator-search")
            .is_some_and(|input| input.focused() == Some(true))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.input("does-not-exist", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("navigator-empty").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("navigator-search", cx);
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
        window.within("navigator-filter").click(1_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("navigator-empty").is_some()
    })
    .await;
}

#[gpui_kit::test]
async fn setting_search_reveals_advanced_fields_without_changing_the_draft(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| this.select(Selection::Root, window, cx));
        window.render_frame(cx);
        assert!(window.try_find("field-debug").is_none());
        window.click("field-search", cx);
        window.input("debug", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("field-debug").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(!view.read(cx).session.dirty());
        window.click("field-debug", cx);
        assert_eq!(view.read(cx).session.value().unwrap()["debug"], true);
        window.click("overrides-only", cx);
        assert!(window.find("field-debug").visible());
        window.click("reset-debug", cx);
        assert!(
            view.read(cx)
                .session
                .value()
                .unwrap()
                .get("debug")
                .is_none()
        );
        assert!(window.try_find("field-debug").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn numeric_stepper_uses_inherited_value_and_groups_can_collapse(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| this.select(Selection::Root, window, cx));
        window.render_frame(cx);
        window.click("field-search", cx);
        window.input("asyncCaptureLimit", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("field-url").is_none()
            && window.try_find("increase-asyncCaptureLimit").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("increase-asyncCaptureLimit", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["asyncCaptureLimit"],
            6
        );
        assert_eq!(window.find("field-asyncCaptureLimit").value(), Some("6"));
        window.click("field-search", cx);
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |this, cx| {
            this.collapsed_fields.insert(3);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("field-asyncCaptureLimit").is_none());
        window.click("expand-fields", cx);
        assert!(window.try_find("field-asyncCaptureLimit").is_some());
        assert_eq!(
            view.read(cx).session.value().unwrap()["asyncCaptureLimit"],
            6
        );
    })
    .unwrap();
}

async fn settle_dialog(cx: &mut TestAppContext, handle: WindowHandle<Root>) {
    // GPUI's dialog animation uses wall time, not the test executor's clock.
    std::thread::sleep(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
}

async fn editor_with_reference(
    cx: &mut TestAppContext,
    root: &Path,
) -> (WindowHandle<Root>, Entity<Workbench>) {
    let reference = root.join(".regressify/ui-test/bitmaps_reference/home.png");
    std::fs::create_dir_all(reference.parent().unwrap()).unwrap();
    lodepng::encode32_file(reference, &[255_u8; 4], 1, 1).unwrap();
    let (handle, view) = editor(cx, root);
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        view.read(cx).runs.read(cx).has_reference("ui-test")
    })
    .await;
    (handle, view)
}

pub(super) fn editor(
    cx: &mut TestAppContext,
    root: &Path,
) -> (WindowHandle<Root>, Entity<Workbench>) {
    editor_at_size(cx, root, size(px(1280.), px(820.)))
}

fn editor_at_size(
    cx: &mut TestAppContext,
    root: &Path,
    dimensions: gpui_kit::Size<gpui_kit::Pixels>,
) -> (WindowHandle<Root>, Entity<Workbench>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| super::design::apply_theme(false, None, cx));
    cx.update(super::bind_keys);
    let mut created = None;
    let handle = cx.open_window(dimensions, |window, cx| {
        let view = cx.new(|cx| Workbench::new(window, cx));
        view.update(cx, |this, cx| {
            this.preferences = crate::desktop::workspace::Preferences::default();
            this.preferences_path = Some(root.join("desktop-preferences.json"));
            this.apply_editor_display(window, cx);
            this.session.connect(root).unwrap();
            this.runs.update(cx, |panel, cx| {
                panel.set_workspace(root.canonicalize().unwrap(), window, cx)
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
fn compact_titlebar_preserves_editor_space_and_navigation_at_both_sizes(cx: &mut TestAppContext) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let titlebar = window.find("workspace-titlebar").bounds();
            assert!(titlebar.size.height <= px(38.));
            let tabs = window.find("titlebar-navigation").bounds();
            assert!(tabs.origin.y < px(38.));
            let open = window.find("open-workspace").bounds();
            assert!(
                open.origin.x + open.size.width <= px(width - 95.),
                "Titlebar overlaps window controls"
            );
            let navigation = window.find("workspace-navigation").bounds();
            assert!(navigation.origin.y <= px(39.));
            assert!(navigation.size.height >= px(height - 65.));
            assert!(window.find("field-url").bounds().size.width >= px(190.));
            assert!(window.find("suite-run-test").visible());
            window.within("workbench-tabs").click(1_usize, cx);
            assert_eq!(view.read(cx).main_tab, 1);
            window.within("workbench-tabs").click(0_usize, cx);
            assert_eq!(view.read(cx).main_tab, 0);
            assert!(window.find("field-url").visible());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
async fn layout_and_view_shortcuts_preserve_the_open_draft(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("field-url", cx);
        window.press(&format!("{modifier}-a"), cx);
        window.input("https://example.com/preserved", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |_, cx| {
        view.read(cx).session.value().unwrap()["scenarios"][0]["url"]
            == "https://example.com/preserved"
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.press(&format!("{modifier}-b"), cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, cx| {
        !view.read(cx).preferences_saving && window.try_find("workspace-navigation").is_none()
    })
    .await;
    let preferences = directory.path().join("desktop-preferences.json");
    assert!(
        crate::desktop::workspace::Preferences::load(&preferences)
            .unwrap()
            .navigator_hidden
    );
    cx.update_window(handle.into(), |_, window, cx| {
        window.press(&format!("{modifier}-2"), cx);
        assert_eq!(view.read(cx).main_tab, 1);
        window.press(&format!("{modifier}-3"), cx);
        assert_eq!(view.read(cx).main_tab, 2);
        window.press(&format!("{modifier}-1"), cx);
        assert_eq!(view.read(cx).main_tab, 0);
        assert_eq!(
            window.find("field-url").value(),
            Some("https://example.com/preserved")
        );
        assert!(view.read(cx).session.dirty());
        window.press(&format!("{modifier}-shift-f"), cx);
        assert!(
            view.read(cx)
                .field_search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        window.press(&format!("{modifier}-p"), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, cx| {
        !view.read(cx).preferences_saving && window.try_find("workspace-navigation").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(
            view.read(cx)
                .search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["url"],
            "https://example.com/preserved"
        );
    })
    .unwrap();
    assert!(
        !crate::desktop::workspace::Preferences::load(&preferences)
            .unwrap()
            .navigator_hidden
    );
}

#[gpui_kit::test]
fn action_breadcrumbs_navigate_without_mutating_the_document(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.edit(|session| session.add_action("click"), window, cx)
        });
        window.render_frame(cx);
        let source = view
            .read(cx)
            .session
            .open
            .as_ref()
            .unwrap()
            .document
            .source()
            .to_string();
        assert_eq!(window.find("editor-heading").label(), Some("1. click"));
        window.click("breadcrumb-scenario", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
        window.click("breadcrumb-document", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Root);
        assert_eq!(
            view.read(cx)
                .session
                .open
                .as_ref()
                .unwrap()
                .document
                .source(),
            source
        );
    })
    .unwrap();
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
async fn editor_run_test_preserves_pending_drafts_and_cancel_never_starts_a_worker(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor_with_reference(cx, directory.path()).await;
    let saved =
        std::fs::read_to_string(directory.path().join("visual_tests/ui-test.tests.yaml")).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("field-url", cx);
        window.press("ctrl-a", cx);
        window.input("https://example.com/unsaved-run", cx);
        window.click("suite-run-test", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("guard-cancel").is_some()
    })
    .await;
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(!view.read(cx).runs.read(cx).running());
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["url"],
            "https://example.com/unsaved-run"
        );
        window.click("guard-cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(!view.read(cx).runs.read(cx).running()));
    assert_eq!(
        std::fs::read_to_string(directory.path().join("visual_tests/ui-test.tests.yaml")).unwrap(),
        saved
    );
}

#[gpui_kit::test]
async fn run_settings_are_available_before_execution_and_validate_only_relevant_commands(
    cx: &mut TestAppContext,
) {
    use crate::desktop::runner::RunCommand;
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor_with_reference(cx, directory.path()).await;
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
        });
        window.render_frame(cx);
        window.within("run-tabs").click(2_usize, cx);
        assert!(window.try_find("run-field-heading").is_some());
        assert!(window.try_find("run-field-speed").is_some());
        window.click("run-settings-search", cx);
        window.input("optimization", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("run-field-siteEnv").is_none()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("run-field-speed", cx);
        window.press("ctrl-a", cx);
        window.input("invalid speed", cx);
        window.click("run-reference", cx);
        assert_eq!(window.find("run-confirm").label(), Some("Run Reference"));
        window.click("run-confirm-cancel", cx);
        window.within("run-tabs").click(2_usize, cx);
        assert_eq!(
            window.find("run-field-speed").value(),
            Some("invalid speed")
        );
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.choose(RunCommand::Optimize, window, cx);
                assert!(
                    panel.prepare_intent(cx).is_ok(),
                    "Rejected command leaves the previous valid command selected"
                );
            });
        });
        window.click("run-settings-search", cx);
        window.press("ctrl-a", cx);
        window.input("browser download", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("browser-download-control").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        for (index, expected) in [(1, Some(true)), (2, Some(false)), (0, None)] {
            window.within("run-field-fetchBrowser").click(index, cx);
            let intent = view
                .update(cx, |this, cx| {
                    this.runs.update(cx, |panel, cx| panel.prepare_intent(cx))
                })
                .unwrap();
            assert_eq!(intent.request.options.fetch_browser, expected);
        }
        window.click("run-settings-search", cx);
        window.press("ctrl-a", cx);
        window.input("compare after test", cx);
        assert_eq!(
            window.find("run-settings-search").value(),
            Some("compare after test")
        );
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("run-field-skipCompare").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("run-field-skipCompare").checked(), Some(true));
        window.click("run-field-skipCompare", cx);
        let intent = view
            .update(cx, |this, cx| {
                this.runs.update(cx, |panel, cx| {
                    panel.choose(RunCommand::Test, window, cx);
                    panel.prepare_intent(cx)
                })
            })
            .unwrap();
        assert!(intent.request.options.skip_compare);
        assert!(!view.read(cx).runs.read(cx).running());
    })
    .unwrap();
}

#[gpui_kit::test]
fn direct_commands_flush_pending_run_options_and_preserve_invalid_input(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
        });
        window.render_frame(cx);
        window.within("run-tabs").click(2_usize, cx);
        window.click("run-field-asyncCaptureLimit", cx);
        window.press("ctrl-a", cx);
        window.input("12", cx);
        window.click("run-reference", cx);
        assert_eq!(window.find("run-confirm").label(), Some("Run Reference"));
        window.click("run-confirm-cancel", cx);
        window.within("run-tabs").click(2_usize, cx);
        assert_eq!(
            window.find("run-field-asyncCaptureLimit").value(),
            Some("12")
        );
        window.click("run-field-asyncCaptureLimit", cx);
        window.press("ctrl-a", cx);
        window.input("invalid count", cx);
        window.click("run-reference", cx);
        assert!(window.try_find("run-confirm").is_none());
        assert_eq!(
            window.find("run-field-asyncCaptureLimit").value(),
            Some("invalid count")
        );
        assert!(!view.read(cx).runs.read(cx).running());
    })
    .unwrap();
}

#[gpui_kit::test]
fn direct_run_commands_do_not_require_a_mode_selection(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
        });
        window.render_frame(cx);
        assert!(
            window.try_find("run-reference").is_some(),
            "Reference must be a direct action"
        );
        assert!(window.try_find("choose-command").is_none());
        assert!(window.try_find("start-command").is_none());
        assert_eq!(window.find("run-test").label(), Some("Run Test"));
        assert_eq!(window.find("run-approve").label(), Some("Approve Results"));
        assert_eq!(window.find("run-compare").label(), Some("Compare Results"));
        window.click("run-reference", cx);
        assert_eq!(window.find("run-confirm").label(), Some("Run Reference"));
        assert!(!view.read(cx).runs.read(cx).running());
        window.click("run-confirm-cancel", cx);
        window.click("run-approve", cx);
        assert_eq!(window.find("run-confirm").label(), Some("Approve Results"));
        assert!(!view.read(cx).runs.read(cx).running());
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
        window.click("run-approve", cx);
        assert!(window.find("run-confirm").visible());
        assert!(!view.read(cx).runs.read(cx).running());
        window.click("run-confirm-cancel", cx);
        assert!(window.try_find("run-confirm").is_none());
        assert!(!view.read(cx).runs.read(cx).running());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn suite_commands_request_execution_directly_and_keep_editor_state(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("suite-run-reference", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("run-confirm").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(view.read(cx).main_tab, 1);
        assert!(window.find("run-test").visible());
        assert_eq!(window.find("run-scope").label(), Some("Suites: ui-test"));
        assert_eq!(window.find("run-confirm").label(), Some("Run Reference"));
        window.click("run-confirm-cancel", cx);
        window.within("run-tabs").click(2_usize, cx);
        assert!(window.try_find("run-field-asyncCaptureLimit").is_some());
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.choose(crate::desktop::runner::RunCommand::Snapshot, window, cx)
            })
        });
        window.render_frame(cx);
        assert!(window.try_find("run-field-asyncCaptureLimit").is_some());
        assert!(window.try_find("run-field-heading").is_some());
        window.within("workbench-tabs").click(0_usize, cx);
        assert!(window.find("field-url").visible());
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn results_show_scenario_names_instead_of_generated_capture_and_run_ids(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let run = directory
        .path()
        .join(".regressify/0123456789abcdef/bitmaps_test/desktop-0123456789abcdef0123456789abcdef");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::write(run.join("report.json"), serde_json::to_vec(&serde_json::json!({"id": "0123456789abcdef", "tests": [
        {"status": "pass", "pair": {"label": "8ac900fea19642cc0000 phone FINAL", "fileName": "8ac900fea19642cc0000_phone__FINAL.png", "misMatchThreshold": 0.1, "diff": {"rawMisMatchPercentage": 0.05}}},
        {"status": "fail", "pair": {"label": "8ac900fea19642cc0000 phone $receipt", "fileName": "8ac900fea19642cc0000_phone__$receipt.png", "scenarioId": "8ac900fea19642cc0000", "scenarioLabel": "Payment at capture time"}}
    ]})).unwrap()).unwrap();
    std::fs::write(directory.path().join("visual_tests/ui-test.tests.yaml"), "id: 0123456789abcdef\nscenarios:\n- id: 8ac900fea19642cc0000\n  label: Payment confirmation\n  url: https://example.com/receipt\n").unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let entry = this.session.open.as_ref().unwrap().entry.clone();
            this.session.load(entry).unwrap();
            this.sync_editor(window, cx);
        });
        window.render_frame(cx);
        window.within("workbench-tabs").click(2_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("capture-heading")
            .is_some_and(|heading| heading.label() == Some("Payment confirmation"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let report = window.find("report-picker");
        let label = report.label().unwrap_or_default();
        assert!(label.starts_with("ui-test / "), "{label}");
        assert!(!label.contains("desktop-"));
        assert_eq!(
            window.find(("capture", 0_usize)).label(),
            Some("Passed Payment confirmation")
        );
        assert_eq!(
            window.find("capture-context").label(),
            Some("Phone / Full page")
        );
        assert_eq!(
            window.find("capture-threshold").label(),
            Some("Allowed: 0.1%")
        );
        window.click("copy-capture-details", cx);
        let summary = cx.read_from_clipboard().unwrap().text().unwrap();
        assert!(summary.contains("Scenario: Payment confirmation"));
        assert!(summary.contains("Capture ID: 8ac900fea19642cc0000_phone__FINAL"));
        assert!(summary.contains("Page: https://example.com/receipt"));
        assert!(summary.contains("Allowed mismatch: 0.1%"));
        assert!(summary.contains("Mismatch: 0.050%"));
        window.click("next-capture", cx);
        assert_eq!(
            window.find("capture-heading").label(),
            Some("Payment at capture time")
        );
        window.click("capture-search", cx);
        window.input("8ac900", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("capture-position")
            .is_some_and(|position| position.label() == Some("2 of 2"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("capture-search", cx);
        window.press("ctrl-a", cx);
        window.input("at capture time", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("capture-position")
            .is_some_and(|position| position.label() == Some("1 of 1"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("edit-capture-suite", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        view.read(cx).main_tab == 0
    })
    .await;
    cx.update(|cx| {
        assert_eq!(view.read(cx).session.selection, Selection::Root);
        assert_eq!(
            view.read(cx)
                .session
                .open
                .as_ref()
                .unwrap()
                .entry
                .suite_name()
                .as_deref(),
            Some("ui-test")
        );
    });
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
        lodepng::encode32_file(path, &[0_u8, 128, 255, 255].repeat(1280 * 720), 1280, 720).unwrap();
    }
    let pair = serde_json::json!({"label": "Home", "reference": "../bitmaps_reference/home.png", "test": "../bitmaps_test/run-1/home.png", "diffImage": "../bitmaps_test/run-1/home_diff.png"});
    let report = serde_json::json!({"id": "shop", "tests": [
        {"status": "pass", "pair": pair},
        {"status": "fail", "pair": {"label": "Checkout", "engineErrorMsg": "Missing selector #pay"}},
        {"status": "pass", "pair": {"label": "Billing"}},
        {"status": "fail", "pair": {"label": "Receipt", "engineErrorMsg": "Unexpected receipt"}}
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
            Some("Current 1280 x 720")
        );
        assert!(
            window.find(("image-content", 1_usize)).bounds().size.width
                <= window.find(("image-scroll", 1_usize)).bounds().size.width
        );
        window.click("zoom-in", cx);
        assert_eq!(window.find("zoom-value").label(), Some("125%"));
        window.click("actual-size", cx);
        assert_eq!(window.find("zoom-value").label(), Some("100%"));
        assert_eq!(
            window.find(("image-content", 1_usize)).bounds().size.width,
            px(1280.)
        );
        window.click("fit-image", cx);
        assert!(
            window.find(("image-content", 1_usize)).bounds().size.width
                <= window.find(("image-scroll", 1_usize)).bounds().size.width
        );
        window.within("image-modes").click(3_usize, cx);
        assert!(window.find(("image-details", 0_usize)).visible());
        assert!(window.find(("image-details", 1_usize)).visible());
        window.click("expand-comparison", cx);
        assert!(window.try_find("capture-list").is_none());
        assert!(window.find(("image-details", 0_usize)).visible());
        assert!(window.find(("image-details", 1_usize)).visible());
        assert_eq!(window.find("capture-heading").label(), Some("Home"));
        window.click("expand-comparison", cx);
        assert!(window.find("capture-list").visible());
        window.within("result-filter").click(1_usize, cx);
        assert!(window.try_find(("capture", 0_usize)).is_none());
        assert_eq!(
            window
                .try_find("capture-error")
                .and_then(|error| error.label().map(str::to_string)),
            Some("Missing selector #pay".into()),
            "Filtering must not leave a passing capture in the preview"
        );
        assert_eq!(
            window.find("capture-error").label(),
            Some("Missing selector #pay")
        );
        assert_eq!(window.find("capture-position").label(), Some("1 of 2"));
        window.click("previous-capture", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Checkout"));
        window.click("next-capture", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Receipt"));
        assert_eq!(window.find("capture-position").label(), Some("2 of 2"));
        window.click("next-capture", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Receipt"));
        window.within("result-filter").click(0_usize, cx);
        window.click("previous-failed-capture", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Checkout"));
        window.click("next-failed-capture", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Receipt"));
        window.click("capture-search", cx);
        window.input("no matching capture", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("capture-empty").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("capture-position").label(), Some("0 of 0"));
        assert!(window.try_find("capture-error").is_none());
        assert!(window.try_find(("image-details", 1_usize)).is_none());
        window.click("next-capture", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Comparison"));
        window.click("capture-search", cx);
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window
            .try_find("capture-heading")
            .is_some_and(|heading| heading.label() == Some("Home"))
    })
    .await;
}

#[gpui_kit::test]
async fn result_navigation_keeps_the_selected_capture_page_visible(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let run = directory.path().join(".regressify/shop/bitmaps_test/run-1");
    std::fs::create_dir_all(&run).unwrap();
    let captures = (0..52)
        .map(|index| {
            serde_json::json!({
                "status": if index == 0 || index == 51 { "fail" } else { "pass" },
                "pair": { "label": format!("Capture {index}") }
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        run.join("report.json"),
        serde_json::to_vec(&serde_json::json!({"id": "shop", "tests": captures})).unwrap(),
    )
    .unwrap();
    let (handle, _) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("workbench-tabs").click(2_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("capture-heading")
            .is_some_and(|heading| heading.label() == Some("Capture 0"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("next-failed-capture", cx);
        assert_eq!(window.find("capture-position").label(), Some("52 of 52"));
        assert!(window.try_find(("capture", 51_usize)).is_some());
        assert!(window.try_find(("capture", 0_usize)).is_none());
        window.click("previous-capture", cx);
        window.click("previous-capture", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 49"));
        assert!(window.try_find(("capture", 49_usize)).is_some());
        assert!(window.try_find(("capture", 51_usize)).is_none());
        window.click("next-failed-capture", cx);
        window.within("result-filter").click(1_usize, cx);
        assert_eq!(window.find("capture-position").label(), Some("2 of 2"));
        assert!(window.try_find(("capture", 0_usize)).is_some());
        assert!(window.try_find(("capture", 51_usize)).is_some());
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
