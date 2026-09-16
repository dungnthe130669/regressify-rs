use std::{path::Path, time::Duration};

use gpui_kit::component::Root;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, Entity, Focusable, TestAppContext, WindowHandle, px, size};

use super::Workbench;
use crate::desktop::session::Selection;

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

#[gpui_kit::test]
async fn run_preflight_rejects_a_selected_suite_changed_on_disk(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
            std::fs::write(
                directory.path().join("visual_tests/ui-test.tests.yaml"),
                "scenarios: [",
            )
            .unwrap();
            this.start_selected_run(window, cx);
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| window.try_find("status").is_some_and(|status| status.label() == Some("Selected suites changed or contain errors; review the selection before running"))).await;
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

fn editor(cx: &mut TestAppContext, root: &Path) -> (WindowHandle<Root>, Entity<Workbench>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| super::design::apply_theme(false, None, cx));
    cx.update(super::bind_keys);
    let mut created = None;
    let handle = cx.open_window(size(px(1280.), px(820.)), |window, cx| {
        let view = cx.new(|cx| Workbench::new(window, cx));
        view.update(cx, |this, cx| {
            this.preferences = crate::desktop::workspace::Preferences::default();
            this.preferences_path = Some(root.join("desktop-preferences.json"));
            this.apply_editor_display(window, cx);
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
        lodepng::encode32_file(path, &[0_u8, 128, 255, 255], 1, 1).unwrap();
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
            Some("Current 1 x 1")
        );
        window.click("zoom-in", cx);
        assert_eq!(window.find("zoom-value").label(), Some("125%"));
        window.click("actual-size", cx);
        assert_eq!(window.find("zoom-value").label(), Some("100%"));
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
