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
        window.within("editor-tabs").click(0_usize, cx);
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
        window.within("editor-tabs").click(1_usize, cx);
        view.update(cx, |this, cx| {
            this.select(Selection::Scenario(0), window, cx)
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("field-url").value(),
            Some("https://example.com/from-source")
        );
        window.within("editor-tabs").click(0_usize, cx);
        window.focus(&source.read(cx).focus_handle(cx), cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        let incomplete = "# Keep this draft\nscenarios:\n- url: https://example.com\n  actions:\n  - click:\n  - hover:\n";
        window.input(incomplete, cx);
        window.within("editor-tabs").click(1_usize, cx);
        assert_eq!(view.read(cx).tab, 0);
        assert_eq!(view.read(cx).source_issues.len(), 2);
        assert_eq!(view.read(cx).source.read(cx).value().as_ref(), incomplete);
        window.render_frame(cx);
        window.within("editor-tabs").click(0_usize, cx);
        assert_eq!(view.read(cx).tab, 1);
        assert_eq!(view.read(cx).source.read(cx).value().as_ref(), incomplete);
        window.focus(&source.read(cx).focus_handle(cx), cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input("scenarios: [}", cx);
        let draft = view.read(cx).source.read(cx).value();
        window.within("editor-tabs").click(1_usize, cx);
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
        window.within("editor-tabs").click(0_usize, cx);
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
    accept_and_save_source_completion(cx, "enter", false).await;
}

#[gpui_kit::test]
async fn source_completion_can_be_accepted_with_tab_and_saved_without_duplicate_text(
    cx: &mut TestAppContext,
) {
    accept_and_save_source_completion(cx, "tab", false).await;
}

#[gpui_kit::test]
async fn source_completion_recovers_keyboard_focus_after_window_reactivation(
    cx: &mut TestAppContext,
) {
    accept_and_save_source_completion(cx, "enter", true).await;
}

#[gpui_kit::test]
async fn source_completion_recovers_tab_and_escape_after_window_reactivation(
    cx: &mut TestAppContext,
) {
    accept_and_save_source_completion(cx, "tab", true).await;
}

async fn reactivate_without_editor_focus(cx: &mut TestAppContext, handle: WindowHandle<Root>) {
    cx.update_window(handle.into(), |_, window, _| window.activate_window())
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.is_window_active()
    })
    .await;
    let other_directory = tempfile::tempdir().unwrap();
    let (other_handle, _) = editor(cx, other_directory.path());
    cx.update_window(other_handle.into(), |_, window, _| window.activate_window())
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        !window.is_window_active()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.blur(cx);
        window.activate_window();
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.is_window_active()
    })
    .await;
    cx.update_window(other_handle.into(), |_, window, _| window.remove_window())
        .unwrap();
}

async fn accept_and_save_source_completion(
    cx: &mut TestAppContext,
    accept_key: &str,
    reactivate: bool,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("editor-tabs").click(0_usize, cx);
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
    if reactivate {
        reactivate_without_editor_focus(cx, handle).await;
    }
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("source-completion-status").label(),
            Some("1 suggestion")
        );
        window.press(accept_key, cx);
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
    if reactivate {
        reactivate_without_editor_focus(cx, handle).await;
    }
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
fn source_tab_keeps_indentation_without_suggestions(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("editor-tabs").click(0_usize, cx);
        let original = "scenarios: []\n";
        view.read(cx).source.clone().update(cx, |state, cx| {
            state.set_value(original, window, cx);
            state.set_selected_range(original.len()..original.len(), cx);
            window.focus(&state.focus_handle(cx), cx);
        });
        window.render_frame(cx);
        assert!(!view.read(cx).source.read(cx).completion_menu_state().open);
        window.press("tab", cx);
        let value = view.read(cx).source.read(cx).value();
        let indentation = value.strip_prefix(original).unwrap();
        assert!(!indentation.is_empty());
        assert!(indentation.chars().all(|character| character == ' '));
        window.press("escape", cx);
        window.press("shift-tab", cx);
        assert_eq!(view.read(cx).source.read(cx).value().as_ref(), original);
    })
    .unwrap();
}

#[gpui_kit::test]
async fn source_focus_restoration_respects_other_controls_and_document_state(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, _| window.activate_window())
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.is_window_active()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let path = view
            .read(cx)
            .session
            .open
            .as_ref()
            .unwrap()
            .entry
            .path
            .clone();
        for (tab, main_tab, overview, busy) in [
            (0, 0, false, false),
            (1, 1, false, false),
            (1, 0, true, false),
            (1, 0, false, true),
        ] {
            window.blur(cx);
            view.update(cx, |this, cx| {
                this.tab = tab;
                this.main_tab = main_tab;
                this.overview = overview;
                this.busy = busy;
                this.source_focus_restore = Some(path.clone());
                this.source_window_activation_changed(window, cx);
                assert!(this.source_focus_restore.is_none());
            });
            assert!(window.focused(cx).is_none());
        }
        view.update(cx, |this, cx| {
            this.tab = 1;
            this.main_tab = 0;
            this.overview = false;
            this.busy = false;
            let other_focus = this.search.read(cx).focus_handle(cx);
            window.focus(&other_focus, cx);
            this.source_focus_restore = Some(path);
            this.source_window_activation_changed(window, cx);
            assert!(other_focus.is_focused(window));

            window.focus(&this.focus, cx);
            this.source_focus_restore = Some(directory.path().join("different.tests.yaml"));
            this.source_window_activation_changed(window, cx);
            assert!(this.focus.is_focused(window));

            this.source_window_activation_changed(window, cx);
            assert!(this.focus.is_focused(window));
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn schema_issue_panel_scrolls_and_keeps_text_inside_compact_and_large_windows(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1000., 720.), (1600., 1000.)] {
        for dark in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let (handle, view) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
            cx.update_window(handle.into(), |_, window, cx| {
                super::design::apply_theme(dark, None, cx);
                view.update(cx, |this, cx| {
                    let source = format!(
                        "scenarios: []\nbasicAuth:\n{}",
                        "- username: qa\n  password: synthetic\n".repeat(10)
                    );
                    this.session
                        .open
                        .as_mut()
                        .unwrap()
                        .document
                        .set_source(source)
                        .unwrap();
                    this.tab = 1;
                    this.sync_editor(window, cx);
                    this.source_issues[1].severity = Some(lsp_types::DiagnosticSeverity::WARNING);
                });
                window.render_frame(cx);
                let panel = window.find("diagnostic").bounds();
                assert!(
                    panel.bottom() <= px(height) && panel.right() <= px(width),
                    "{panel:?}"
                );
                assert!(panel.left() >= window.find("workspace-navigation").bounds().right());
                assert_eq!(panel.size.height, px(160.));
                assert_eq!(
                    window.find("source-issues-table").label(),
                    Some("Source issues")
                );
                let line_header = window.find("Line").bounds();
                let type_header = window.find("Type").bounds();
                let description_header = window.find("Description").bounds();
                for index in 0_usize..10 {
                    let text = window.find(("source-diagnostic-message", index)).bounds();
                    let jump = window.find(("source-diagnostic", index)).bounds();
                    let severity = window.find(("source-diagnostic-type", index));
                    let dot = window.find(("source-diagnostic-dot", index)).bounds();
                    assert_eq!(
                        severity.label(),
                        Some(if index == 1 { "Warning" } else { "Error" })
                    );
                    assert_eq!(jump.top(), text.top());
                    assert_eq!(severity.bounds().top(), text.top());
                    assert_eq!(jump.size.height, px(20.));
                    assert_eq!(jump.center().x, line_header.center().x);
                    assert_eq!(severity.bounds().left(), type_header.left());
                    assert_eq!(dot.center().x, type_header.center().x);
                    assert_eq!(dot.center().y, severity.bounds().center().y);
                    assert_eq!(text.left(), description_header.left());
                    assert!(text.left() >= severity.bounds().right());
                    assert!(text.right() <= panel.right(), "{text:?}, {panel:?}");
                    assert!(text.size.width > px(100.));
                    if index < 9 {
                        assert!(
                            text.bottom()
                                <= window
                                    .find(("source-diagnostic-message", index + 1))
                                    .bounds()
                                    .top()
                        );
                    }
                }
                assert!(
                    window.find(("source-diagnostic", 9_usize)).bounds().top() > panel.bottom()
                );
                window.scroll(
                    "source-issues-table",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-10_000.))),
                    cx,
                );
                window.render_frame(cx);
                let last = window.find(("source-diagnostic", 9_usize)).bounds();
                assert!(
                    last.top() >= panel.top() && last.bottom() <= panel.bottom(),
                    "{last:?}, {panel:?}"
                );
                window.click(("source-diagnostic", 9_usize), cx);
                assert_eq!(view.read(cx).source.read(cx).cursor_position().line, 20);
            })
            .unwrap();
        }
    }
}

#[gpui_kit::test]
fn schema_issue_descriptions_support_pointer_selection_and_plain_text_copy(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let message = "scenarios[0].actions[0].select: Expected a <select> element.\nExample: '#country' & \"region\"; **literal text**, not markup.";
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.tab = 1;
            this.source_issues = vec![lsp_types::Diagnostic {
                message: message.into(),
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                ..Default::default()
            }];
            this.diagnostic = Some(message.into());
            cx.notify();
        });
        window.render_frame(cx);
        let bounds = window.find(("source-diagnostic-message", 0_usize)).bounds();
        let start = gpui_kit::point(bounds.left() + px(1.), bounds.top() + px(10.));
        window.drag(start, start + gpui_kit::point(px(150.), px(0.)), cx);
        window.press("ctrl-c", cx);
        let selected = cx.read_from_clipboard().unwrap().text().unwrap();
        assert!(!selected.is_empty());
        assert!(message.contains(&selected), "{selected:?}");
        assert_ne!(selected, message);
        window.press("ctrl-a", cx);
        window.press("ctrl-c", cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), message);
        window.input("must not edit the issue", cx);
        assert_eq!(view.read(cx).source_issues[0].message, message);
        assert!(!view.read(cx).unsaved());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn schema_issues_have_precise_jumps_hover_help_and_keep_the_save_guard(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let path = directory.path().join("visual_tests/ui-test.tests.yaml");
    let original = "scenarios: []\n";
    std::fs::write(&path, original).unwrap();
    let draft = "scenarios:\n- url: /\n  actions:\n  - press: '#search'\nbasicAuth:\n- username: private-user\n  password: private-password\ndebugg: true\n";
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let entry = this.session.open.as_ref().unwrap().entry.clone();
            this.session.load(entry).unwrap();
            this.sync_editor(window, cx);
            assert!(
                this.session
                    .suites
                    .iter()
                    .any(|suite| suite.name == "ui-test" && suite.error.is_some())
            );
        });
        window.render_frame(cx);
        window.within("editor-tabs").click(0_usize, cx);
        window.focus(&view.read(cx).source.read(cx).focus_handle(cx), cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input(draft, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
        view.read(cx).source_issues.len() == 3
    })
    .await;
    let hover = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("source-issue").label(), Some("3 issues"));
            assert_eq!(
                view.read(cx).source.read(cx).diagnostics().unwrap().len(),
                3
            );
            let messages = view.read(cx).diagnostic.as_ref().unwrap();
            assert!(messages.contains("basicAuth[0].origin"));
            assert!(messages.contains("Did you mean 'debug'"));
            assert!(!messages.contains("private-user"));
            assert!(!messages.contains("private-password"));
            window.within("editor-tabs").click(1_usize, cx);
            assert_eq!(view.read(cx).tab, 0);
            window.render_frame(cx);
            window.within("editor-tabs").click(0_usize, cx);
            assert_eq!(view.read(cx).tab, 1);
            assert_eq!(view.read(cx).source.read(cx).value().as_ref(), draft);
            for (index, line) in [3, 5, 7].into_iter().enumerate() {
                window.click(("source-diagnostic", index), cx);
                assert_eq!(view.read(cx).source.read(cx).cursor_position().line, line);
            }
            assert_eq!(view.read(cx).source.read(cx).value().as_ref(), draft);
            window.click("save", cx);
            assert!(view.read(cx).session.dirty());
            assert!(!view.read(cx).busy);
            let source = view.read(cx).source.read(cx);
            let provider = source.lsp().hover_provider.clone().unwrap();
            let text = source.text().clone();
            provider.hover(&text, draft.find("password:").unwrap(), window, cx)
        })
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    let lsp_types::HoverContents::Markup(content) = hover.contents else {
        panic!("Expected schema hover text")
    };
    assert!(content.value.contains("${BASIC_AUTH_PASSWORD}"));
    assert!(content.value.contains("Required"));
    assert!(!content.value.contains("private-password"));
    assert_eq!(hover.range.unwrap().start, lsp_types::Position::new(6, 2));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    cx.update_window(handle.into(), |_, window, cx| {
        window.focus(&view.read(cx).source.read(cx).focus_handle(cx), cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input(
            "scenarios:\n- url: https://example.com\n  label: Corrected schema\n",
            cx,
        );
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
        view.read(cx).diagnostic.is_none()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(
            view.read(cx)
                .source
                .read(cx)
                .diagnostics()
                .unwrap()
                .is_empty()
        );
        window.render_frame(cx);
        window.click("save", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
        !view.read(cx).busy && !view.read(cx).session.dirty()
    })
    .await;
    cx.update(|cx| {
        let suite = view
            .read(cx)
            .session
            .suites
            .iter()
            .find(|suite| suite.name == "ui-test")
            .unwrap();
        assert!(suite.error.is_none(), "{:?}", suite.error);
        assert_eq!(suite.scenarios.len(), 1);
        assert!(
            !view
                .read(cx)
                .runs
                .read(cx)
                .artifact_summary("ui-test")
                .contains("configuration errors")
        );
    });
}

#[gpui_kit::test]
async fn source_tools_preserve_text_and_jump_to_syntax_errors(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("editor-tabs").click(0_usize, cx);
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
async fn switch_tab_can_be_added_from_navigation_menu(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.edit(
                |session| session.set_field("actions", Some(serde_json::json!([]))),
                window,
                cx,
            );
            this.field_search
                .update(cx, |state, cx| state.set_value("actions", window, cx));
        });
        window.render_frame(cx);
        window.click("sequence-add", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.within("popup-menu").try_find(1_usize).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.within("popup-menu").find(1_usize).label(),
            Some("switchTab")
        );
        window.within("popup-menu").click(1_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("field-switchTab").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.field_search
                .update(cx, |state, cx| state.set_value("", window, cx));
        });
        window.render_frame(cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["actions"][0],
            serde_json::json!({"switchTab": "new"})
        );
        assert!(window.try_find("field-frame").is_none());
        assert!(window.try_find("field-url").is_some());
        window.click("undo", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["actions"],
            serde_json::json!([])
        );
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
        let draft = view.read(cx).session.value().unwrap();
        window.press("end", cx);
        assert_eq!(
            view.read(cx)
                .sequence_list
                .current
                .as_ref()
                .map(|item| item.2),
            Some(2)
        );
        window.press("up", cx);
        assert_eq!(
            view.read(cx)
                .sequence_list
                .current
                .as_ref()
                .map(|item| item.2),
            Some(1)
        );
        window.press("home", cx);
        window.press("down", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
        assert_eq!(view.read(cx).session.value().unwrap(), draft);
        window.press("enter", cx);
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
    let filename = format!("{}_desktop__FINAL.png", scenario.id);
    lodepng::encode32_file(run.join(&filename), &[30_u8, 150, 180, 255], 1, 1).unwrap();
    std::fs::write(run.join("report.json"), serde_json::to_vec(&serde_json::json!({"id": "ui-test", "tests": [
        {"status": "fail", "pair": {"label": "Checkout", "scenarioId": scenario.id, "scenarioLabel": "Checkout", "fileName": filename, "test": format!("../bitmaps_test/reviewed-run/{filename}")}}
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
            && window.try_find(("image-details", 1_usize)).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("approve-report").is_none());
        window.click("approve-scenario", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("scenario-approval-confirm").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.find("scenario-approval-run").label(),
            Some("Run: reviewed-run")
        );
        assert_eq!(
            window.find("scenario-approval-confirm").label(),
            Some("Approve This Scenario")
        );
        assert_eq!(
            window.find("scenario-approval-viewport").label(),
            Some("Viewport: desktop")
        );
        assert_eq!(view.read(cx).main_tab, 2);
        assert!(window.try_find("run-confirm").is_none());
        window.click("scenario-approval-cancel", cx);
        assert!(window.try_find("scenario-approval-confirm").is_none());
        assert_eq!(view.read(cx).main_tab, 2);
        window.click("approve-scenario", cx);
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
        window.click("scenario-approval-confirm", cx);
        assert_eq!(view.read(cx).main_tab, 2);
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
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(!view.read(cx).runs.read(cx).running());
        assert_eq!(view.read(cx).main_tab, 2);
        assert!(
            window
                .find("scenario-approval-error")
                .label()
                .is_some_and(|label| label.contains("report's suite output changed"))
        );
    })
    .unwrap();
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
    let newer = directory
        .path()
        .join(".regressify/second-suite/bitmaps_test/newer-run");
    std::fs::create_dir_all(&newer).unwrap();
    std::fs::write(newer.join("report.json"), serde_json::to_vec(&serde_json::json!({
        "id": "second-suite", "tests": [{"status": "pass", "pair": {"label": "Newer unrequested capture"}}]
    })).unwrap()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("refresh-results", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("results-status")
            .is_some_and(|status| status.label() == Some("2 reports, 0 warnings"))
    })
    .await;
    for (name, label) in [
        ("second-suite", "second-suite / second-run"),
        ("ui-test", "ui-test / first-run"),
    ] {
        cx.update_window(handle.into(), |_, window, cx| {
            let index = view
                .read(cx)
                .session
                .entries
                .iter()
                .position(|entry| entry.suite_name().as_deref() == Some(name))
                .unwrap();
            window.click(("file", index), cx);
            assert_eq!(window.find("report-title").label(), Some(label));
            assert_eq!(view.read(cx).main_tab, 2);
        })
        .unwrap();
    }
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
async fn suite_panel_restores_all_saved_statuses_and_keeps_them_during_other_runs(
    cx: &mut TestAppContext,
) {
    use crate::desktop::runner::{RunCommand, RunOptions, RunOutcome, RunRequest};
    use crate::progress::{Event, Job, LiveProgress, Record};
    let directory = tempfile::tempdir().unwrap();
    for (suite, run, passed) in [
        ("ui-test", "first-run", false),
        ("other", "second-run", true),
    ] {
        let path = directory.path().join(format!(
            ".regressify/{suite}/bitmaps_test/{run}/report.json"
        ));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let tests = ["home_phone__FINAL", "home_phone__$step", "home_desktop__FINAL"]
            .map(|capture| serde_json::json!({
                "status": if passed || capture.ends_with("$step") { "pass" } else { "fail" },
                "pair": { "fileName": format!("{capture}.png"), "scenarioId": "home", "scenarioLabel": "Home" }
            }));
        std::fs::write(
            path,
            serde_json::to_vec(&serde_json::json!({ "id": suite, "tests": tests })).unwrap(),
        )
        .unwrap();
    }
    let (handle, view) = editor(cx, directory.path());
    let indices = cx
        .update_window(handle.into(), |_, _, cx| {
            view.update(cx, |this, cx| {
                this.session
                    .workspace
                    .as_ref()
                    .unwrap()
                    .create_suite("other")
                    .unwrap();
                this.session.refresh().unwrap();
                this.sync_suites(cx);
                ["ui-test", "other"].map(|name| {
                    this.session
                        .entries
                        .iter()
                        .position(|entry| entry.suite_name().as_deref() == Some(name))
                        .unwrap()
                })
            })
        })
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find(("suite-activity", indices[0]))
            .is_some_and(|status| status.label() == Some("2 failed / 2 finished"))
            && window
                .try_find(("suite-activity", indices[1]))
                .is_some_and(|status| status.label() == Some("2 passed"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.manager.request = Some(RunRequest {
                    root: directory.path().canonicalize().unwrap(),
                    command: RunCommand::Test,
                    options: RunOptions {
                        suites: vec!["other".into()],
                        ..Default::default()
                    },
                });
                panel.manager.progress = LiveProgress::default();
                for event in [
                    Event::Queued {
                        job: Job {
                            id: 1,
                            suite_id: "other".into(),
                            suite_name: "other".into(),
                            scenario_id: "home".into(),
                            label: "Home".into(),
                            viewport: "phone".into(),
                            captures: vec!["home_phone__FINAL".into()],
                        },
                    },
                    Event::Started { id: 1 },
                ] {
                    panel.manager.progress.apply(Record {
                        version: 1,
                        elapsed_ms: 1,
                        event,
                    });
                }
                cx.notify();
            });
        });
        window.render_frame(cx);
        assert_eq!(
            window.find(("suite-activity", indices[0])).label(),
            Some("2 failed / 2 finished")
        );
        assert_eq!(
            window.find(("suite-activity", indices[1])).label(),
            Some("1 running / 0 finished")
        );
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| {
                panel.manager.progress.apply(Record {
                    version: 1,
                    elapsed_ms: 5,
                    event: Event::Compared {
                        suite_id: "other".into(),
                        capture: "home_phone__FINAL".into(),
                        label: "Home".into(),
                        scenario_id: Some("home".into()),
                        passed: true,
                        error: None,
                    },
                });
                panel.manager.progress.end(false, None);
                panel.manager.outcome = Some(RunOutcome::Completed);
                let snapshot = panel.manager.snapshot().unwrap();
                panel.manager = Default::default();
                panel.restore_activity(None, Ok(Some(snapshot)), None, window, cx);
            });
        });
        window.render_frame(cx);
        assert_eq!(
            window.find(("suite-activity", indices[0])).label(),
            Some("2 failed / 2 finished")
        );
        assert_eq!(
            window.find(("suite-activity", indices[1])).label(),
            Some("1 passed")
        );
        assert_eq!(view.read(cx).runs.read(cx).progress().counts(None).total, 1);
    })
    .unwrap();

    let report = directory
        .path()
        .join(".regressify/other/bitmaps_test/second-run/report.json");
    let mut updated: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    updated["tests"][0]["status"] = "fail".into();
    std::fs::write(&report, serde_json::to_vec(&updated).unwrap()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.within("workbench-tabs").click(2_usize, cx);
        window.click(("file", indices[1]), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("report-title")
            .is_some_and(|title| title.label() == Some("other / second-run"))
            && window
                .try_find(("capture", 0_usize))
                .is_some_and(|capture| capture.label() == Some("Mismatch Home"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.find(("suite-activity", indices[1])).label(),
            Some("1 failed / 2 finished")
        );
    })
    .unwrap();
    updated["tests"][0]["status"] = "pass".into();
    std::fs::write(&report, serde_json::to_vec(&updated).unwrap()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("refresh-results", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find(("capture", 0_usize))
            .is_some_and(|capture| capture.label() == Some("Passed Home"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.find(("suite-activity", indices[1])).label(),
            Some("2 passed")
        );
        assert_eq!(
            window.find(("suite-activity", indices[0])).label(),
            Some("2 failed / 2 finished")
        );
    })
    .unwrap();
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
async fn scenario_row_selection_keeps_drafts_and_scopes_reference_to_selected_scenario(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(1));
        window.click(("scenario", 0_usize), cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
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
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
    })
    .unwrap();
}

#[gpui_kit::test]
fn suite_row_click_checks_the_selected_suite(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.runs
                .update(cx, |panel, cx| panel.select_all(false, cx));
        });
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("ui-test"))
            .unwrap();
        window.render_frame(cx);
        window.click(("file", index), cx);
        assert!(view.read(cx).runs.read(cx).all_selected("ui-test"));
        assert_eq!(view.read(cx).tab, 1, "Selecting a suite must open Source");
        assert_eq!(
            window.within("editor-tabs").find(0_usize).label(),
            Some("Source")
        );
        assert_eq!(
            window.within("editor-tabs").find(1_usize).label(),
            Some("Fields")
        );
        assert!(
            window.within("editor-tabs").find(0_usize).bounds().right()
                <= window.within("editor-tabs").find(1_usize).bounds().left()
        );
        window.within("editor-tabs").click(1_usize, cx);
        assert_eq!(view.read(cx).tab, 0);
        window.click(("file", index), cx);
        assert_eq!(view.read(cx).tab, 1);
    })
    .unwrap();
}

fn click_with_modifiers(
    window: &mut gpui_kit::Window,
    target: impl Into<gpui_kit::ElementId>,
    modifiers: gpui_kit::Modifiers,
    cx: &mut gpui_kit::App,
) {
    use gpui_kit::{InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    window.render_frame(cx);
    let target = window.find(target);
    assert!(target.visible());
    let position = target.bounds().center();
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers,
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

#[gpui_kit::test]
fn navigator_suite_selection_supports_modifiers_filters_and_empty_space(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, point};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            for name in ["a-ui", "middle"] {
                this.session
                    .workspace
                    .as_ref()
                    .unwrap()
                    .create_suite(name)
                    .unwrap();
            }
            this.session.refresh().unwrap();
            this.sync_suites(cx);
            this.main_tab = 2;
        });
        let indices = ["a-ui", "middle", "ui-test"].map(|name| {
            view.read(cx)
                .session
                .entries
                .iter()
                .position(|entry| entry.suite_name().as_deref() == Some(name))
                .unwrap()
        });
        let selected = |cx: &gpui_kit::App| view.read(cx).runs.read(cx).selected_names();
        let control = Modifiers {
            control: true,
            ..Default::default()
        };
        let shift = Modifiers {
            shift: true,
            ..Default::default()
        };
        let both = Modifiers {
            control: true,
            shift: true,
            ..Default::default()
        };
        window.render_frame(cx);
        assert!(window.try_find(("sidebar-suite", indices[0])).is_none());
        window.click(("file", indices[0]), cx);
        assert_eq!(selected(cx), ["a-ui"]);
        click_with_modifiers(window, ("file", indices[2]), control, cx);
        assert_eq!(selected(cx), ["a-ui", "ui-test"]);
        click_with_modifiers(window, ("file", indices[0]), control, cx);
        assert_eq!(selected(cx), ["ui-test"]);
        window.click(("file", indices[1]), cx);
        assert_eq!(selected(cx), ["middle"]);
        click_with_modifiers(window, ("file", indices[0]), shift, cx);
        assert_eq!(selected(cx), ["a-ui", "middle"]);
        click_with_modifiers(window, ("file", indices[2]), shift, cx);
        assert_eq!(selected(cx), ["middle", "ui-test"]);
        click_with_modifiers(window, ("file", indices[0]), both, cx);
        assert_eq!(selected(cx), ["a-ui", "middle", "ui-test"]);
        window.click(("file", indices[2]), cx);
        click_with_modifiers(window, ("file", indices[0]), shift, cx);
        assert_eq!(selected(cx), ["a-ui", "middle", "ui-test"]);
        let pane = window.find("suite-pane").bounds();
        let last = window.find(("suite-row", indices[2])).bounds();
        assert!(last.bottom() < pane.bottom() - px(12.));
        window.click_at("suite-pane", point(px(12.), pane.size.height - px(12.)), cx);
        assert!(selected(cx).is_empty());
        click_with_modifiers(window, ("file", indices[2]), shift, cx);
        assert_eq!(selected(cx), ["ui-test"]);
        window.click("sidebar-select-none", cx);
        assert!(selected(cx).is_empty());
        window.click("sidebar-select-all", cx);
        assert_eq!(selected(cx), ["a-ui", "middle", "ui-test"]);
        view.update(cx, |this, cx| {
            this.search
                .update(cx, |state, cx| state.set_value("ui", window, cx));
        });
        window.render_frame(cx);
        assert!(window.try_find(("file", indices[1])).is_none());
        window.click(("file", indices[0]), cx);
        click_with_modifiers(window, ("file", indices[2]), shift, cx);
        assert_eq!(selected(cx), ["a-ui", "ui-test"]);
        view.update(cx, |this, cx| {
            this.search
                .update(cx, |state, cx| state.set_value("ui-test", window, cx));
        });
        window.render_frame(cx);
        click_with_modifiers(window, ("file", indices[2]), shift, cx);
        assert_eq!(selected(cx), ["ui-test"]);
        window.click("sidebar-select-all", cx);
        assert_eq!(selected(cx), ["a-ui", "middle", "ui-test"]);
        window.click("sidebar-select-none", cx);
        assert!(selected(cx).is_empty());
        window.click(("file", indices[2]), cx);
        window.press("ctrl-a", cx);
        assert_eq!(selected(cx), ["ui-test"]);
        window.press("ctrl-shift-a", cx);
        assert!(selected(cx).is_empty());
        view.update(cx, |this, cx| {
            this.search
                .update(cx, |state, cx| state.set_value("", window, cx));
        });
        window.render_frame(cx);
        window.click(("file", indices[1]), cx);
        window.press("shift-up", cx);
        assert_eq!(selected(cx), ["a-ui", "middle"]);
        window.press("shift-down", cx);
        assert_eq!(selected(cx), ["middle"]);
        window.press("ctrl-shift-end", cx);
        assert_eq!(selected(cx), ["middle", "ui-test"]);
        window.press("ctrl-home", cx);
        assert_eq!(selected(cx), ["middle", "ui-test"]);
        window.press("ctrl-space", cx);
        assert_eq!(selected(cx), ["a-ui", "middle", "ui-test"]);
        window.press("space", cx);
        assert_eq!(selected(cx), ["middle", "ui-test"]);
        window.press("home", cx);
        assert_eq!(selected(cx), ["a-ui"]);
        window.press("end", cx);
        assert_eq!(selected(cx), ["ui-test"]);
        window.press("ctrl-a", cx);
        assert_eq!(selected(cx), ["a-ui", "middle", "ui-test"]);
        window.click("navigator-search", cx);
        window.press("ctrl-a", cx);
        window.input("no matching suites", cx);
        assert_eq!(selected(cx), ["a-ui", "middle", "ui-test"]);
        window.render_frame(cx);
        assert!(window.find("navigator-empty").visible());
        assert_eq!(view.read(cx).main_tab, 2);
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
    })
    .unwrap();
}

#[gpui_kit::test]
fn navigator_scenario_selection_supports_modifiers_filters_and_empty_space(
    cx: &mut TestAppContext,
) {
    use gpui_kit::{Modifiers, point};
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor_at_size(cx, directory.path(), size(px(1600.), px(1000.)));
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.session
                .workspace
                .as_ref()
                .unwrap()
                .create_suite("other-suite")
                .unwrap();
            this.session.refresh().unwrap();
            this.sync_suites(cx);
            let mut value = this.session.value().unwrap();
            let scenario = value["scenarios"][0].clone();
            value["scenarios"] = (0..5)
                .map(|index| {
                    let mut scenario = scenario.clone();
                    scenario["id"] = serde_json::json!(format!("scenario-{index}"));
                    scenario["label"] = serde_json::json!(format!(
                        "{} {index}",
                        if index % 2 == 0 { "Range" } else { "Other" }
                    ));
                    scenario
                })
                .collect();
            this.session
                .open
                .as_mut()
                .unwrap()
                .document
                .set_source(serde_yaml::to_string(&value).unwrap())
                .unwrap();
            this.sync_editor(window, cx);
        });
        let selected = |cx: &gpui_kit::App| {
            (0..5)
                .filter(|index| {
                    view.read(cx)
                        .runs
                        .read(cx)
                        .scenario_selected("ui-test", &format!("scenario-{index}"))
                })
                .collect::<Vec<_>>()
        };
        let control = Modifiers {
            control: true,
            ..Default::default()
        };
        let shift = Modifiers {
            shift: true,
            ..Default::default()
        };
        let both = Modifiers {
            control: true,
            shift: true,
            ..Default::default()
        };
        window.render_frame(cx);
        assert!(window.try_find(("sidebar-scenario", 0_usize)).is_none());
        window.click(("scenario", 0_usize), cx);
        assert_eq!(selected(cx), [0]);
        click_with_modifiers(window, ("scenario", 4_usize), control, cx);
        assert_eq!(selected(cx), [0, 4]);
        click_with_modifiers(window, ("scenario", 0_usize), control, cx);
        assert_eq!(selected(cx), [4]);
        window.click(("scenario", 2_usize), cx);
        assert_eq!(selected(cx), [2]);
        click_with_modifiers(window, ("scenario", 0_usize), shift, cx);
        assert_eq!(selected(cx), [0, 1, 2]);
        click_with_modifiers(window, ("scenario", 4_usize), shift, cx);
        assert_eq!(selected(cx), [2, 3, 4]);
        click_with_modifiers(window, ("scenario", 0_usize), both, cx);
        assert_eq!(selected(cx), [0, 1, 2, 3, 4]);
        window.click(("scenario", 4_usize), cx);
        click_with_modifiers(window, ("scenario", 0_usize), shift, cx);
        assert_eq!(selected(cx), [0, 1, 2, 3, 4]);
        let pane = window.find("scenario-pane").bounds();
        let last = window.find(("scenario-row", 4_usize)).bounds();
        assert!(last.bottom() < pane.bottom() - px(12.));
        window.click_at(
            "scenario-pane",
            point(px(12.), pane.size.height - px(12.)),
            cx,
        );
        assert!(selected(cx).is_empty());
        click_with_modifiers(window, ("scenario", 4_usize), shift, cx);
        assert_eq!(selected(cx), [4]);
        window.click("sidebar-scenarios-none", cx);
        assert!(selected(cx).is_empty());
        window.click("sidebar-scenarios-all", cx);
        assert_eq!(selected(cx), [0, 1, 2, 3, 4]);
        view.update(cx, |this, cx| {
            this.scenario_search
                .update(cx, |state, cx| state.set_value("Range", window, cx));
        });
        window.render_frame(cx);
        assert!(window.try_find(("scenario", 1_usize)).is_none());
        window.click(("scenario", 0_usize), cx);
        click_with_modifiers(window, ("scenario", 4_usize), shift, cx);
        assert_eq!(selected(cx), [0, 2, 4]);
        view.update(cx, |this, cx| {
            this.scenario_search
                .update(cx, |state, cx| state.set_value("Other", window, cx));
        });
        window.render_frame(cx);
        click_with_modifiers(window, ("scenario", 3_usize), shift, cx);
        assert_eq!(selected(cx), [3]);
        view.update(cx, |this, cx| {
            this.runs
                .update(cx, |panel, cx| panel.toggle_suite("other-suite", true, cx));
        });
        window.click(("scenario", 1_usize), cx);
        assert_eq!(selected(cx), [1]);
        assert!(view.read(cx).runs.read(cx).all_selected("other-suite"));
        window.click("sidebar-scenarios-none", cx);
        assert!(selected(cx).is_empty());
        assert!(view.read(cx).runs.read(cx).all_selected("other-suite"));
        window.click("sidebar-scenarios-all", cx);
        assert_eq!(selected(cx), [0, 1, 2, 3, 4]);
        assert!(view.read(cx).runs.read(cx).all_selected("other-suite"));
        window.click(("scenario", 1_usize), cx);
        window.press("ctrl-a", cx);
        assert_eq!(selected(cx), [1, 3]);
        assert!(view.read(cx).runs.read(cx).all_selected("other-suite"));
        window.press("ctrl-shift-a", cx);
        assert!(selected(cx).is_empty());
        assert!(view.read(cx).runs.read(cx).all_selected("other-suite"));
        window.click(("scenario", 1_usize), cx);
        window.press("shift-end", cx);
        assert_eq!(selected(cx), [1, 3]);
        window.press("shift-up", cx);
        assert_eq!(selected(cx), [1]);
        window.press("ctrl-end", cx);
        assert_eq!(selected(cx), [1]);
        window.press("ctrl-space", cx);
        assert_eq!(selected(cx), [1, 3]);
        window.press("space", cx);
        assert_eq!(selected(cx), [1]);
        window.press("enter", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(3));
        window.press("up", cx);
        assert_eq!(selected(cx), [1]);
        window.press("escape", cx);
        assert!(selected(cx).is_empty());
        assert!(view.read(cx).runs.read(cx).all_selected("other-suite"));
        assert!(view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn scenario_row_clicks_replace_selection_and_none_all_clear_it(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        window.click("duplicate-item", cx);
        window.click("sidebar-scenarios-all", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 3);
        window.click(("scenario", 1_usize), cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        let previous = view.read(cx).draft_suite_summary().unwrap().scenarios[1]
            .id
            .clone();
        window.click(("scenario", 2_usize), cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        assert!(
            !view
                .read(cx)
                .runs
                .read(cx)
                .scenario_selected("ui-test", &previous)
        );
        assert_eq!(view.read(cx).session.selection, Selection::Scenario(2));
        window.click("sidebar-scenarios-none", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 0);
        window.click("sidebar-scenarios-all", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 3);
        window.click("sidebar-select-none", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 0);
        assert!(view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn navigator_list_keyboard_select_all_and_clear_scenarios(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            for _ in 0..2 {
                this.edit(
                    crate::desktop::session::Session::duplicate_selected,
                    window,
                    cx,
                );
            }
        });
        window.render_frame(cx);
        window.click(("scenario", 0_usize), cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 3);
        window.press("escape", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 0);
        view.update(cx, |this, cx| this.select(Selection::Root, window, cx));
        window.render_frame(cx);
        window.within("workbench-tabs").click(1_usize, cx);
        window.within("run-tabs").click(1_usize, cx);
        window.click("select-all-suites", cx);
        window.press("ctrl-shift-a", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 0);
        window.within("workbench-tabs").click(0_usize, cx);
        window.click(("scenario", 0_usize), cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        assert!(view.read(cx).scenario_list.focus.is_focused(window));
        window.press("ctrl-a", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 3);
        window.press("escape", cx);
        window.click(("scenario", 0_usize), cx);
        view.update(cx, |this, cx| {
            this.busy = true;
            cx.notify();
        });
        for key in [
            "ctrl-a",
            "ctrl-shift-a",
            "escape",
            "shift-end",
            "ctrl-space",
        ] {
            window.press(key, cx);
            assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
            assert_eq!(view.read(cx).session.selection, Selection::Scenario(0));
        }
        view.update(cx, |this, cx| {
            this.busy = false;
            this.runs
                .update(cx, |panel, cx| panel.set_disabled(true, cx));
            cx.notify();
        });
        for key in ["ctrl-a", "ctrl-shift-a", "escape", "ctrl-space"] {
            window.press(key, cx);
            assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
        }
        assert!(view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn navigator_list_keyboard_scrolls_viewports_and_handles_empty_filters(cx: &mut TestAppContext) {
    use crate::desktop::{document::DocumentKind, workspace::Entry};

    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor_at_size(cx, directory.path(), size(px(1080.), px(720.)));
    let viewports = (0..40)
        .map(|index| {
            serde_json::json!({
                "label": format!("Viewport {index}"), "width": 1280, "height": 720
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        directory.path().join("viewports.yaml"),
        serde_yaml::to_string(&viewports).unwrap(),
    )
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.session.open = Some(
                this.session
                    .workspace
                    .as_ref()
                    .unwrap()
                    .open_document(Entry {
                        path: "viewports.yaml".into(),
                        kind: DocumentKind::Viewports,
                        editable: true,
                    })
                    .unwrap(),
            );
            this.session.selection = Selection::Viewport(0);
            this.sync_editor(window, cx);
        });
        window.render_frame(cx);
        window.click(("scenario", 0_usize), cx);
        window.press("down", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Viewport(1));
        window.press("end", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Viewport(39));
        let pane = window.find("scenario-pane").bounds();
        assert!(
            window.find(("scenario-row", 39_usize)).bounds().bottom() <= pane.bottom() + px(1.)
        );
        window.press("home", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Viewport(0));
        assert!(window.find(("scenario-row", 0_usize)).bounds().top() >= pane.top());
        view.update(cx, |this, cx| {
            this.scenario_search
                .update(cx, |state, cx| state.set_value("Viewport 1", window, cx));
        });
        window.render_frame(cx);
        window.press("home", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Viewport(1));
        window.press("end", cx);
        assert_eq!(view.read(cx).session.selection, Selection::Viewport(19));
        view.update(cx, |this, cx| {
            this.scenario_search.update(cx, |state, cx| {
                state.set_value("No matching viewport", window, cx)
            });
        });
        window.render_frame(cx);
        for key in ["up", "down", "home", "end", "ctrl-a", "ctrl-space", "enter"] {
            window.press(key, cx);
            assert_eq!(view.read(cx).session.selection, Selection::Viewport(19));
        }
        assert!(window.find("scenario-empty").visible());
        assert_eq!(
            view.read(cx).session.value().unwrap(),
            serde_json::json!(viewports)
        );
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn navigator_selection_controls_fit_both_window_sizes_and_themes(cx: &mut TestAppContext) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
        cx.update_window(handle.into(), |_, window, cx| {
            for dark in [false, true] {
                super::design::apply_theme(dark, None, cx);
                window.render_frame(cx);
                for (pane, none, all) in [
                    ("suite-pane", "sidebar-select-none", "sidebar-select-all"),
                    (
                        "scenario-pane",
                        "sidebar-scenarios-none",
                        "sidebar-scenarios-all",
                    ),
                ] {
                    let pane = window.find(pane).bounds();
                    let none = window.find(none);
                    let all = window.find(all);
                    assert!(none.visible() && all.visible());
                    assert!(pane.contains(&none.bounds().origin));
                    assert!(pane.contains(&none.bounds().bottom_right()));
                    assert!(pane.contains(&all.bounds().origin));
                    assert!(pane.contains(&all.bounds().bottom_right()));
                    assert!(none.bounds().right() <= all.bounds().left());
                }
                let filter = window.within("navigator-filter").find(1_usize).bounds();
                assert!(filter.right() <= window.find("sidebar-select-none").bounds().left());
                window.click("sidebar-select-all", cx);
                assert_eq!(view.read(cx).runs.read(cx).selected_count(), 1);
                window.click("sidebar-select-none", cx);
                assert_eq!(view.read(cx).runs.read(cx).selected_count(), 0);
                window.click("sidebar-scenarios-all", cx);
                assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 1);
                window.click("sidebar-scenarios-none", cx);
                assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 0);
            }
        })
        .unwrap();
    }
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
        window.press("ctrl-a", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 2);
        window.press("ctrl-shift-a", cx);
        assert_eq!(view.read(cx).runs.read(cx).selected_scenario_count(), 0);
        window.press("ctrl-a", cx);
        window.press("escape", cx);
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
        assert_eq!(view.read(cx).tab, 1, "Newly loaded suites must open Source");
        assert!(window.find("source-find").visible());
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
        assert_eq!(
            view.read(cx).tab,
            0,
            "Scenario selection must still open Fields"
        );
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
async fn workspace_files_text_documents_support_source_editing_and_save(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("common")).unwrap();
    let files = [
        ("common/_on-ready.js", "module.exports = async () => {};\n"),
        ("common/_on-ready.css", "body { color: red; }\n"),
        ("common/_override.css", "* { animation: none; }\n"),
    ];
    for (path, source) in files {
        std::fs::write(directory.path().join(path), source).unwrap();
    }
    std::fs::write(
        directory.path().join("common/0-binary.css"),
        b"binary\0data",
    )
    .unwrap();
    let (handle, view) = editor(cx, directory.path());
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.workspace_files_open = true;
            cx.notify();
        });
        window.render_frame(cx);
        let original = view
            .read(cx)
            .session
            .open
            .as_ref()
            .unwrap()
            .entry
            .path
            .clone();
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.path == Path::new("common/0-binary.css"))
            .unwrap();
        assert!(!view.read(cx).session.entries[index].supports_editing());
        window.click(("file", index), cx);
        view.update(cx, |this, cx| {
            this.workspace_list
                .focus_item("common/0-binary.css".into(), window, cx);
        });
        window.press("enter", cx);
        window.press("space", cx);
        assert!(!view.read(cx).busy);
        assert_eq!(
            view.read(cx).session.open.as_ref().unwrap().entry.path,
            original
        );
    })
    .unwrap();
    for (path, source) in files {
        cx.update_window(handle.into(), |_, window, cx| {
            let index = view
                .read(cx)
                .session
                .entries
                .iter()
                .position(|entry| entry.path == Path::new(path))
                .unwrap();
            assert!(view.read(cx).session.entries[index].supports_editing());
            if path.ends_with(".js") {
                view.update(cx, |this, cx| {
                    this.workspace_list.focus_item(path.into(), window, cx);
                });
                window.press("enter", cx);
            } else {
                window.click(("file", index), cx);
            }
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).busy
                && view.read(cx).session.open.as_ref().unwrap().entry.path == Path::new(path)
        })
        .await;
        let updated = format!("{source}\n/* Keep raw text: [ */\n");
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(view.read(cx).tab, 1);
            assert!(view.read(cx).source_issues.is_empty());
            assert!(view.read(cx).diagnostic.is_none());
            assert_eq!(
                view.read(cx).source.read(cx).language_name().as_ref(),
                "text"
            );
            assert!(window.try_find("advanced-fields").is_none());
            window.click("breadcrumb-document", cx);
            assert_eq!(view.read(cx).tab, 1);
            view.update(cx, |this, cx| {
                assert!(!this.select_editor_tab(0, window, cx));
                assert!(!this.select_editor_tab(2, window, cx));
                window.focus(&this.source.read(cx).focus_handle(cx), cx);
            });
            window.render_frame(cx);
            window.press(&format!("{modifier}-a"), cx);
            window.input(&updated, cx);
            window.press(&format!("{modifier}-z"), cx);
            assert_ne!(view.read(cx).source.read(cx).value().as_ref(), updated);
            window.press(&format!("{modifier}-shift-z"), cx);
            assert_eq!(view.read(cx).source.read(cx).value().as_ref(), updated);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            assert!(view.read(cx).session.dirty());
            assert!(view.read(cx).diagnostic.is_none());
            window.press(&format!("{modifier}-s"), cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            !view.read(cx).busy && !view.read(cx).session.dirty()
        })
        .await;
        assert_eq!(
            std::fs::read_to_string(directory.path().join(path)).unwrap(),
            updated
        );
    }
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("open-commands", cx);
        window.input("Save As", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("command:Save As").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("command:Save As", cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("file-name", cx);
        window.press(&format!("{modifier}-a"), cx);
        window.input("common/copied.css", cx);
        window.click("file-confirm", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        !view.read(cx).busy
            && view.read(cx).session.open.as_ref().unwrap().entry.path
                == Path::new("common/copied.css")
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(view.read(cx).tab, 1);
        assert!(window.find("source-find").visible());
        assert!(view.read(cx).source_issues.is_empty());
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("common/copied.css")).unwrap(),
        std::fs::read(directory.path().join("common/_override.css")).unwrap(),
    );
}

#[gpui_kit::test]
async fn workspace_files_scroll_within_sidebar_at_both_window_sizes(cx: &mut TestAppContext) {
    use std::path::PathBuf;

    use crate::desktop::{document::DocumentKind, workspace::Entry};

    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        for (dark, file_count) in [
            (false, 7_usize),
            (true, 7),
            (false, 8),
            (true, 8),
            (false, 9),
            (true, 9),
            (false, 12),
            (true, 12),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let (handle, view) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
            let existing_files = cx.update(|cx| {
                view.read(cx)
                    .session
                    .entries
                    .iter()
                    .filter(|entry| entry.kind != DocumentKind::Suite)
                    .count()
            });
            let additional_files = file_count
                .checked_sub(existing_files)
                .expect("Fixture defaults must fit within the requested file count");
            let entries = (0..additional_files)
                .map(|index| {
                    let path = PathBuf::from(format!("settings-{index:02}.yaml"));
                    std::fs::write(directory.path().join(&path), "{}\n").unwrap();
                    Entry {
                        path,
                        kind: DocumentKind::Workspace,
                        editable: true,
                    }
                })
                .collect::<Vec<_>>();
            let last_path = entries.last().unwrap().path.clone();
            cx.update_window(handle.into(), |_, window, cx| {
                super::design::apply_theme(dark, None, cx);
                let last_index = view.update(cx, |this, cx| {
                    this.session.entries.extend(entries);
                    assert_eq!(
                        this.session
                            .entries
                            .iter()
                            .filter(|entry| entry.kind != DocumentKind::Suite)
                            .count(),
                        file_count
                    );
                    this.workspace_files_open = true;
                    cx.notify();
                    this.session.entries.len() - 1
                });
                window.render_frame(cx);
                for path in ["common/_on-ready.js", "common/_override.css"] {
                    let index = view
                        .read(cx)
                        .session
                        .entries
                        .iter()
                        .position(|entry| entry.path == Path::new(path))
                        .expect("Common helpers must appear in Workspace files");
                    assert!(window.find(("file", index)).visible());
                    assert_eq!(window.find(("file", index)).label(), Some(path));
                    assert!(view.read(cx).session.entries[index].supports_editing());
                }
                let sidebar = window.find("workspace-navigation").bounds();
                let files = window.find("workspace-files").bounds();
                assert!(files.top() >= window.find("scenario-pane").bounds().bottom());
                assert!(files.bottom() <= sidebar.bottom());
                assert_eq!(
                    window.find(("file", last_index)).bounds().bottom() > files.bottom(),
                    file_count > 8,
                    "Short file lists must fit without scrolling"
                );
                window.scroll(
                    "workspace-files",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-10_000.))),
                    cx,
                );
                window.render_frame(cx);
                let last = window.find(("file", last_index)).bounds();
                assert!(last.top() >= files.top());
                assert!(
                    last.bottom() <= files.bottom() + px(1.),
                    "The last workspace file must scroll fully into view: {last:?}, viewport: {files:?}"
                );
                window.scroll(
                    "workspace-files",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(10_000.))),
                    cx,
                );
                window.render_frame(cx);
                if file_count > 8 {
                    let thumb = gpui_kit::point(files.right() - px(8.), files.top() + px(12.));
                    window.drag(
                        thumb,
                        gpui_kit::point(thumb.x, files.bottom() - px(8.)),
                        cx,
                    );
                    window.render_frame(cx);
                    let last = window.find(("file", last_index)).bounds();
                    assert!(last.top() >= files.top());
                    assert!(
                        last.bottom() <= files.bottom() + px(1.),
                        "The workspace file scrollbar must reach the last file"
                    );
                }
                window.click(("file", last_index), cx);
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
                view.read(cx)
                    .session
                    .open
                    .as_ref()
                    .is_some_and(|open| open.entry.path == last_path)
            })
            .await;
            cx.update_window(handle.into(), |_, window, cx| {
                let first_index = view
                    .read(cx)
                    .session
                    .entries
                    .iter()
                    .position(|entry| entry.kind != DocumentKind::Suite)
                    .unwrap();
                let last_index = view.read(cx).session.entries.len() - 1;
                window.press("home", cx);
                assert_eq!(
                    view.read(cx).workspace_list.current.as_ref(),
                    Some(&view.read(cx).session.entries[first_index].path)
                );
                let files = window.find("workspace-files").bounds();
                assert!(window.find(("file", first_index)).bounds().top() >= files.top());
                window.press("end", cx);
                assert_eq!(
                    view.read(cx).workspace_list.current.as_ref(),
                    Some(&last_path)
                );
                assert!(
                    window.find(("file", last_index)).bounds().bottom() <= files.bottom() + px(1.)
                );
                window.press("enter", cx);
                assert_eq!(
                    view.read(cx).session.open.as_ref().unwrap().entry.path,
                    last_path
                );
                let expanded = window.find("scenario-pane").bounds();
                window.click("workspace-files-toggle", cx);
                assert!(window.try_find("workspace-files").is_none());
                assert!(window.find("scenario-pane").bounds().bottom() > expanded.bottom());
                window.click("workspace-files-toggle", cx);
                assert!(window.find("workspace-files").visible());
                assert_eq!(window.find("scenario-pane").bounds(), expanded);
            })
            .unwrap();
        }
    }
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
        window.within("editor-tabs").click(1_usize, cx);
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
async fn pixel_match_threshold_controls_inherit_edit_and_reset(cx: &mut TestAppContext) {
    for (selection, parent, inherited) in [
        (Selection::Root, "", 10.0),
        (Selection::Scenario(0), "/scenarios/0", 3.0),
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("regressify.yaml"),
            "pixelMatchThreshold: 10\n",
        )
        .unwrap();
        let (handle, view) = editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.select(Selection::Root, window, cx);
                if selection == Selection::Scenario(0) {
                    this.edit(
                        |session| {
                            session.set_field("pixelMatchThreshold", Some(serde_json::json!(3.0)))
                        },
                        window,
                        cx,
                    );
                }
                this.select(selection.clone(), window, cx);
                this.select_editor_tab(0, window, cx);
            });
            window.render_frame(cx);
            assert!(!view.read(cx).advanced);
            assert!(window.try_find("field-pixelMatchThreshold").is_some());
            window.click("field-search", cx);
            window.input("pixelMatchThreshold", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window
                .try_find("increase-pixelMatchThreshold")
                .is_some_and(|field| field.visible())
                && window.try_find("field-url").is_none()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("increase-pixelMatchThreshold", cx);
            let value = view.read(cx).session.value().unwrap();
            let threshold = value.pointer(parent).unwrap()["pixelMatchThreshold"]
                .as_f64()
                .unwrap();
            assert!((threshold - inherited - 0.1).abs() < 1e-10);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window
                .try_find("hint-pixelMatchThreshold")
                .is_some_and(|hint| hint.label() == Some("Override"))
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("reset-pixelMatchThreshold", cx);
            assert!(
                view.read(cx)
                    .session
                    .value()
                    .unwrap()
                    .pointer(parent)
                    .unwrap()
                    .get("pixelMatchThreshold")
                    .is_none(),
                "{:?}",
                view.read(cx).field_errors
            );
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("field-pixelMatchThreshold", cx);
            window.press("ctrl-a", cx);
            window.input("0", cx);
            window.click("field-search", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx)
                .session
                .value()
                .unwrap()
                .pointer(parent)
                .unwrap()["pixelMatchThreshold"]
                .as_f64()
                == Some(0.0)
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("decrease-pixelMatchThreshold", cx);
            assert_eq!(
                view.read(cx)
                    .session
                    .value()
                    .unwrap()
                    .pointer(parent)
                    .unwrap()["pixelMatchThreshold"],
                0.0
            );
            window.click("reset-pixelMatchThreshold", cx);
            assert!(
                view.read(cx)
                    .session
                    .value()
                    .unwrap()
                    .pointer(parent)
                    .unwrap()
                    .get("pixelMatchThreshold")
                    .is_none()
            );
            view.read(cx)
                .session
                .open
                .as_ref()
                .unwrap()
                .document
                .validate()
                .unwrap();
        })
        .unwrap();
    }
}

#[gpui_kit::test]
async fn numeric_stepper_uses_inherited_value_and_groups_can_collapse(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| this.select(Selection::Root, window, cx));
        window.render_frame(cx);
        window.within("editor-tabs").click(1_usize, cx);
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

#[gpui_kit::test]
fn source_line_commands_toggle_selected_yaml_comments(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let original = "debug: true\nheadless: true\nscenarios: []\n";
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            assert!(this.select_editor_tab(1, window, cx));
            window.focus(&this.source.read(cx).focus_handle(cx), cx);
        });
        window.render_frame(cx);
        window.press(&format!("{modifier}-a"), cx);
        window.input(original, cx);
        let source = view.read(cx).source.clone();
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-up"
            } else {
                "ctrl-home"
            },
            cx,
        );
        window.press("shift-down", cx);
        window.press("shift-down", cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-shift-left"
            } else {
                "shift-home"
            },
            cx,
        );
        assert_eq!(
            source.read(cx).selected_range(),
            0..original.find("scenarios").unwrap()
        );
        window.render_frame(cx);
        assert_eq!(window.find("source-position").label(), Some("Ln 3, Col 1"));
        window.press(&format!("{modifier}-/"), cx);
        assert_eq!(
            view.read(cx).source.read(cx).value().as_ref(),
            "# debug: true\n# headless: true\nscenarios: []\n"
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            view.read(cx)
                .session
                .open
                .as_ref()
                .unwrap()
                .document
                .source(),
            "# debug: true\n# headless: true\nscenarios: []\n"
        );
        window.press(&format!("{modifier}-/"), cx);
        assert_eq!(view.read(cx).source.read(cx).value().as_ref(), original);
    })
    .unwrap();
}

#[gpui_kit::test]
fn source_line_commands_use_native_undo_and_redo(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let original = "debug: true\nheadless: true\nscenarios: []\n";
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            assert!(this.select_editor_tab(1, window, cx));
        });
        let source = view.read(cx).source.clone();
        for (key, expected) in [
            (
                format!("{modifier}-/"),
                "debug: true\n# headless: true\nscenarios: []\n",
            ),
            (
                "alt-shift-up".into(),
                "debug: true\nheadless: true\nheadless: true\nscenarios: []\n",
            ),
            (
                "alt-shift-down".into(),
                "debug: true\nheadless: true\nheadless: true\nscenarios: []\n",
            ),
            (
                "alt-up".into(),
                "headless: true\ndebug: true\nscenarios: []\n",
            ),
            (
                "alt-down".into(),
                "debug: true\nscenarios: []\nheadless: true\n",
            ),
            (
                format!("{modifier}-shift-k"),
                "debug: true\nscenarios: []\n",
            ),
        ] {
            source.update(cx, |state, cx| {
                state.set_value(original, window, cx);
                state.set_selected_range(12..26, cx);
                window.focus(&state.focus_handle(cx), cx);
            });
            window.render_frame(cx);
            window.press(&key, cx);
            assert_eq!(source.read(cx).value().as_ref(), expected, "{key}");
            window.press(&format!("{modifier}-z"), cx);
            assert_eq!(source.read(cx).value().as_ref(), original, "Undo {key}");
            window.press(&format!("{modifier}-shift-z"), cx);
            assert_eq!(source.read(cx).value().as_ref(), expected, "Redo {key}");
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn source_line_commands_preserve_native_unicode_crlf_and_reversed_selection(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let original = "label: caf\u{e9} \u{1f680}\r\ndebug: true\r\nscenarios: []\r\n";
    let end = original.find("scenarios").unwrap();
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            assert!(this.select_editor_tab(1, window, cx));
            this.source.update(cx, |state, cx| {
                state.set_value(original, window, cx);
                state.set_selected_range(end..0, cx);
                window.focus(&state.focus_handle(cx), cx);
            });
        });
        window.render_frame(cx);
        window.press(&format!("{modifier}-/"), cx);
        let source = view.read(cx).source.read(cx);
        assert_eq!(
            source.value().as_ref(),
            "# label: caf\u{e9} \u{1f680}\r\n# debug: true\r\nscenarios: []\r\n"
        );
        assert_eq!(source.cursor(), source.selected_range().start);
        window.press(&format!("{modifier}-/"), cx);
        let source = view.read(cx).source.read(cx);
        assert_eq!(source.value().as_ref(), original);
        assert_eq!(source.selected_range(), 0..end);
        assert_eq!(source.cursor(), 0);
    })
    .unwrap();
}

#[gpui_kit::test]
fn source_line_commands_respect_focus_busy_and_read_only_views(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            assert!(this.select_editor_tab(1, window, cx));
            window.focus(&this.source.read(cx).focus_handle(cx), cx);
            this.busy = true;
            cx.notify();
        });
        window.render_frame(cx);
        let original = view.read(cx).source.read(cx).value();
        for key in [
            format!("{modifier}-/"),
            "alt-shift-down".into(),
            format!("{modifier}-shift-k"),
        ] {
            window.press(&key, cx);
            assert_eq!(view.read(cx).source.read(cx).value(), original);
        }
        view.update(cx, |this, cx| {
            this.busy = false;
            window.focus(&this.search.read(cx).focus_handle(cx), cx);
            cx.notify();
        });
        window.render_frame(cx);
        for key in [
            format!("{modifier}-/"),
            "alt-down".into(),
            format!("{modifier}-shift-k"),
        ] {
            window.press(&key, cx);
            assert_eq!(view.read(cx).source.read(cx).value(), original);
        }
        view.update(cx, |this, cx| {
            assert!(this.select_editor_tab(2, window, cx));
            window.focus(&this.resolved.read(cx).focus_handle(cx), cx);
        });
        window.render_frame(cx);
        let resolved = view.read(cx).resolved.read(cx).value();
        for key in [
            format!("{modifier}-/"),
            "alt-shift-down".into(),
            format!("{modifier}-shift-k"),
        ] {
            window.press(&key, cx);
            assert_eq!(view.read(cx).source.read(cx).value(), original);
            assert_eq!(view.read(cx).resolved.read(cx).value(), resolved);
        }
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn source_line_commands_keep_json_strict_and_indent_selected_lines(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let original = "{\n  \"scenarios\": []\n}\n";
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            assert!(this.select_editor_tab(1, window, cx));
            this.session.open.as_mut().unwrap().document.format =
                crate::desktop::document::Format::Json;
            this.configure_source(cx);
            this.source.update(cx, |state, cx| {
                state.set_value(original, window, cx);
                state.set_selected_range(2..original.find('}').unwrap(), cx);
                window.focus(&state.focus_handle(cx), cx);
            });
        });
        window.render_frame(cx);
        window.press(&format!("{modifier}-/"), cx);
        assert_eq!(view.read(cx).source.read(cx).value().as_ref(), original);
        window.press(&format!("{modifier}-]"), cx);
        assert_eq!(
            view.read(cx).source.read(cx).value().as_ref(),
            "{\n    \"scenarios\": []\n}\n"
        );
        window.press(&format!("{modifier}-["), cx);
        assert_eq!(view.read(cx).source.read(cx).value().as_ref(), original);
        window.press("tab", cx);
        assert_eq!(
            view.read(cx).source.read(cx).value().as_ref(),
            "{\n    \"scenarios\": []\n}\n"
        );
        window.press("shift-tab", cx);
        assert_eq!(view.read(cx).source.read(cx).value().as_ref(), original);
        window.click("source-edit-menu", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("popup-menu").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.within("popup-menu").find(0_usize).label(),
            Some("Toggle Line Comment")
        );
        window.within("popup-menu").click(0_usize, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("popup-menu").is_some());
        assert_eq!(view.read(cx).source.read(cx).value().as_ref(), original);
        window.press("escape", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
async fn source_line_commands_menu_preserves_selection_and_returns_focus(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let original = "debug: true\nheadless: true\nscenarios: []\n";
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            assert!(this.select_editor_tab(1, window, cx));
            this.source.update(cx, |state, cx| {
                state.set_value(original, window, cx);
                state.set_selected_range(0..original.find("scenarios").unwrap(), cx);
                window.focus(&state.focus_handle(cx), cx);
            });
        });
        window.render_frame(cx);
        window.click("source-edit-menu", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("popup-menu").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.within("popup-menu").click(0_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, cx| {
        let source = view.read(cx).source.read(cx);
        source.value().as_ref() == "# debug: true\n# headless: true\nscenarios: []\n"
            && source.focus_handle(cx).is_focused(window)
            && view.read(cx).session.dirty()
    })
    .await;
}

pub(super) async fn settle_dialog(cx: &mut TestAppContext, handle: WindowHandle<Root>) {
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

pub(super) fn editor_at_size(
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
async fn workspace_external_menus_and_about_show_version_without_changing_drafts(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        assert!(view.read(cx).unsaved());
        assert_eq!(
            window.find("application-version").label(),
            Some(format!("v{}", env!("CARGO_PKG_VERSION")).as_str())
        );
        window.click("file-menu", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("popup-menu").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        for operation in [
            super::files::Operation::OpenVSCode,
            super::files::Operation::OpenFolder,
        ] {
            let entries = super::files::menu_entries();
            let (index, (label, _)) = entries
                .iter()
                .enumerate()
                .find(|(_, (_, entry))| entry.operation == operation)
                .unwrap();
            assert_eq!(
                window.within("popup-menu").find(index).label(),
                Some(*label)
            );
            assert!(view.read(cx).file_operation_enabled(operation, cx));
        }
        let root = &view.read(cx).session.workspace.as_ref().unwrap().root;
        let url = super::files::vscode_workspace_url(root).unwrap();
        assert_eq!(url.scheme(), "vscode");
        assert_eq!(url.host_str(), Some("file"));
        assert_eq!(
            url.path().trim_start_matches('/'),
            super::workspace::display_path(root).trim_start_matches('/')
        );
        window.press("escape", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("popup-menu").is_none()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| window.click("help-menu", cx))
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("popup-menu").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.within("popup-menu").find(0_usize).label(),
            Some("About")
        );
        window.within("popup-menu").click(0_usize, cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.find("about-application").label(),
            Some("Regressify Desktop")
        );
        assert_eq!(
            window.find("about-version").label(),
            Some(format!("Version: {}", env!("CARGO_PKG_VERSION")).as_str())
        );
        assert_eq!(
            window.find("about-author").label(),
            Some("Author: Tuyen Pham")
        );
        assert_eq!(
            window.find("about-website").label(),
            Some("https://tuyen.blog/")
        );
        window.click("about-close", cx);
        assert!(view.read(cx).unsaved());
        view.update(cx, |this, _| this.session.workspace = None);
        assert!(
            !view
                .read(cx)
                .file_operation_enabled(super::files::Operation::OpenVSCode, cx)
        );
        assert!(
            !view
                .read(cx)
                .file_operation_enabled(super::files::Operation::OpenFolder, cx)
        );
    })
    .unwrap();
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
            let logo = window.find("application-logo").bounds();
            assert_eq!(logo.size, size(px(22.), px(22.)));
            assert!(logo.top() >= titlebar.top() && logo.bottom() <= titlebar.bottom());
            assert_eq!(
                window.find("application-logo").label(),
                Some("Regressify logo")
            );
            let status = window.find("status").bounds();
            assert!(status.top() >= titlebar.top() && status.bottom() <= titlebar.bottom());
            assert_eq!(
                window.find("application-version").label(),
                Some(format!("v{}", env!("CARGO_PKG_VERSION")).as_str())
            );
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
            assert!(navigation.bottom() >= px(height - 2.));
            assert!(window.find("field-url").bounds().size.width >= px(190.));
            assert!(window.find("suite-run-test").visible());
            window.within("workbench-tabs").click(1_usize, cx);
            assert_eq!(view.read(cx).main_tab, 1);
            window.within("workbench-tabs").click(0_usize, cx);
            assert_eq!(view.read(cx).main_tab, 0);
            assert!(window.find("field-url").visible());
            window.within("workbench-tabs").click(2_usize, cx);
            assert!(window.find("results-panel").bounds().bottom() >= px(height - 2.));
            assert!(
                window.find("results-status").bounds().bottom()
                    <= window.find("capture-heading").bounds().top()
            );
        })
        .unwrap();
    }
}

#[gpui_kit::test]
async fn overview_suite_columns_align_with_headers_at_both_sizes(cx: &mut TestAppContext) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let captures = directory
            .path()
            .join(".regressify/ui-test/bitmaps_test/layout-run");
        let references = directory
            .path()
            .join(".regressify/ui-test/bitmaps_reference");
        let png = lodepng::encode32(&[255_u8; 4], 1, 1).unwrap();
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::create_dir_all(&references).unwrap();
        std::fs::write(captures.join("home.png"), &png).unwrap();
        std::fs::write(references.join("home.png"), &png).unwrap();
        std::fs::write(
            captures.join("report.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": "ui-test", "tests": [{"status": "pass", "pair": {"label": "Home"}}]
            }))
            .unwrap(),
        )
        .unwrap();
        let (handle, view) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.session
                    .workspace
                    .as_ref()
                    .unwrap()
                    .create_suite("without-saved-reports")
                    .unwrap();
                this.session.refresh().unwrap();
                this.sync_suites(cx);
                this.overview = true;
                this.runs
                    .update(cx, |panel, cx| panel.refresh_artifacts(cx));
                cx.notify();
            });
            window.render_frame(cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
            view.read(cx).runs.read(cx).can_open_saved_suite("ui-test")
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            for dark in [false, true] {
                super::design::apply_theme(dark, Some(window), cx);
                window.render_frame(cx);
                let suite_count = view.read(cx).session.suites.len();
                assert_eq!(suite_count, 2);
                let mut report_buttons = 0;
                for index in 0..suite_count {
                    for (heading, cell) in [
                        ("overview-scenarios-header", "overview-scenarios"),
                        ("overview-status-header", "overview-status"),
                        ("overview-suite-header", "overview-suite"),
                        ("overview-actions-header", "overview-actions"),
                    ] {
                        let header = window.find(heading).bounds();
                        let value = window.find((cell, index)).bounds();
                        assert_eq!(value.left(), header.left(), "Misaligned {heading}");
                        assert_eq!(
                            value.size.width, header.size.width,
                            "Unequal {heading} width"
                        );
                    }
                    let actions = window.find(("overview-actions", index)).bounds();
                    let edit = window.find(("overview-edit", index)).bounds();
                    assert!(edit.left() >= actions.left());
                    assert!(edit.right() <= actions.right());
                    if let Some(report) = window.try_find(("overview-report", index)) {
                        report_buttons += 1;
                        let report = report.bounds();
                        assert!(report.left() >= actions.left());
                        assert!(report.right() <= edit.left());
                    }
                }
                assert_eq!(report_buttons, 1);
            }
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn panel_tabs_align_with_content_at_both_window_sizes(cx: &mut TestAppContext) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let (handle, _) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
        for dark in [false, true] {
            cx.update_window(handle.into(), |_, window, cx| {
                super::design::apply_theme(dark, Some(window), cx);
                window.render_frame(cx);
                window.within("workbench-tabs").click(0_usize, cx);
                assert_eq!(
                    window.within("editor-tabs").find(0_usize).bounds().left(),
                    window.find("editor-heading").bounds().left(),
                    "Editor tabs must align with the heading"
                );
                window.within("workbench-tabs").click(1_usize, cx);
                assert_eq!(
                    window.within("run-tabs").find(0_usize).bounds().left(),
                    window.find("run-scope").bounds().left(),
                    "Run tabs must align with the command controls"
                );
                window.within("workbench-tabs").click(2_usize, cx);
                for mode in 0_usize..4 {
                    window.within("image-modes").click(mode, cx);
                    let first = window.within("image-modes").find(0_usize).bounds();
                    let last = window.within("image-modes").find(3_usize).bounds();
                    let comparison = window.find("comparison-view").bounds();
                    assert_eq!(
                        first.left(),
                        window.find("capture-heading").bounds().left(),
                        "Comparison tabs must align with the capture heading"
                    );
                    assert!(first.left() > comparison.left());
                    assert!(last.right() <= comparison.right());
                }
            })
            .unwrap();
        }
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
        window.within("editor-tabs").click(0_usize, cx);
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
async fn corrected_invalid_source_saves_and_refreshes_suite_status(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let path = directory.path().join("visual_tests/ui-test.tests.yaml");
    let corrected = std::fs::read_to_string(&path).unwrap();
    let invalid = "scenarios: [";
    std::fs::write(&path, invalid).unwrap();

    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let entry = this.session.open.as_ref().unwrap().entry.clone();
            this.session.load(entry).unwrap();
            this.tab = 1;
            this.sync_editor(window, cx);
        });
        window.render_frame(cx);
        assert!(window.find("diagnostic").visible());
        assert!(view.read(cx).session.suites[0].error.is_some());
        let focus = view.read(cx).source.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input(&corrected, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, cx| {
        window.try_find("diagnostic").is_none() && view.read(cx).session.dirty()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).source_issues.is_empty());
        assert!(view.read(cx).session.suites[0].error.is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
        window.click("save", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Saved"))
    })
    .await;
    cx.update(|cx| {
        assert!(!view.read(cx).session.dirty());
        assert!(view.read(cx).session.suites[0].error.is_none());
        assert_eq!(view.read(cx).session.suites[0].scenarios.len(), 1);
    });
    assert_eq!(std::fs::read_to_string(path).unwrap(), corrected);
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
async fn suite_panel_activity_remap_finishes_persistence_and_unlocks_catalog(
    cx: &mut TestAppContext,
) {
    use crate::{
        desktop::runner::{RunCommand, RunOptions, RunOutcome, RunRequest},
        progress::{Event, Record},
    };
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let history = directory.path().join("saved-activity.json");
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let old = this.session.suites[0].clone();
            let mut new = old.clone();
            new.name = "renamed".into();
            new.output_id = "renamed".into();
            new.entry.path = "visual_tests/renamed.tests.yaml".into();
            let root = this.session.workspace.as_ref().unwrap().root.clone();
            this.runs.update(cx, |panel, cx| {
                panel.manager.set_storage_path(history.clone());
                panel.manager.request = Some(RunRequest {
                    root,
                    command: RunCommand::Compare,
                    options: RunOptions {
                        suites: vec!["ui-test".into()],
                        ..Default::default()
                    },
                });
                panel.manager.progress.apply(Record {
                    version: 1,
                    elapsed_ms: 1,
                    event: Event::Compared {
                        suite_id: "ui-test".into(),
                        capture: "home__FINAL".into(),
                        label: "Home".into(),
                        scenario_id: Some("home".into()),
                        passed: true,
                        error: None,
                    },
                });
                panel.manager.progress.end(false, None);
                panel.manager.outcome = Some(RunOutcome::Completed);
                panel.suite_changed(&old, Some(&new), cx);
            });
            this.sync_editor(window, cx);
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
        !view.read(cx).runs.read(cx).running()
    })
    .await;
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(history).unwrap()).unwrap();
    assert!(saved["progress"]["suites"].get("renamed").is_some());
    cx.update(|cx| {
        assert!(
            view.read(cx)
                .runs
                .read(cx)
                .manager
                .persistence_error
                .is_none()
        )
    });
}

#[gpui_kit::test]
async fn suite_panel_context_rename_fields_and_delete_keep_artifacts_aligned(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor_with_reference(cx, directory.path()).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("ui-test"))
            .unwrap();
        window.right_click(("suite-row", index), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("popup-menu").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.within("popup-menu").find(0_usize).label(),
            Some("Rename suite")
        );
        assert_eq!(
            window.within("popup-menu").find(1_usize).label(),
            Some("Delete suite")
        );
        window.within("popup-menu").click(0_usize, cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("file-name", cx);
        window.press("ctrl-a", cx);
        window.input("checkout.tests.yaml", cx);
        window.click("suite-display-name", cx);
        window.input("Customer checkout", cx);
        window.click("file-confirm", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
        !view.read(cx).busy
    })
    .await;
    cx.update(|cx| assert_eq!(view.read(cx).status, "File saved"));
    assert!(
        !directory
            .path()
            .join("visual_tests/ui-test.tests.yaml")
            .exists()
    );
    assert!(
        directory
            .path()
            .join(".regressify/checkout/bitmaps_reference/home.png")
            .exists()
    );
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("checkout"))
            .unwrap();
        assert_eq!(
            window.find(("file", index)).label(),
            Some("Customer checkout")
        );
        assert_eq!(
            window.find("field-displayName").value(),
            Some("Customer checkout")
        );
        assert_eq!(
            window.find("field-displayName").label(),
            Some("Display name")
        );
        window.click("field-displayName", cx);
        window.press("ctrl-a", cx);
        window.input("Checkout review", cx);
        window.press("f2", cx);
        assert!(
            window.try_find("file-confirm").is_none(),
            "F2 must not rename from a field input"
        );
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        view.read(cx).unsaved()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| window.click("save", cx))
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
        !view.read(cx).busy
    })
    .await;
    cx.update(|cx| assert!(!view.read(cx).unsaved(), "{}", view.read(cx).status));
    let path = directory.path().join("visual_tests/checkout.tests.yaml");
    let suite: crate::types::TestSuite =
        serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(suite.display_name.as_deref(), Some("Checkout review"));
    cx.update_window(handle.into(), |_, window, cx| {
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("checkout"))
            .unwrap();
        window.click(("file", index), cx);
        window.press("delete", cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("remove-cancel", cx)
    })
    .unwrap();
    assert!(path.exists());
    assert!(directory.path().join(".regressify/checkout").exists());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| window.focus(&this.suite_list.focus, cx));
        window.press("delete", cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("remove-confirm", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
        !view.read(cx).busy && view.read(cx).status == "Suite deleted"
    })
    .await;
    assert!(!path.exists());
    assert!(!directory.path().join(".regressify/checkout").exists());
    cx.update(|cx| assert!(view.read(cx).session.suites.is_empty()));
}

#[gpui_kit::test]
async fn suite_panel_f2_targets_focused_results_row_and_ignores_search(cx: &mut TestAppContext) {
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
            this.main_tab = 2;
        });
        window.render_frame(cx);
        window.click("navigator-search", cx);
        window.input("second", cx);
        window.press("f2", cx);
        window.press("delete", cx);
        assert!(window.try_find("file-confirm").is_none());
        assert!(window.try_find("remove-confirm").is_none());
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("second"))
            .unwrap();
        window.render_frame(cx);
        window.click(("file", index), cx);
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
        window.press("f2", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("file-name").is_some()
    })
    .await;
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("file-name").value(), Some("second.tests.yaml"));
        window.click("suite-display-name", cx);
        window.input("Second suite label", cx);
        window.click("file-confirm", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |_, cx| {
        !view.read(cx).busy && view.read(cx).status == "File saved"
    })
    .await;
    assert!(
        directory
            .path()
            .join("visual_tests/second.tests.yaml")
            .exists()
    );
    assert!(
        directory
            .path()
            .join("visual_tests/ui-test.tests.yaml")
            .exists()
    );
    let suite: crate::types::TestSuite = serde_yaml::from_str(
        &std::fs::read_to_string(directory.path().join("visual_tests/second.tests.yaml")).unwrap(),
    )
    .unwrap();
    assert_eq!(suite.display_name.as_deref(), Some("Second suite label"));
}

#[gpui_kit::test]
async fn suite_panel_context_delete_preserves_unsaved_draft_on_cancel(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        let index = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("ui-test"))
            .unwrap();
        window.right_click(("suite-row", index), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("popup-menu").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.within("popup-menu").click(1_usize, cx)
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("guard-cancel").visible());
        window.click("guard-cancel", cx);
        assert!(view.read(cx).unsaved());
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    })
    .unwrap();
    assert!(
        directory
            .path()
            .join("visual_tests/ui-test.tests.yaml")
            .exists()
    );
}

#[gpui_kit::test]
async fn new_suite_dialog_creates_a_real_file(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let mut expected_paths = cx.update(|cx| {
        view.read(cx)
            .session
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<std::collections::BTreeSet<_>>()
    });
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
    let new_path = Path::new("visual_tests/checkout.tests.json");
    assert!(expected_paths.insert(new_path.to_path_buf()));
    cx.update(|cx| {
        let session = &view.read(cx).session;
        assert_eq!(session.entries.len(), expected_paths.len());
        assert_eq!(
            session
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            expected_paths
        );
        let open = session.open.as_ref().unwrap();
        assert_eq!(open.entry.path, new_path);
        assert_eq!(
            open.entry.kind,
            crate::desktop::document::DocumentKind::Suite
        );
    });
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
fn deployment_preserves_inherited_and_explicit_report_options(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.main_tab = 1;
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
        });
        window.render_frame(cx);
        window.within("run-tabs").click(2_usize, cx);
        for key in ["title", "heading", "subheading"] {
            assert_eq!(window.find(format!("run-field-{key}")).value(), Some(""));
        }
        let inherited = view
            .update(cx, |this, cx| {
                this.runs
                    .update(cx, |panel, cx| panel.deployment_intent(cx))
            })
            .unwrap();
        assert!(inherited.request.options.title.is_none());
        assert!(inherited.request.options.heading.is_none());
        assert!(inherited.request.options.subheading.is_none());

        window.click("run-field-heading", cx);
        window.input("Custom summary", cx);
        let overridden = view
            .update(cx, |this, cx| {
                this.runs
                    .update(cx, |panel, cx| panel.deployment_intent(cx))
            })
            .unwrap();
        assert!(overridden.request.options.title.is_none());
        assert_eq!(
            overridden.request.options.heading.as_deref(),
            Some("Custom summary")
        );
        assert!(overridden.request.options.subheading.is_none());

        window.press("ctrl-a", cx);
        window.press("backspace", cx);
        let empty = view
            .update(cx, |this, cx| {
                this.runs
                    .update(cx, |panel, cx| panel.deployment_intent(cx))
            })
            .unwrap();
        assert_eq!(empty.request.options.heading.as_deref(), Some(""));
        assert!(!view.read(cx).runs.read(cx).running());
    })
    .unwrap();
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
async fn scenario_information_hover_shows_metadata_and_copies_recorded_urls(
    cx: &mut TestAppContext,
) {
    let long_url = format!("https://example.test/{}", "long-capture-path/".repeat(24));
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let run = directory
            .path()
            .join(".regressify/ui-test/bitmaps_test/run-1");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(run.join("report.json"), serde_json::to_vec(&serde_json::json!({
            "id": "ui-test", "tests": [
                {"status": "fail", "pair": {"scenarioLabel": "Carousel", "selector": "body .carousel", "referenceUrl": "https://production.example.test/start", "url": "https://staging.example.test/start", "diff": {"misMatchPercentage": "4.20", "dimensionDifference": {"width": -20, "height": 40}}}},
                {"status": "pass", "pair": {"scenarioLabel": "Whole document", "selector": "document", "referenceUrl": "https://example.test/", "url": "https://example.test/", "diff": {"rawMisMatchPercentage": 0.0, "dimensionDifference": {"width": 0, "height": 0}}}},
                {"status": "fail", "pair": {"label": "Legacy capture"}},
                {"status": "pass", "pair": {"scenarioLabel": "Long URL capture", "selector": "document", "referenceUrl": long_url, "url": long_url}}
            ]
        })).unwrap()).unwrap();
        let (handle, _) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("workbench-tabs").click(2_usize, cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window.try_find("capture-information").is_some()
        })
        .await;
        for index in 0_usize..4 {
            cx.update_window(handle.into(), |_, window, cx| {
                window.click(("capture", index), cx);
                window.hover("capture-information", cx);
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
                window.try_find("capture-info-label").is_some_and(|label| {
                    label.label()
                        == Some(match index {
                            0 => "Label: Carousel",
                            1 => "Label: Whole document",
                            3 => "Label: Long URL capture",
                            _ => "Label: Legacy capture",
                        })
                })
            })
            .await;
            cx.update_window(handle.into(), |_, window, cx| {
                let bounds = window.find("capture-information-content").bounds();
                assert!(bounds.left() >= px(0.) && bounds.right() <= px(width));
                assert!(bounds.top() >= px(0.) && bounds.bottom() <= px(height));
                let label = window.find("capture-info-label").bounds();
                let selector = window.find("capture-info-selector").bounds();
                assert_eq!(label.top(), selector.top());
                assert!(label.right() < selector.left() && selector.right() <= bounds.right());
                let diff_stats = [
                    "capture-info-diff",
                    "capture-info-diff-x",
                    "capture-info-diff-y",
                ]
                .map(|id| window.find(id).bounds());
                for pair in diff_stats.windows(2) {
                    assert_eq!(pair[0].top(), pair[1].top());
                    assert!(pair[0].right() < pair[1].left());
                }
                assert!(diff_stats[2].right() <= bounds.right());
                assert!(bounds.size.height <= px(120.));
                assert_eq!(
                    window.find("capture-info-selector").label(),
                    Some(match index {
                        0 => "Selector: body .carousel",
                        1 | 3 => "Selector: document",
                        _ => "Selector: Unavailable",
                    })
                );
                assert_eq!(
                    window.find("capture-info-diff").label(),
                    Some(match index {
                        0 => "Diff %: 4.20%",
                        1 => "Diff %: 0.000%",
                        _ => "Diff %: Unavailable",
                    })
                );
                assert_eq!(
                    window.find("capture-info-diff-x").label(),
                    Some(match index {
                        0 => "Diff X: -20",
                        1 => "Diff X: 0",
                        _ => "Diff X: Unavailable",
                    })
                );
                assert_eq!(
                    window.find("capture-info-diff-y").label(),
                    Some(match index {
                        0 => "Diff Y: 40",
                        1 => "Diff Y: 0",
                        _ => "Diff Y: Unavailable",
                    })
                );
                if index == 1 || index == 3 {
                    assert_eq!(
                        window.find(("capture-info-url-value", 1_usize)).label(),
                        Some("Same URL")
                    );
                }
                for image in 0_usize..2 {
                    let label = window.find(("capture-info-url-label", image)).bounds();
                    let url = window.find(("capture-info-url-value", image)).bounds();
                    let copy = window.find(("capture-info-copy-url", image)).bounds();
                    assert!((label.center().y - url.center().y).abs() <= px(1.));
                    assert!((copy.center().y - url.center().y).abs() <= px(1.));
                    assert!(url.left() >= label.right() && url.left() - label.right() <= px(8.));
                    assert!(copy.left() >= url.right() && copy.left() - url.right() <= px(8.));
                    assert!(copy.right() <= bounds.right());
                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("unchanged".into()));
                    window.click(("capture-info-copy-url", image), cx);
                    assert_eq!(
                        cx.read_from_clipboard().unwrap().text().unwrap(),
                        match (index, image) {
                            (0, 0) => "https://production.example.test/start",
                            (0, _) => "https://staging.example.test/start",
                            (1, _) => "https://example.test/",
                            (3, _) => long_url.as_str(),
                            _ => "unchanged",
                        }
                    );
                    assert!(window.find("capture-information-content").visible());
                }
                window.hover("capture-search", cx);
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
                window.try_find("capture-information-content").is_none()
            })
            .await;
        }
    }
}

#[gpui_kit::test]
async fn url_only_scenario_names_match_editor_lists_and_headings(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let path = directory.path().join("visual_tests/ui-test.tests.yaml");
    let source = "scenarios:\n- url: https://example.test/men/running-shoes\n- url: https://example.test/women/running-shoes\n- url: https://example.test/help-center\n";
    std::fs::write(&path, source).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let entry = this.session.open.as_ref().unwrap().entry.clone();
            this.session.load(entry).unwrap();
            this.sync_editor(window, cx);
            let draft = this.draft_suite_summary().unwrap();
            assert_eq!(
                draft
                    .scenarios
                    .iter()
                    .map(|scenario| scenario.label.clone())
                    .collect::<Vec<_>>(),
                this.scenario_titles()
            );
            assert!(this.value["scenarios"][0].get("label").is_none());
            assert!(this.value["scenarios"][0].get("id").is_none());
        });
        window.render_frame(cx);
        for (index, expected) in [
            "Men / Running shoes",
            "Women / Running shoes",
            "Help center",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(window.find(("scenario", index)).label(), Some(expected));
            window.click(("scenario", index), cx);
            window.render_frame(cx);
            assert_eq!(view.read(cx).selection_title(), expected);
            assert_eq!(window.find("editor-heading").label(), Some(expected));
        }
    })
    .unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
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
        let report = window.find("report-title");
        let label = report.label().unwrap_or_default();
        assert!(label.starts_with("ui-test / "), "{label}");
        assert!(!label.contains("desktop-"));
        assert_eq!(
            window.find(("capture", 0_usize)).label(),
            Some("Passed Payment confirmation")
        );
        assert!(window.try_find("capture-context").is_none());
        assert!(window.try_find("capture-threshold").is_none());
        window.click("copy-capture-details", cx);
        let summary = cx.read_from_clipboard().unwrap().text().unwrap();
        assert!(summary.contains("Scenario: Payment confirmation"));
        assert!(summary.contains("Capture: Phone / Full page"));
        assert!(summary.contains("Capture ID: 8ac900fea19642cc0000_phone__FINAL"));
        assert!(summary.contains("Page: https://example.com/receipt"));
        assert!(summary.contains("Allowed mismatch: 0.1%"));
        assert!(summary.contains("Mismatch: 0.050%"));
        window.click(("capture", 1_usize), cx);
        assert_eq!(
            window.find("capture-heading").label(),
            Some("Payment at capture time")
        );
        window.click("capture-search", cx);
        window.input("8ac900", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find(("capture", 0_usize)).is_some()
            && window.try_find(("capture", 1_usize)).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("capture-search", cx);
        window.press("ctrl-a", cx);
        window.input("at capture time", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find(("capture", 0_usize)).is_none()
            && window.try_find(("capture", 1_usize)).is_some()
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
async fn results_suite_navigation_is_isolated_and_preserves_drafts_during_refresh(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor_at_size(cx, directory.path(), size(px(1080.), px(720.)));
    let (open_path, draft) = cx
        .update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                for name in ["second-suite", "empty-suite"] {
                    this.session
                        .workspace
                        .as_ref()
                        .unwrap()
                        .create_suite(name)
                        .unwrap();
                }
                this.session.refresh().unwrap();
                this.sync_suites(cx);
                let open = this.session.open.as_mut().unwrap();
                let path = open.entry.path.clone();
                let draft = format!("{}\n# Unsaved editor draft\n", open.document.source());
                open.document.set_source(draft.clone()).unwrap();
                this.sync_editor(window, cx);
                (path, draft)
            })
        })
        .unwrap();
    for (suite, label) in [
        ("ui-test", "Active capture"),
        ("second-suite", "Second capture"),
        ("unmatched-suite", "Unrelated capture"),
    ] {
        let run = directory
            .path()
            .join(".regressify")
            .join(suite)
            .join("bitmaps_test/run-1");
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(
            run.join("report.json"),
            serde_json::to_vec(&serde_json::json!({
                "id": suite, "tests": [{"status": "pass", "pair": {"label": label}}]
            }))
            .unwrap(),
        )
        .unwrap();
    }
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("workbench-tabs").click(2_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("capture-heading")
            .is_some_and(|heading| heading.label() == Some("Active capture"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let second = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("second-suite"))
            .unwrap();
        window.click(("file", second), cx);
        assert_eq!(view.read(cx).main_tab, 2);
        assert_eq!(
            window.find("capture-heading").label(),
            Some("Second capture")
        );
        assert_eq!(
            window.find(("capture", 0_usize)).label(),
            Some("Passed Second capture")
        );
        assert!(window.try_find(("capture", 1_usize)).is_none());
        assert!(window.try_find("report-picker").is_none());
        assert!(window.try_find("scenario-pane").is_none());
        let sidebar = window.find("workspace-navigation").bounds();
        let captures = window.find("capture-list").bounds();
        assert!(captures.left() >= sidebar.left() && captures.right() <= sidebar.right());
        assert!(captures.top() >= window.find("suite-pane").bounds().bottom());
        assert!(window.find("results-panel").bounds().left() >= sidebar.right());
        let open = view.read(cx).session.open.as_ref().unwrap();
        assert_eq!(open.entry.path, open_path);
        assert_eq!(open.document.source(), draft);
        assert!(view.read(cx).session.dirty());
        window.click("refresh-results", cx);
        let empty = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.suite_name().as_deref() == Some("empty-suite"))
            .unwrap();
        window.click(("file", empty), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("capture-empty")
            .is_some_and(|empty| empty.label() == Some("No captures for this suite"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("capture-heading").label(), Some("Comparison"));
        assert!(window.try_find(("capture", 0_usize)).is_none());
        assert!(window.try_find(("image-details", 1_usize)).is_none());
        window.click("approve-scenario", cx);
        assert_eq!(view.read(cx).main_tab, 2);
        let original = view
            .read(cx)
            .session
            .entries
            .iter()
            .position(|entry| entry.path == open_path)
            .unwrap();
        window.click(("file", original), cx);
        assert_eq!(
            window.find("capture-heading").label(),
            Some("Active capture")
        );
        let open = view.read(cx).session.open.as_ref().unwrap();
        assert_eq!(open.entry.path, open_path);
        assert_eq!(open.document.source(), draft);
        assert!(view.read(cx).session.dirty());
        window.within("workbench-tabs").click(0_usize, cx);
        assert!(window.try_find("capture-list").is_none());
        assert!(window.try_find("scenario-pane").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn side_by_side_scrolls_both_axes_from_either_pane_with_different_image_sizes(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let suite = directory.path().join(".regressify/ui-test");
    let run = suite.join("bitmaps_test/run-1");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::create_dir_all(suite.join("html_report")).unwrap();
    std::fs::create_dir_all(suite.join("bitmaps_reference")).unwrap();
    for (path, width, height) in [
        (suite.join("bitmaps_reference/home.png"), 1600, 2200),
        (run.join("home.png"), 1280, 1800),
    ] {
        lodepng::encode32_file(
            path,
            &[30_u8, 150, 180, 255].repeat(width * height),
            width,
            height,
        )
        .unwrap();
    }
    let pair = serde_json::json!({"label": "Home", "reference": "../bitmaps_reference/home.png", "test": "../bitmaps_test/run-1/home.png"});
    let report = serde_json::json!({"id": "ui-test", "tests": [
        {"status": "fail", "pair": pair}, {"status": "fail", "pair": pair}
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
        window.within("image-modes").click(3_usize, cx);
        assert!(window.try_find(("image-scroll", 2_usize)).is_none());
        for index in [0_usize, 1] {
            let viewport = window.find(("image-scroll", index)).bounds();
            let content = window.find(("image-content", index)).bounds();
            assert!(content.right() <= viewport.right());
            assert!(
                content.bottom() <= viewport.bottom(),
                "Fit area must display the entire image in pane {index}"
            );
        }
        window.click("actual-size", cx);
        let offset = |window: &gpui_kit::Window, index| {
            window.find(("image-content", index)).bounds().origin
                - window.find(("image-scroll", index)).bounds().origin
        };
        for index in [0_usize, 1] {
            assert_eq!(
                window.find(("image-content", index)).bounds().size,
                size(px(1600.), px(2200.))
            );
        }
        window.scroll(
            ("image-scroll", 0_usize),
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(-120.), px(-260.))),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(offset(window, 0_usize), offset(window, 1_usize));
        assert!(offset(window, 0_usize).x < px(-100.) && offset(window, 0_usize).y < px(-200.));
        window.scroll(
            ("image-scroll", 1_usize),
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(-90.), px(-170.))),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(offset(window, 0_usize), offset(window, 1_usize));
        assert!(offset(window, 1_usize).x < px(-200.) && offset(window, 1_usize).y < px(-400.));
        for index in [0_usize, 1] {
            window.scroll(
                ("image-scroll", index),
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(20_000.), px(20_000.))),
                cx,
            );
            let viewport = window.find(("image-scroll", index)).bounds();
            let vertical = gpui_kit::point(viewport.right() - px(8.), viewport.top() + px(12.));
            window.drag(vertical, vertical + gpui_kit::point(px(0.), px(70.)), cx);
            window.render_frame(cx);
            assert_eq!(offset(window, 0_usize), offset(window, 1_usize));
            assert!(
                offset(window, index).y < px(-10.),
                "Vertical thumb in pane {index} must scroll"
            );
            let horizontal = gpui_kit::point(viewport.left() + px(12.), viewport.bottom() - px(8.));
            window.drag(
                horizontal,
                horizontal + gpui_kit::point(px(60.), px(0.)),
                cx,
            );
            window.render_frame(cx);
            assert_eq!(offset(window, 0_usize), offset(window, 1_usize));
            assert!(
                offset(window, index).x < px(-10.),
                "Horizontal thumb in pane {index} must scroll"
            );
        }
        window.click("zoom-in", cx);
        window.render_frame(cx);
        assert_eq!(offset(window, 0_usize), offset(window, 1_usize));
        window.click("fit-image", cx);
        window.render_frame(cx);
        assert_eq!(offset(window, 0_usize), offset(window, 1_usize));
        assert_eq!(
            window.find(("image-content", 0_usize)).bounds().size,
            window.find(("image-content", 1_usize)).bounds().size
        );
        window.click(("capture", 1_usize), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find(("image-details", 1_usize)).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        for index in [0_usize, 1] {
            let content = window.find(("image-content", index)).bounds();
            let viewport = window.find(("image-scroll", index)).bounds();
            assert!((content.center().x - viewport.center().x).abs() <= px(1.));
            assert_eq!(content.top(), viewport.top());
        }
    })
    .unwrap();
}

#[gpui_kit::test]
async fn image_context_menus_offer_external_targets_without_switching_tabs(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let run = directory
            .path()
            .join(".regressify/ui-test/bitmaps_test/run-1");
        std::fs::create_dir_all(&run).unwrap();
        lodepng::encode32_file(
            run.join("capture.png"),
            &[30_u8, 150, 180, 255].repeat(160 * 100),
            160,
            100,
        )
        .unwrap();
        let image = "../bitmaps_test/run-1/capture.png";
        std::fs::write(run.join("report.json"), serde_json::to_vec(&serde_json::json!({
            "id": "ui-test", "tests": [
                {"status": "fail", "pair": {
                    "label": "With replacement profile", "reference": image, "test": image, "diffImage": image,
                    "referenceUrl": "https://production.example.test/start", "url": "https://staging.example.test/start"
                }},
                {"status": "pass", "pair": {"label": "Legacy capture", "reference": image, "test": image}}
            ]
        })).unwrap()).unwrap();
        let (handle, _) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("workbench-tabs").click(2_usize, cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        for mode in 0_usize..4 {
            cx.update_window(handle.into(), |_, window, cx| {
                window.within("image-modes").click(mode, cx);
            })
            .unwrap();
            let panes = if mode == 3 {
                vec![0_usize, 1, 2]
            } else {
                vec![mode]
            };
            for index in &panes {
                cx.update_window(handle.into(), |_, window, cx| {
                    window.right_click(("image-content", *index), cx);
                })
                .unwrap();
                cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
                    window.try_find("popup-menu").is_some()
                })
                .await;
                cx.update_window(handle.into(), |_, window, cx| {
                    let menu = window.find("popup-menu").bounds();
                    assert!(menu.left() >= px(0.) && menu.right() <= px(width));
                    assert!(menu.top() >= px(0.) && menu.bottom() <= px(height));
                    for (item, label) in [
                        "Open in default browser",
                        "Open in default image viewer",
                        "Show in file manager",
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        assert_eq!(window.within("popup-menu").find(item).label(), Some(label));
                    }
                    if *index < 2 {
                        let kind = if *index == 0 { "Reference" } else { "Test" };
                        assert_eq!(
                            window.within("popup-menu").find(4_usize).label(),
                            Some(format!("Open {kind} URL").as_str())
                        );
                        assert_eq!(
                            window.within("popup-menu").find(5_usize).label(),
                            Some(format!("Copy {kind} URL").as_str())
                        );
                    } else {
                        assert!(window.within("popup-menu").try_find(4_usize).is_none());
                        assert!(window.within("popup-menu").try_find(5_usize).is_none());
                    }
                    for pane in &panes {
                        assert!(window.find(("image-content", *pane)).visible());
                    }
                    if *index < 2 {
                        window.within("popup-menu").click(5_usize, cx);
                        assert_eq!(
                            cx.read_from_clipboard().unwrap().text().unwrap(),
                            if *index == 0 {
                                "https://production.example.test/start"
                            } else {
                                "https://staging.example.test/start"
                            }
                        );
                    } else {
                        window.press("escape", cx);
                    }
                })
                .unwrap();
                cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
                    window.try_find("popup-menu").is_none()
                })
                .await;
            }
        }
        cx.update_window(handle.into(), |_, window, cx| {
            window.click(("capture", 1_usize), cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        for index in 0_usize..2 {
            cx.update_window(handle.into(), |_, window, cx| {
                window.within("image-modes").click(index, cx);
                window.right_click(("image-content", index), cx);
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
                window.try_find("popup-menu").is_some()
            })
            .await;
            cx.update_window(handle.into(), |_, window, cx| {
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("unchanged".into()));
                window.within("popup-menu").click(4_usize, cx);
                window.within("popup-menu").click(5_usize, cx);
                assert_eq!(
                    cx.read_from_clipboard().unwrap().text().unwrap(),
                    "unchanged"
                );
                assert!(window.try_find("popup-menu").is_some());
                window.press("escape", cx);
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
                window.try_find("popup-menu").is_none()
            })
            .await;
        }
    }
}

fn assert_fitted_zoom(window: &gpui_kit::Window, index: usize, canvas_width: f32) {
    let width = window.find(("image-content", index)).bounds().size.width;
    assert_eq!(
        window.find("zoom-value").label(),
        Some(format!("{:.1}%", width / px(canvas_width) * 100.).as_str())
    );
}

#[gpui_kit::test]
async fn image_previews_fit_area_without_upscaling_and_center_horizontally(
    cx: &mut TestAppContext,
) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let run = directory
            .path()
            .join(".regressify/ui-test/bitmaps_test/run-1");
        std::fs::create_dir_all(&run).unwrap();
        let cases = [
            ((96, 64), (64, 96)),
            ((1600, 200), (1200, 300)),
            ((300, 1600), (200, 1200)),
        ];
        let mut captures = Vec::new();
        for (index, &(reference, current)) in cases.iter().enumerate() {
            for (label, (width, height)) in [("reference", reference), ("current", current)] {
                lodepng::encode32_file(
                    run.join(format!("{index}-{label}.png")),
                    &[30_u8, 150, 180, 255].repeat(width * height),
                    width,
                    height,
                )
                .unwrap();
            }
            captures.push(serde_json::json!({"status": "pass", "pair": {
                "label": format!("Capture {index}"),
                "reference": format!("../bitmaps_test/run-1/{index}-reference.png"),
                "test": format!("../bitmaps_test/run-1/{index}-current.png"),
                "diffImage": format!("../bitmaps_test/run-1/{index}-reference.png")
            }}));
        }
        std::fs::write(
            run.join("report.json"),
            serde_json::to_vec(&serde_json::json!({"id": "ui-test", "tests": captures})).unwrap(),
        )
        .unwrap();
        let (handle, view) = editor_at_size(cx, directory.path(), size(px(width), px(height)));
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
            window.render_frame(cx);
            let panel = window.find("results-panel").bounds();
            let image = window.find(("image-scroll", 1_usize)).bounds();
            assert!(image.top() - panel.top() <= px(110.));
            assert!(image.size.height >= panel.size.height * 0.8);
            let toolbar = window.find("image-toolbar").bounds();
            let tabs = window.within("image-modes").find(3_usize).bounds();
            let fit = window.find("fit-image").bounds();
            let actual_size = window.find("actual-size").bounds();
            assert!(fit.left() >= tabs.right());
            assert!((fit.center().y - tabs.center().y).abs() <= px(2.));
            assert!((actual_size.center().y - fit.center().y).abs() <= px(2.));
            assert!(actual_size.right() <= toolbar.right());
            assert!(toolbar.bottom() <= image.top());
            assert!(window.try_find("capture-position").is_none());
            assert!(window.try_find("next-capture").is_none());
            assert!(window.try_find("previous-captures").is_none());
        })
        .unwrap();
        for (capture, (reference, current)) in cases.into_iter().enumerate() {
            cx.update_window(handle.into(), |_, window, cx| {
                window.click(("capture", capture), cx);
            })
            .unwrap();
            cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
                window.try_find(("image-details", 1_usize)).is_some()
            })
            .await;
            cx.update_window(handle.into(), |_, window, cx| {
                for mode in 0..4 {
                    window.within("image-modes").click(mode, cx);
                    assert_eq!(window.find("fit-image").checked(), Some(true));
                    let (canvas_width, canvas_height) = match mode {
                        0 | 2 => reference,
                        1 => current,
                        _ => (reference.0.max(current.0), reference.1.max(current.1)),
                    };
                    let canvas_width = canvas_width as f32;
                    let canvas_height = canvas_height as f32;
                    let panes = if mode == 3 {
                        vec![0_usize, 1, 2]
                    } else {
                        vec![mode]
                    };
                    let scale = panes.iter().fold(1.0_f32, |scale, index| {
                        let viewport = window.find(("image-scroll", *index)).bounds();
                        scale
                            .min(viewport.size.width / px(canvas_width))
                            .min(viewport.size.height / px(canvas_height))
                    });
                    assert_fitted_zoom(window, panes[0], canvas_width);
                    for index in &panes {
                        let content = window.find(("image-content", *index)).bounds();
                        let viewport = window.find(("image-scroll", *index)).bounds();
                        assert_eq!(window.find(("image-details", *index)).bounds(), viewport);
                        assert!(window.try_find(("open-image", *index)).is_none());
                        assert!(content.left() >= viewport.left() - px(1.));
                        assert!(content.top() >= viewport.top());
                        assert!(content.right() <= viewport.right() + px(1.));
                        assert!(content.bottom() <= viewport.bottom() + px(1.));
                        assert!((content.center().x - viewport.center().x).abs() <= px(1.));
                        assert!(content.size.width <= px(canvas_width));
                        assert!(content.size.height <= px(canvas_height));
                        assert!(
                            (content.size.width / px(canvas_width) - scale).abs()
                                <= 1.0 / canvas_width
                        );
                        assert!(
                            (content.size.height / px(canvas_height) - scale).abs()
                                <= 1.0 / canvas_height
                        );
                    }
                    if capture == 0 {
                        assert_eq!(scale, 1.0);
                        window.click("zoom-in", cx);
                        assert_eq!(window.find("fit-image").checked(), Some(false));
                        assert_eq!(window.find("zoom-value").label(), Some("110.0%"));
                        for index in &panes {
                            let content = window.find(("image-content", *index)).bounds();
                            let viewport = window.find(("image-scroll", *index)).bounds();
                            assert!((content.size.width - px(canvas_width * 1.1)).abs() <= px(1.));
                            assert!(
                                (content.size.height - px(canvas_height * 1.1)).abs() <= px(1.)
                            );
                            assert!((content.center().x - viewport.center().x).abs() <= px(1.));
                        }
                        window.click("actual-size", cx);
                        window.click("fit-image", cx);
                        for index in &panes {
                            assert_eq!(
                                window.find(("image-content", *index)).bounds().size,
                                size(px(canvas_width), px(canvas_height))
                            );
                        }
                    }
                    window.click("fit-width", cx);
                    assert_eq!(window.find("fit-width").checked(), Some(true));
                    assert_eq!(window.find("fit-image").checked(), Some(false));
                    assert_fitted_zoom(window, panes[0], canvas_width);
                    for index in &panes {
                        let content = window.find(("image-content", *index)).bounds();
                        let viewport = window.find(("image-scroll", *index)).bounds();
                        let scale = (viewport.size.width / px(canvas_width)).min(1.);
                        assert!((content.size.width - px(canvas_width * scale)).abs() <= px(1.));
                        assert!((content.size.height - px(canvas_height * scale)).abs() <= px(1.));
                        assert!((content.center().x - viewport.center().x).abs() <= px(1.));
                        if content.size.height > viewport.size.height + px(80.) {
                            window.scroll(
                                ("image-scroll", *index),
                                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-40.))),
                                cx,
                            );
                            window.render_frame(cx);
                            assert!(
                                window.find(("image-content", *index)).bounds().top()
                                    < content.top()
                            );
                        }
                    }
                    window.click("fit-image", cx);
                    assert_fitted_zoom(window, panes[0], canvas_width);
                }
            })
            .unwrap();
        }
        cx.update_window(handle.into(), |_, window, cx| {
            window.click(("capture", 1_usize), cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            for mode in [0_usize, 3] {
                window.within("image-modes").click(mode, cx);
                for fit in ["fit-image", "fit-width"] {
                    window.click(fit, cx);
                    let before = window.find("zoom-value").label().unwrap().to_string();
                    for hidden in [true, false] {
                        view.update(cx, |editor, cx| {
                            editor.preferences.navigator_hidden = hidden;
                            cx.notify();
                        });
                        window.render_frame(cx);
                        assert_fitted_zoom(window, 0, 1600.);
                        if hidden {
                            assert_ne!(window.find("zoom-value").label(), Some(before.as_str()));
                        } else {
                            assert_eq!(window.find("zoom-value").label(), Some(before.as_str()));
                        }
                    }
                }
            }
        })
        .unwrap();
    }
}

#[gpui_kit::test]
async fn image_previews_keep_independent_modes_until_scenario_or_results_exit(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let run = directory
        .path()
        .join(".regressify/ui-test/bitmaps_test/run-1");
    std::fs::create_dir_all(&run).unwrap();
    lodepng::encode32_file(
        run.join("capture.png"),
        &[30_u8, 150, 180, 255].repeat(1200 * 1800),
        1200,
        1800,
    )
    .unwrap();
    let image = "../bitmaps_test/run-1/capture.png";
    std::fs::write(run.join("report.json"), serde_json::to_vec(&serde_json::json!({
        "id": "ui-test", "tests": [
            {"status": "fail", "pair": {"label": "First viewport", "scenarioId": "first", "reference": image, "test": image, "diffImage": image}},
            {"status": "fail", "pair": {"label": "Second viewport", "scenarioId": "first", "reference": image, "test": image, "diffImage": image}},
            {"status": "pass", "pair": {"label": "Other scenario", "scenarioId": "second", "reference": image, "test": image}},
            {"status": "pass", "pair": {"label": "Legacy capture", "reference": image, "test": image}},
            {"status": "pass", "pair": {"label": "Empty scenario ID", "scenarioId": "", "reference": image, "test": image}}
        ]
    })).unwrap()).unwrap();
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
            window.within("image-modes").find(3_usize).selected(),
            Some(true)
        );
        for mode in 0..4 {
            window.within("image-modes").click(mode, cx);
            assert_eq!(window.find("fit-image").checked(), Some(true));
            if mode == 0 {
                window.click("fit-width", cx);
            } else {
                window.click("actual-size", cx);
                if mode == 2 {
                    window.click("zoom-out", cx);
                } else {
                    for _ in 0..mode {
                        window.click("zoom-in", cx);
                    }
                }
            }
        }
        let left = window.find(("image-scroll", 0_usize)).bounds();
        let middle = window.find(("image-scroll", 1_usize)).bounds();
        let right = window.find(("image-scroll", 2_usize)).bounds();
        assert!(left.right() <= middle.left() && middle.right() <= right.left());
        for (index, label) in [(0_usize, None), (1, Some("110.0%")), (2, Some("90.0%"))] {
            window.within("image-modes").click(3_usize, cx);
            assert_eq!(window.find("zoom-value").label(), Some("150.0%"));
            window.click(("image-scroll", index), cx);
            assert_eq!(
                window.within("image-modes").find(index).selected(),
                Some(true)
            );
            if let Some(label) = label {
                assert_eq!(window.find("zoom-value").label(), Some(label));
            } else {
                assert_fitted_zoom(window, index, 1200.);
            }
            assert_eq!(window.find("fit-width").checked(), Some(index == 0));
        }
        window.within("image-modes").click(2_usize, cx);
        window.click("refresh-results", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find(("image-details", 2_usize)).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.within("image-modes").find(2_usize).selected(),
            Some(true)
        );
    })
    .unwrap();
    for capture in [0_usize, 1] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.within("image-modes").click(1_usize, cx);
            window.click(("capture", capture), cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(
                window.within("image-modes").find(1_usize).selected(),
                Some(true)
            );
            for (mode, label) in [
                (0_usize, None),
                (1, Some("110.0%")),
                (2, Some("90.0%")),
                (3, Some("150.0%")),
            ] {
                window.within("image-modes").click(mode, cx);
                assert_eq!(window.find("fit-width").checked(), Some(mode == 0));
                assert_eq!(window.find("fit-image").checked(), Some(false));
                if let Some(label) = label {
                    assert_eq!(window.find("zoom-value").label(), Some(label));
                } else {
                    assert_fitted_zoom(window, mode, 1200.);
                }
            }
        })
        .unwrap();
    }
    for (mode, capture) in [(0_usize, 2_usize), (1, 0), (2, 2), (0, 3), (1, 4), (2, 2)] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.within("image-modes").click(mode, cx);
            window.click(("capture", capture), cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
            window.try_find(("image-details", 1_usize)).is_some()
        })
        .await;
        cx.update_window(handle.into(), |_, window, _| {
            assert_eq!(
                window.within("image-modes").find(3_usize).selected(),
                Some(true)
            );
        })
        .unwrap();
    }
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find(("image-scroll", 2_usize)).is_none());
        window.click(("image-content", 0_usize), cx);
        assert_eq!(
            window.within("image-modes").find(0_usize).selected(),
            Some(true)
        );
        for mode in 0..4 {
            window.within("image-modes").click(mode, cx);
            assert_eq!(window.find("fit-image").checked(), Some(true));
            if mode == 2 {
                assert_eq!(window.find("zoom-value").label(), Some("100.0%"));
            } else {
                assert_fitted_zoom(window, if mode == 3 { 0 } else { mode }, 1200.);
            }
            window.click("actual-size", cx);
        }
        window.within("workbench-tabs").click(0_usize, cx);
        window.within("workbench-tabs").click(2_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find(("image-details", 1_usize)).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        for mode in 0..4 {
            window.within("image-modes").click(mode, cx);
            assert_eq!(window.find("fit-image").checked(), Some(true));
            if mode == 2 {
                assert_eq!(window.find("zoom-value").label(), Some("100.0%"));
            } else {
                assert_fitted_zoom(window, if mode == 3 { 0 } else { mode }, 1200.);
            }
        }
    })
    .unwrap();
}

#[gpui_kit::test]
async fn image_previews_zoom_at_pointer_and_pan_on_drag(cx: &mut TestAppContext) {
    use gpui_kit::InputEvent as _;

    let zoom_levels = [
        0.25_f32,
        1. / 3.,
        0.5,
        2. / 3.,
        0.75,
        0.8,
        0.9,
        1.,
        1.1,
        1.25,
        1.5,
        1.75,
        2.,
        2.5,
        3.,
        4.,
        5.,
    ];
    let directory = tempfile::tempdir().unwrap();
    let run = directory
        .path()
        .join(".regressify/ui-test/bitmaps_test/run-1");
    std::fs::create_dir_all(&run).unwrap();
    lodepng::encode32_file(
        run.join("capture.png"),
        &[30_u8, 150, 180, 255].repeat(1200 * 1800),
        1200,
        1800,
    )
    .unwrap();
    let image = "../bitmaps_test/run-1/capture.png";
    std::fs::write(
        run.join("report.json"),
        serde_json::to_vec(&serde_json::json!({"id": "ui-test", "tests": [{
            "status": "pass", "pair": {
                "label": "Zoom fixture", "reference": image, "test": image, "diffImage": image
            }
        }]}))
        .unwrap(),
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
        for mode in 0..4 {
            window.within("image-modes").click(mode, cx);
            let panes = if mode == 3 { vec![0_usize, 1, 2] } else { vec![mode] };
            for index in panes {
                for fitted in [true, false] {
                    if window.find("fit-image").checked() != Some(true) {
                        window.click("fit-image", cx);
                    }
                    if !fitted {
                        window.click("actual-size", cx);
                    }
                    let viewport = window.find(("image-scroll", index)).bounds();
                    let content = window.find(("image-content", index)).bounds();
                    let pointer = content.origin
                        + gpui_kit::point(
                            content.size.width.min(viewport.size.width) * 0.4,
                            content.size.height.min(viewport.size.height) * 0.4,
                        );
                    for (delta, increase) in [
                        (gpui_kit::ScrollDelta::Lines(gpui_kit::point(0., 1.)), true),
                        (gpui_kit::ScrollDelta::Lines(gpui_kit::point(0., -1.)), false),
                        (gpui_kit::ScrollDelta::Lines(gpui_kit::point(0., 3.)), true),
                        (gpui_kit::ScrollDelta::Lines(gpui_kit::point(0., -3.)), false),
                        (gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(40.))), true),
                        (gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-40.))), false),
                    ] {
                        let before = window.find(("image-content", index)).bounds();
                        let current_zoom = before.size.width / px(1200.);
                        let expected_zoom = if increase {
                            zoom_levels.iter().copied().find(|zoom| *zoom > current_zoom + 0.001).unwrap()
                        } else {
                            zoom_levels.iter().copied().rfind(|zoom| *zoom < current_zoom - 0.001).unwrap_or(0.25)
                        };
                        window.dispatch_event(
                            gpui_kit::ScrollWheelEvent {
                                position: pointer,
                                delta,
                                modifiers: gpui_kit::Modifiers {
                                    control: true,
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                            .to_platform_input(),
                            cx,
                        );
                        window.render_frame(cx);
                        let after = window.find(("image-content", index)).bounds();
                        assert!(
                            (after.size.width - px(1200. * expected_zoom)).abs() <= px(1.),
                            "Ctrl+wheel must select the next preset in mode {mode}, pane {index}, fitted {fitted}"
                        );
                        assert_eq!(
                            window.find("zoom-value").label(),
                            Some(format!("{:.1}%", expected_zoom * 100.).as_str())
                        );
                        let anchored = after.origin
                            + gpui_kit::point(
                                (pointer.x - before.origin.x) * (after.size.width / before.size.width),
                                (pointer.y - before.origin.y) * (after.size.height / before.size.height),
                            );
                        assert!(
                            (anchored.x - pointer.x).abs() <= px(2.)
                                && (anchored.y - pointer.y).abs() <= px(2.),
                            "Zoom must preserve the image point under the pointer in mode {mode}, fitted {fitted}, zoom {expected_zoom}: {before:?} -> {after:?}, pointer {pointer:?}, anchored {anchored:?}"
                        );
                        assert_eq!(window.find("fit-image").checked(), Some(false));
                        if mode == 3 {
                            let other = window.find(("image-content", (index + 1) % 3)).bounds();
                            let other_viewport = window.find(("image-scroll", (index + 1) % 3)).bounds();
                            assert_eq!(after.size, other.size);
                            assert_eq!(after.origin - viewport.origin, other.origin - other_viewport.origin);
                        }
                    }
                    if fitted {
                        window.click("fit-image", cx);
                    }
                    let before = window.find(("image-content", index)).bounds();
                    let movement = gpui_kit::point(px(-60.), px(-80.));
                    window.drag(pointer, pointer + movement, cx);
                    window.render_frame(cx);
                    let mut after = window.find(("image-content", index)).bounds();
                    let expected = if fitted { gpui_kit::point(px(0.), px(0.)) } else { movement };
                    let actual = after.origin - before.origin;
                    assert!(
                        (actual.x - expected.x).abs() <= px(2.)
                            && (actual.y - expected.y).abs() <= px(2.),
                        "Dragging mode {mode}, pane {index}, fitted {fitted}: {actual:?} != {expected:?}"
                    );
                    if !fitted {
                        let start = viewport.origin + gpui_kit::point(px(10.), px(10.));
                        let movement = gpui_kit::point(px(-40.), px(-50.));
                        window.drag(start, start + movement, cx);
                        window.render_frame(cx);
                        let moved = window.find(("image-content", index)).bounds();
                        assert_eq!(moved.origin - after.origin, movement);
                        after = moved;
                        window.scroll(
                            ("image-scroll", index),
                            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-45.))),
                            cx,
                        );
                        window.render_frame(cx);
                        let scrolled = window.find(("image-content", index)).bounds();
                        assert_eq!(scrolled.size, after.size);
                        assert_eq!(scrolled.origin - after.origin, gpui_kit::point(px(0.), px(-45.)));
                        after = scrolled;
                    }
                    if mode == 3 {
                        let other = window.find(("image-content", (index + 1) % 3)).bounds();
                        let other_viewport = window.find(("image-scroll", (index + 1) % 3)).bounds();
                        let difference = (after.origin - viewport.origin) - (other.origin - other_viewport.origin);
                        assert!(difference.x.abs() <= px(1.) && difference.y.abs() <= px(1.));
                    }
                    window.hover("actual-size", cx);
                    window.render_frame(cx);
                    assert_eq!(window.find(("image-content", index)).bounds(), after);
                }
            }
        }
    })
    .unwrap();
}

#[gpui_kit::test]
async fn failed_reference_uses_error_placeholder_in_image_views(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join(".regressify/ui-test");
    let run = output.join("bitmaps_test/run-1");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::create_dir_all(output.join("html_report")).unwrap();
    let placeholder = crate::test::capture::error_icon_png();
    let image = lodepng::decode32(placeholder).unwrap();
    let legacy = lodepng::encode32(&[214_u8, 40, 40, 255], 1, 1).unwrap();
    std::fs::write(run.join("failed.png"), &legacy).unwrap();
    std::fs::write(
        run.join("failed.png.error.json"),
        r#"{"error":true,"message":"Capture failed: missing selector #pay"}"#,
    )
    .unwrap();
    std::fs::write(run.join("failed_diff.png"), placeholder).unwrap();
    let report = serde_json::json!({"id": "ui-test", "tests": [{
        "status": "fail",
        "pair": {
            "label": "Failed reference",
            "reference": "../bitmaps_reference/failed.png",
            "test": "../bitmaps_test/run-1/failed.png",
            "diffImage": "../bitmaps_test/run-1/failed_diff.png",
            "engineErrorMsg": "Capture failed: missing selector #pay"
        }
    }]});
    std::fs::write(
        run.join("report.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    let (handle, _) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("workbench-tabs").click(2_usize, cx);
        window.within("image-modes").click(3_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find(("image-details", 0_usize)).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let dimensions = format!("{} x {}", image.width, image.height);
        for (index, label) in ["Reference", "Test", "Diff"].into_iter().enumerate() {
            assert_eq!(
                window.find(("image-details", index)).label(),
                Some(format!("{label} {dimensions}").as_str())
            );
            let canvas = window.find(("image-content", index)).bounds().size;
            let reference = window.find(("image-content", 0_usize)).bounds().size;
            assert!(
                (canvas.width - reference.width).abs() <= px(1.)
                    && (canvas.height - reference.height).abs() <= px(1.)
            );
        }
        assert_eq!(
            window.find("capture-error").label(),
            Some("Capture failed: missing selector #pay")
        );
        for index in [0_usize, 1] {
            window.within("image-modes").click(index, cx);
            assert!(window.find(("image-content", index)).visible());
            assert!(window.find(("image-details", index)).visible());
            window.right_click(("image-content", index), cx);
            assert!(window.try_find("popup-menu").is_none());
        }
    })
    .unwrap();
    assert!(!output.join("bitmaps_reference/failed.png").exists());
    assert!(std::fs::read(run.join("failed.png")).unwrap() == legacy);
}

#[gpui_kit::test]
async fn results_filter_errors_and_load_image_comparison_modes(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join(".regressify");
    let run = output.join("ui-test/bitmaps_test/run-1");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::create_dir_all(output.join("ui-test/html_report")).unwrap();
    std::fs::create_dir_all(output.join("ui-test/bitmaps_reference")).unwrap();
    for path in [
        run.join("home.png"),
        run.join("home_diff.png"),
        output.join("ui-test/bitmaps_reference/home.png"),
    ] {
        lodepng::encode32_file(path, &[0_u8, 128, 255, 255].repeat(1280 * 720), 1280, 720).unwrap();
    }
    let pair = serde_json::json!({"label": "Home", "reference": "../bitmaps_reference/home.png", "test": "../bitmaps_test/run-1/home.png", "diffImage": "../bitmaps_test/run-1/home_diff.png"});
    let report = serde_json::json!({"id": "ui-test", "tests": [
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
            Some("Test 1280 x 720")
        );
        assert!(
            window.find(("image-content", 1_usize)).bounds().size.width
                <= window.find(("image-scroll", 1_usize)).bounds().size.width
        );
        let fit_scale = window.find(("image-content", 1_usize)).bounds().size.width / px(1280.);
        let next_fit_zoom = [0.25, 1. / 3., 0.5, 2. / 3., 0.75, 0.8, 0.9, 1., 1.1]
            .into_iter()
            .find(|zoom| *zoom > fit_scale + 0.001)
            .unwrap();
        window.click("zoom-in", cx);
        assert_eq!(
            window.find("zoom-value").label(),
            Some(format!("{:.1}%", next_fit_zoom * 100.).as_str())
        );
        window.click("actual-size", cx);
        window.click("zoom-in", cx);
        assert_eq!(window.find("zoom-value").label(), Some("110.0%"));
        window.click("zoom-in", cx);
        assert_eq!(window.find("zoom-value").label(), Some("125.0%"));
        window.click("zoom-out", cx);
        assert_eq!(window.find("zoom-value").label(), Some("110.0%"));
        window.click("zoom-out", cx);
        assert_eq!(window.find("zoom-value").label(), Some("100.0%"));
        window.click("zoom-out", cx);
        assert_eq!(window.find("zoom-value").label(), Some("90.0%"));
        window.click("actual-size", cx);
        assert_eq!(window.find("zoom-value").label(), Some("100.0%"));
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
        assert_eq!(window.find("capture-heading").label(), Some("Home"));
        assert_eq!(
            window.within("image-modes").find(0_usize).bounds().left(),
            window.find("capture-heading").bounds().left()
        );
        assert!(window.find("capture-list").visible());
        let navigator = window.find("workspace-navigation").bounds();
        let captures = window.find("capture-list").bounds();
        assert!(captures.left() >= navigator.left() && captures.right() <= navigator.right());
        assert!(captures.top() >= window.find("suite-pane").bounds().bottom());
        assert!(window.find("results-panel").bounds().left() >= navigator.right());
        assert!(window.try_find("scenario-pane").is_none());
        assert!(window.try_find("expand-comparison").is_none());
        assert!(window.try_find("report-picker").is_none());
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
        assert!(window.try_find("capture-position").is_none());
        assert_eq!(window.find("capture-heading").label(), Some("Checkout"));
        window.click(("capture", 3_usize), cx);
        assert_eq!(window.find("capture-heading").label(), Some("Receipt"));
        window.click(("capture", 1_usize), cx);
        assert_eq!(window.find("capture-heading").label(), Some("Checkout"));
        window.click(("capture", 3_usize), cx);
        assert_eq!(window.find("capture-heading").label(), Some("Receipt"));
        window.within("result-filter").click(0_usize, cx);
        window.click("capture-search", cx);
        window.input("no matching capture", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, _| {
        window.try_find("capture-empty").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("capture-position").is_none());
        assert!(window.try_find("capture-error").is_none());
        assert!(window.try_find(("image-details", 1_usize)).is_none());
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
async fn result_capture_list_scrolls_without_pagination_and_preserves_filtered_selection(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let run = directory
        .path()
        .join(".regressify/ui-test/bitmaps_test/run-1");
    std::fs::create_dir_all(&run).unwrap();
    let captures = (0..152)
        .map(|index| {
            serde_json::json!({
                "status": if index == 0 || index == 151 { "fail" } else { "pass" },
                "pair": { "label": format!("Capture {index}") }
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        run.join("report.json"),
        serde_json::to_vec(&serde_json::json!({"id": "ui-test", "tests": captures})).unwrap(),
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
        assert!(window.try_find("previous-captures").is_none());
        assert!(window.try_find("next-captures").is_none());
        assert!(window.try_find(("capture", 151_usize)).is_none());
        window.scroll(
            "capture-scroll",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-100_000.))),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find(("capture", 151_usize)).visible());
        assert!(window.try_find(("capture", 0_usize)).is_none());
        window.click(("capture", 151_usize), cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 151"));
        window.press("home", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 0"));
        assert!(window.find(("capture", 0_usize)).visible());
        window.press("down", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 1"));
        window.press("up", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 0"));
        window.press("up", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 0"));
        window.press("end", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 151"));
        assert!(window.find(("capture", 151_usize)).visible());
        window.within("result-filter").click(1_usize, cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 151"));
        assert!(window.find(("capture", 0_usize)).visible());
        assert!(window.find(("capture", 151_usize)).visible());
        window.click(("capture", 151_usize), cx);
        window.press("up", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 0"));
        window.press("down", cx);
        assert_eq!(window.find("capture-heading").label(), Some("Capture 151"));
        window.within("result-filter").click(0_usize, cx);
        assert!(window.find(("capture", 151_usize)).visible());
        assert!(window.try_find(("capture", 0_usize)).is_none());
        window.click("capture-search", cx);
        window.input("Capture 100", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window
            .try_find("capture-heading")
            .is_some_and(|heading| heading.label() == Some("Capture 100"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find(("capture", 100_usize)).visible());
        assert!(window.try_find(("capture", 151_usize)).is_none());
        window.press("ctrl-a", cx);
        window.input("No such capture", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("capture-empty").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("capture-empty", cx);
        window.press("down", cx);
        window.press("end", cx);
        assert!(window.find("capture-empty").visible());
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
