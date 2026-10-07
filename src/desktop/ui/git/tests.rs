use std::{path::Path, time::Duration};

use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, Focusable, TestAppContext, px, size};

use crate::desktop::git::{self as model, Cancellation};

fn git(root: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[gpui_kit::test]
fn git_activity_refresh_preserves_editing_focus(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    git(directory.path(), &["init", "-b", "main"]);
    let snapshot = model::status(directory.path(), &Cancellation::default()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().canonicalize().unwrap());
                panel.install_snapshot(snapshot, window, cx);
                panel.edit_remote(super::RemoteEdit::Add, window, cx);
            });
            this.select_view(3, window, cx);
        });
        let panel = view.read(cx).git.clone();
        let inputs = [
            panel.read(cx).remote_name.read(cx).focus_handle(cx),
            panel.read(cx).remote_url.read(cx).focus_handle(cx),
            panel.read(cx).message.read(cx).focus_handle(cx),
        ];
        for follow in [false, true] {
            for (index, focus) in inputs.iter().enumerate() {
                window.focus(focus, cx);
                window.render_frame(cx);
                panel.update(cx, |panel, cx| {
                    panel.activity_follow = follow;
                    panel
                        .activity_history
                        .push_back(format!("Refresh {follow} {index}"));
                    panel.sync_activity_log(window, cx);
                });
                assert!(focus.is_focused(window), "follow={follow}, input={index}");
                assert!(
                    panel
                        .read(cx)
                        .activity_log
                        .read(cx)
                        .value()
                        .contains("Refresh")
                );
            }
        }
    })
    .unwrap();
}

#[gpui_kit::test]
async fn remote_management_forms_menu_confirmation_and_logs_work_in_app(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    for remote in [&first, &second] {
        git(remote.path(), &["init", "--bare", "-b", "main"]);
    }
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    git(directory.path(), &["init", "-b", "main"]);
    let snapshot = model::status(directory.path(), &Cancellation::default()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().canonicalize().unwrap());
                panel.install_snapshot(snapshot, window, cx);
            });
            this.select_view(3, window, cx);
        });
    })
    .unwrap();
    for (name, remote) in [("origin", &first), ("z-team", &second)] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("git-remote-add", cx);
            window.click("git-remote-name", cx);
            window.press("ctrl-a", cx);
            window.input(name, cx);
            window.click("git-remote-url", cx);
            window.input(remote.path().to_str().unwrap(), cx);
            window.click("git-remote-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("git-operation-confirm").is_some()
        })
        .await;
        super::super::tests::settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("git-operation-confirm", cx)
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(30), |_, cx| {
            let panel = view.read(cx).git.read(cx);
            !panel.active()
                && panel
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.remotes.iter().any(|remote| remote == name))
        })
        .await;
        super::super::tests::settle_dialog(cx, handle).await;
    }
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("git-remote-menu", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.within("popup-menu").try_find(1_usize).is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.within("popup-menu").find(1_usize).label(),
            Some("z-team")
        );
        window.within("popup-menu").click(1_usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("git-operation-confirm").is_some()
    })
    .await;
    super::super::tests::settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("git-operation-confirm", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(30), |_, cx| {
        let panel = view.read(cx).git.read(cx);
        !panel.active()
            && panel
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.selected_remote.as_deref() == Some("z-team"))
    })
    .await;
    super::super::tests::settle_dialog(cx, handle).await;
    for (button, push, remote) in [
        ("git-edit-fetch-url", false, &first),
        ("git-edit-push-url", true, &second),
    ] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.click(button, cx);
            window.click("git-remote-url", cx);
            window.press("ctrl-a", cx);
            window.input(remote.path().to_str().unwrap(), cx);
            window.click("git-remote-save", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("git-operation-confirm").is_some()
        })
        .await;
        super::super::tests::settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("git-operation-confirm", cx)
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(30), |_, cx| {
            let panel = view.read(cx).git.read(cx);
            !panel.active()
                && panel.remote_edit.is_none()
                && panel.snapshot.as_ref().is_some_and(|snapshot| {
                    let urls = &snapshot.remote_urls["z-team"];
                    (if push { &urls.push } else { &urls.fetch })
                        .first()
                        .and_then(Option::as_deref)
                        == remote.path().to_str()
                })
        })
        .await;
        super::super::tests::settle_dialog(cx, handle).await;
    }
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("git-remote-rename", cx);
        window.click("git-remote-name", cx);
        window.press("ctrl-a", cx);
        window.input("mirror", cx);
        window.click("git-remote-save", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("git-operation-confirm").is_some()
    })
    .await;
    super::super::tests::settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("git-operation-confirm", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(30), |_, cx| {
        let panel = view.read(cx).git.read(cx);
        !panel.active()
            && panel
                .snapshot
                .as_ref()
                .and_then(model::Snapshot::default_remote)
                == Some("mirror")
    })
    .await;
    super::super::tests::settle_dialog(cx, handle).await;
    for confirm in [false, true] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("git-remote-remove", cx)
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("git-operation-confirm").is_some()
        })
        .await;
        super::super::tests::settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click(
                if confirm {
                    "git-operation-confirm"
                } else {
                    "git-operation-cancel"
                },
                cx,
            )
        })
        .unwrap();
        if confirm {
            cx.wait_for(handle.into(), Duration::from_secs(30), |_, cx| {
                let panel = view.read(cx).git.read(cx);
                !panel.active()
                    && panel
                        .snapshot
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.remotes == ["origin"])
            })
            .await;
        } else {
            cx.update_window(handle.into(), |_, _, cx| {
                assert_eq!(
                    view.read(cx)
                        .git
                        .read(cx)
                        .snapshot
                        .as_ref()
                        .unwrap()
                        .remotes
                        .len(),
                    2
                )
            })
            .unwrap();
        }
        super::super::tests::settle_dialog(cx, handle).await;
    }
    cx.update_window(handle.into(), |_, window, cx| {
        let panel = view.read(cx).git.clone();
        assert!(
            panel
                .read(cx)
                .snapshot
                .as_ref()
                .unwrap()
                .default_remote()
                .is_none()
        );
        assert!(
            panel
                .read(cx)
                .activity_text
                .contains("Remote 'mirror' removed locally")
        );
        assert!(panel.read(cx).activity_text.contains("Running git remote"));
        window.click("git-log-copy", cx);
        assert!(
            cx.read_from_clipboard()
                .unwrap()
                .text()
                .unwrap()
                .contains("removed locally")
        );
        window.click("git-activity-toggle", cx);
        assert!(window.try_find("git-logs").is_none());
        window.click("git-activity-toggle", cx);
        assert!(window.find("git-logs").visible());
        window.click("git-log-clear", cx);
        assert!(panel.read(cx).activity_text.is_empty());
        panel.update(cx, |panel, _| panel.cancel_all());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn remote_editor_rejects_configuration_changed_after_opening_and_keeps_the_draft(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let remote = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    git(remote.path(), &["init", "--bare"]);
    git(external.path(), &["init", "--bare"]);
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    git(directory.path(), &["init", "-b", "main"]);
    git(
        directory.path(),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    let snapshot = model::status(directory.path(), &Cancellation::default()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().canonicalize().unwrap());
                panel.last_fetch_attempt = Some(std::time::Instant::now());
                panel.install_snapshot(snapshot, window, cx);
            });
            this.select_view(3, window, cx);
        });
        window.render_frame(cx);
        window.click("git-edit-fetch-url", cx);
        window.click("git-remote-url", cx);
        window.press("ctrl-a", cx);
        window.input(remote.path().to_str().unwrap(), cx);
    })
    .unwrap();
    git(
        directory.path(),
        &[
            "remote",
            "set-url",
            "origin",
            external.path().to_str().unwrap(),
        ],
    );
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("git-remote-save", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("git-operation-confirm").is_some()
    })
    .await;
    super::super::tests::settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("git-operation-confirm", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(30), |_, cx| {
        let panel = view.read(cx).git.read(cx);
        !panel.active() && panel.status.contains("remote or upstream changed")
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let panel = view.read(cx).git.clone();
        assert!(panel.read(cx).remote_edit.is_some());
        assert_eq!(
            panel.read(cx).remote_url.read(cx).value().as_ref(),
            remote.path().to_str().unwrap()
        );
        assert_eq!(
            panel.read(cx).snapshot.as_ref().unwrap().remote_urls["origin"].fetch[0].as_deref(),
            external.path().to_str()
        );
        window.click("git-remote-edit-cancel", cx);
        assert!(panel.read(cx).remote_edit.is_none());
        panel.update(cx, |panel, _| panel.cancel_all());
    })
    .unwrap();
}

#[gpui_kit::test]
fn remote_controls_and_live_activity_fit_compact_and_wide_windows(cx: &mut TestAppContext) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) =
            super::super::tests::editor_at_size(cx, directory.path(), size(px(width), px(height)));
        let reporter = Cancellation::reporting();
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.git.update(cx, |panel, cx| {
                    let name = "team-with-a-long-name".repeat(6);
                    panel.install_snapshot(
                        model::Snapshot {
                            repository: Some(model::Repository {
                                root: directory.path().into(),
                                git_dir: directory.path().join(".git"),
                            }),
                            branch: "main".into(),
                            remotes: vec![name.clone()],
                            selected_remote: Some(name.clone()),
                            changes: ["nested/file.txt", ".regressify/qa-review/report.json"]
                                .into_iter()
                                .map(|path| model::Change {
                                    path: path.into(),
                                    original_path: None,
                                    index: '.',
                                    worktree: 'M',
                                    untracked: false,
                                    conflict: false,
                                    submodule: false,
                                })
                                .collect(),
                            remote_urls: [(
                                name,
                                model::RemoteUrls {
                                    fetch: vec![Some(format!(
                                        "https://example.test/{}",
                                        "repository".repeat(60)
                                    ))],
                                    push: vec![None],
                                },
                            )]
                            .into(),
                            ..Default::default()
                        },
                        window,
                        cx,
                    );
                    panel.comparison_open = true;
                    panel.status = "Push in progress".into();
                    panel.action = Some(reporter.clone());
                    panel.begin_activity("Push".into(), reporter.clone(), true, window, cx);
                    reporter.stage("Receiving objects: 10%");
                    panel.sync_activity_log(window, cx);
                });
                this.select_view(3, window, cx);
            });
            window.render_frame(cx);
            for id in [
                "git-remote-menu",
                "git-remote-add",
                "git-remote-rename",
                "git-remote-remove",
                "git-fetch-url",
                "git-commit",
                "git-progress",
                "git-logs",
                "git-stop",
            ] {
                let element = window.find(id);
                let bounds = element.bounds();
                assert!(
                    element.visible(),
                    "{id} must remain visible at {width}x{height}"
                );
                assert!(
                    bounds.right() <= px(width) && bounds.bottom() <= px(height),
                    "{id} must remain within the window at {width}x{height}"
                );
            }
            assert!(window.find(("git-select", 1_usize)).visible());
            assert!(
                window.find("git-commit").bounds().bottom()
                    <= window.find("git-changes-panel").bounds().bottom()
            );
            assert!(
                window.find("git-changes-panel").bounds().right()
                    <= window.find("git-activity").bounds().left()
            );
            assert!(
                window.find("git-remote-menu").bounds().right()
                    <= window.find("git-remote-add").bounds().left()
            );
            assert!(view.read(cx).git.read(cx).activity_text.contains("10%"));
            let panel = view.read(cx).git.clone();
            let log = panel.read(cx).activity_log.clone();
            assert_eq!(log.read(cx).cursor_position().line, 1);
            window.click("git-log-follow", cx);
            log.update(cx, |state, cx| {
                state.set_cursor_position(lsp_types::Position::new(0, 0), window, cx)
            });
            panel.update(cx, |panel, cx| {
                reporter.stage("Receiving objects: 20%");
                panel.sync_activity_log(window, cx);
            });
            assert_eq!(
                log.read(cx).cursor_position(),
                lsp_types::Position::new(0, 0)
            );
            window.click("git-log-follow", cx);
            assert_eq!(log.read(cx).cursor_position().line, 2);
            window.click("git-remote-add", cx);
            assert!(view.read(cx).git.read(cx).remote_edit.is_none());
            window.click("git-stop", cx);
            assert!(reporter.is_cancelled());
            view.read(cx).git.clone().update(cx, |panel, cx| {
                panel.action = None;
                panel.finish_activity(false, "Git operation cancelled", window, cx);
            });
            window.render_frame(cx);
            assert!(
                window
                    .find("git-progress")
                    .label()
                    .unwrap()
                    .contains("Cancelled")
            );
            window.click(("git-select", 1_usize), cx);
            assert!(
                view.read(cx)
                    .git
                    .read(cx)
                    .selection
                    .contains(".regressify/qa-review/report.json")
            );
            view.read(cx)
                .git
                .clone()
                .update(cx, |panel, _| panel.cancel_all());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
async fn untracked_workspace_can_initialize_through_guarded_git_ui(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.set_workspace(Some(directory.path().canonicalize().unwrap()), window, cx)
            });
            this.select_view(3, window, cx);
        });
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(15), |window, _| {
        window.try_find("git-untracked-workspace").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(view.read(cx).git.read(cx).badge(), "Not tracked");
        window.click("git-init", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("git-operation-confirm").is_some()
    })
    .await;
    super::super::tests::settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("git-operation-confirm", cx)
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(15), |_, cx| {
        let panel = view.read(cx).git.read(cx);
        !panel.active()
            && panel
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.repository.is_some())
    })
    .await;
    cx.update_window(handle.into(), |_, _, cx| {
        assert!(directory.path().join(".git").is_dir());
        assert!(view.read(cx).git.read(cx).status.contains("initialized"));
        view.read(cx)
            .git
            .clone()
            .update(cx, |panel, _| panel.cancel_all());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn git_tree_reviews_files_and_commit_preserves_the_unsaved_guard(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    git(directory.path(), &["init", "-b", "main"]);
    std::fs::create_dir_all(directory.path().join("nested/child")).unwrap();
    std::fs::write(
        directory.path().join("nested/review.txt"),
        "review this change\n",
    )
    .unwrap();
    std::fs::write(directory.path().join("nested/child/review.txt"), "child\n").unwrap();
    std::fs::write(directory.path().join("nested-other.txt"), "leave alone\n").unwrap();
    let snapshot = model::status(directory.path(), &Cancellation::default()).unwrap();
    let index = snapshot
        .changes
        .iter()
        .position(|change| change.path == "nested/review.txt")
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().canonicalize().unwrap());
                panel.install_snapshot(snapshot, window, cx);
            });
            this.select_view(3, window, cx);
        });
        window.render_frame(cx);
        assert!(window.find("git-folder:nested").visible());
        window.click(("git-file", index), cx);
        window.click(("git-select", index), cx);
        window.render_frame(cx);
        assert_eq!(view.read(cx).git.read(cx).selection.len(), 1);
        window.click("git-folder:nested", cx);
        window.render_frame(cx);
        assert!(window.try_find(("git-file", index)).is_none());
        window.click("git-select-folder:nested", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("git-select-folder:nested").checked(),
            Some(true)
        );
        let selection = &view.read(cx).git.read(cx).selection;
        assert_eq!(selection.len(), 2);
        assert!(selection.contains("nested/review.txt"));
        assert!(selection.contains("nested/child/review.txt"));
        assert!(!selection.contains("nested-other.txt"));
        window.click("git-select-folder:nested", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("git-select-folder:nested").checked(),
            Some(false)
        );
        assert!(view.read(cx).git.read(cx).selection.is_empty());
        window.click("git-select-folder:nested", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(15), |_, cx| {
        view.read(cx)
            .git
            .read(cx)
            .text_review
            .as_ref()
            .and_then(|review| review.as_ref().ok())
            .is_some_and(|review| review.after.concat().contains("review this change"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let message = view.read(cx).git.read(cx).message.clone();
        window.click("git-commit-message", cx);
        assert!(message.read(cx).focus_handle(cx).is_focused(window));
        window.render_frame(cx);
        window.input("reviewed change", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        view.read(cx).git.read(cx).message.read(cx).value().as_ref() == "reviewed change"
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                assert!(panel.selection.contains("nested/review.txt"));
                assert_eq!(panel.message.read(cx).value().as_ref(), "reviewed change");
            });
            this.edit(
                crate::desktop::session::Session::duplicate_selected,
                window,
                cx,
            );
        });
    })
    .unwrap();
    for action in ["git-commit", "git-discard"] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click(action, cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
            window.try_find("guard-cancel").is_some()
        })
        .await;
        super::super::tests::settle_dialog(cx, handle).await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("guard-cancel", cx);
            assert!(view.read(cx).unsaved());
            assert!(!view.read(cx).git.read(cx).active());
        })
        .unwrap();
        super::super::tests::settle_dialog(cx, handle).await;
    }
    cx.update_window(handle.into(), |_, window, cx| {
        let panel = view.read(cx).git.clone();
        panel.update(cx, |panel, cx| {
            let mut snapshot = panel.snapshot.clone().unwrap();
            snapshot.changes.clear();
            panel.install_snapshot(snapshot, window, cx);
            assert!(
                panel.selection.is_empty()
                    && panel.rows.is_empty()
                    && panel.active_file.is_none()
                    && panel.diff_text.is_empty()
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn git_text_diff_displays_unparseable_saved_files(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    git(directory.path(), &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Git UI fixture"),
        ("user.email", "fixture@example.test"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "disabled-hooks"),
        ("core.autocrlf", "false"),
    ] {
        git(directory.path(), &["config", key, value]);
    }
    let path = "visual_tests/ui-test.tests.yaml";
    let before = "scenarios: [}\n# committed draft\n";
    std::fs::write(directory.path().join(path), before).unwrap();
    git(directory.path(), &["add", "."]);
    git(directory.path(), &["commit", "-m", "initial"]);
    std::fs::write(
        directory.path().join(path),
        before.replace("committed", "edited"),
    )
    .unwrap();
    std::fs::write(directory.path().join("invalid.yaml"), "scenarios: [}").unwrap();
    std::fs::write(directory.path().join("binary.bin"), [0_u8, 1, 2]).unwrap();
    let snapshot = model::status(directory.path(), &Cancellation::default()).unwrap();
    let index = snapshot
        .changes
        .iter()
        .position(|change| change.path == path)
        .unwrap();
    let invalid = snapshot
        .changes
        .iter()
        .position(|change| change.path == "invalid.yaml")
        .unwrap();
    let binary = snapshot
        .changes
        .iter()
        .position(|change| change.path == "binary.bin")
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().into());
                panel.install_snapshot(snapshot, window, cx);
            });
            this.select_view(3, window, cx);
        });
        window.render_frame(cx);
        window.click(("git-file", index), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(15), |window, _| {
        window.try_find("git-text-diff").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("git-review-tabs").is_none());
        assert!(window.try_find("git-structured-review").is_none());
        assert!(window.try_find("git-review-unavailable").is_none());
        assert_eq!(
            window.find("git-before-line-0").label(),
            Some("scenarios: [}")
        );
        assert_eq!(
            window.find("git-after-line-1").label(),
            Some("# edited draft")
        );
        window.click("git-copy-before", cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), before);
        window.click("git-copy-after", cx);
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            before.replace("committed", "edited")
        );
        window.click(("git-file", invalid), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(15), |window, cx| {
        window.try_find("git-text-diff").is_some()
            && view.read(cx).git.read(cx).diff_cancel.is_none()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("git-structured-review").is_none());
        assert!(window.try_find("git-review-unavailable").is_none());
        assert!(window.find("git-text-diff").visible());
        window.click(("git-file", binary), cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(15), |window, cx| {
        window.try_find("git-review-unavailable").is_some()
            && view.read(cx).git.read(cx).diff_cancel.is_none()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("git-text-diff").is_none());
        let panel = view.read(cx).git.read(cx);
        assert!(
            panel
                .text_review
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap_err()
                .contains("Binary")
        );
        assert!(!panel.diff_text.is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn git_text_diff_scrolls_both_panes_and_navigates_changes(cx: &mut TestAppContext) {
    for dark in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, directory.path());
        let before = (0..180)
            .map(|index| format!("line-{index}: {}\r\n", "long text ".repeat(20)))
            .collect::<String>();
        let after = before
            .replace("line-10:", "added: one\r\nadded: two\r\nline-10:")
            .replace("line-150:", "changed-150:");
        cx.update_window(handle.into(), |_, window, cx| {
            super::super::design::apply_theme(dark, None, cx);
            view.update(cx, |this, cx| {
                this.git.update(cx, |panel, cx| {
                    panel.install_snapshot(
                        model::Snapshot {
                            repository: Some(model::Repository {
                                root: directory.path().into(),
                                git_dir: directory.path().join(".git"),
                            }),
                            branch: "main".into(),
                            ..Default::default()
                        },
                        window,
                        cx,
                    );
                    panel.active_file = Some("invalid.yaml".into());
                    panel.text_review = Some(Ok(model::review::compare_text(
                        &before,
                        &after,
                        &Cancellation::default(),
                    )
                    .unwrap()));
                });
                this.select_view(3, window, cx);
            });
            window.render_frame(cx);
            window.click("git-diff-next", cx);
            window.click("git-diff-next", cx);
            window.render_frame(cx);
            assert_eq!(view.read(cx).git.read(cx).diff_change, Some(1));
            let before_row = window.find("git-before-line-152").bounds();
            let after_row = window.find("git-after-line-152").bounds();
            assert_eq!(before_row.top(), after_row.top());
            assert!(before_row.top() >= window.find("git-diff-before").bounds().top());
            let scroll = view
                .read(cx)
                .git
                .read(cx)
                .diff_scroll
                .0
                .borrow()
                .base_handle
                .clone();
            let previous = scroll.offset();
            window.scroll(
                "git-diff-after",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(-160.), px(0.))),
                cx,
            );
            window.render_frame(cx);
            assert!(scroll.offset().x < previous.x);
            window.scroll(
                "git-diff-after",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-24.))),
                cx,
            );
            window.render_frame(cx);
            assert!(scroll.offset().y < previous.y);
            assert_eq!(
                window.find("git-before-line-152").bounds().top(),
                window.find("git-after-line-152").bounds().top()
            );
            window.click("git-diff-previous", cx);
            window.render_frame(cx);
            assert_eq!(view.read(cx).git.read(cx).diff_change, Some(0));
            assert_eq!(
                window.find("git-before-line-10").bounds().top(),
                window.find("git-after-line-10").bounds().top()
            );
            window.click("git-copy-before", cx);
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), before);
            window.click("git-copy-after", cx);
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), after);
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn comparison_changes_require_explicit_selection_and_stay_separate(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    let paths = [
        "project/suite.tests.yaml",
        "project/.regressify/result.json",
        ".regressify__staging/result.json",
    ];
    let snapshot = model::Snapshot {
        repository: Some(model::Repository {
            root: directory.path().into(),
            git_dir: directory.path().join(".git"),
        }),
        branch: "main".into(),
        changes: paths
            .iter()
            .map(|path| model::Change {
                path: (*path).into(),
                original_path: None,
                index: 'M',
                worktree: '.',
                untracked: false,
                conflict: false,
                submodule: false,
            })
            .collect(),
        ..Default::default()
    };
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().into());
                panel.install_snapshot(snapshot.clone(), window, cx);
            });
            this.select_view(3, window, cx);
        });
        window.render_frame(cx);
        let panel = view.read(cx).git.clone();
        assert!(!panel.read(cx).comparison_open);
        assert!(panel.read(cx).selection.is_empty());
        assert!(window.try_find(("git-file", 1_usize)).is_none());
        assert_eq!(
            window.find("git-change-count").label(),
            Some("1 changed files")
        );
        window.click("git-select-folder:project", cx);
        assert_eq!(
            panel.read(cx).selection,
            [paths[0].to_string()].into_iter().collect()
        );
        window.click("git-select-all", cx);
        assert!(panel.read(cx).selection.is_empty());
        window.click("git-select-all", cx);
        assert_eq!(panel.read(cx).selection.len(), 1);
        window.click("git-toggle-comparison", cx);
        window.render_frame(cx);
        assert!(window.find(("git-file", 1_usize)).visible());
        assert!(window.find(("git-file", 2_usize)).visible());
        window.click(("git-select", 1_usize), cx);
        assert!(panel.read(cx).selection.contains(paths[1]));
        assert!(panel.read(cx).active_file.is_none());
        window.press("space", cx);
        assert!(!panel.read(cx).selection.contains(paths[1]));
        assert!(panel.read(cx).selection.contains(paths[0]));
        window.press("space", cx);
        assert!(panel.read(cx).selection.contains(paths[1]));
        assert!(panel.read(cx).active_file.is_none());
        window.click("git-select-all", cx);
        assert_eq!(
            panel.read(cx).selection,
            [paths[1].to_string()].into_iter().collect()
        );
        window.click("git-toggle-comparison", cx);
        assert!(panel.read(cx).selection.contains(paths[1]));
        panel.update(cx, |panel, cx| panel.install_snapshot(snapshot, window, cx));
        assert!(!panel.read(cx).selection.contains(paths[2]));
        window.click("git-toggle-comparison", cx);
        window.click("git-select-all-comparison", cx);
        assert_eq!(panel.read(cx).selection.len(), 2);
        window.click("git-select-all-comparison", cx);
        assert!(panel.read(cx).selection.is_empty());
        window.click(("git-select", 0_usize), cx);
        window.press("ctrl-a", cx);
        assert_eq!(
            panel.read(cx).selection,
            [paths[0].to_string()].into_iter().collect()
        );
        window.press("end", cx);
        window.press("space", cx);
        assert!(panel.read(cx).selection.is_empty());
        assert!(panel.read(cx).active_file.is_none());
        window.press("home", cx);
        window.press("enter", cx);
        assert!(window.try_find(("git-file", 0_usize)).is_none());
        window.press("enter", cx);
        assert!(window.find(("git-file", 0_usize)).visible());
        window.press("ctrl-a", cx);
        window.click(("git-select", 1_usize), cx);
        window.press("ctrl-a", cx);
        assert_eq!(panel.read(cx).selection.len(), 3);
        window.press("ctrl-shift-a", cx);
        assert_eq!(
            panel.read(cx).selection,
            [paths[0].to_string()].into_iter().collect()
        );
        window.press("ctrl-a", cx);
        window.press("escape", cx);
        assert_eq!(
            panel.read(cx).selection,
            [paths[0].to_string()].into_iter().collect()
        );
        window.click("git-commit-message", cx);
        window.press("ctrl-a", cx);
        window.input("Keyboard selection stays scoped", cx);
        assert_eq!(
            panel.read(cx).selection,
            [paths[0].to_string()].into_iter().collect()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn folder_selection_excludes_conflicts_and_submodules_and_respects_disabled_state(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    git(directory.path(), &["init", "-b", "main"]);
    std::fs::create_dir_all(directory.path().join("nested/child")).unwrap();
    std::fs::create_dir(directory.path().join("blocked")).unwrap();
    for path in [
        "nested/file.txt",
        "nested/child/file.txt",
        "nested/conflict.txt",
        "nested/submodule",
        "blocked/submodule",
    ] {
        std::fs::write(directory.path().join(path), "change\n").unwrap();
    }
    let mut snapshot = model::status(directory.path(), &Cancellation::default()).unwrap();
    for change in &mut snapshot.changes {
        change.conflict = change.path.ends_with("conflict.txt");
        change.submodule = change.path.ends_with("submodule");
    }
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().canonicalize().unwrap());
                panel.install_snapshot(snapshot, window, cx);
            });
            this.select_view(3, window, cx);
        });
        window.render_frame(cx);
        window.scroll(
            "git-select-folder:blocked",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-160.))),
            cx,
        );
        window.click("git-select-folder:nested", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("git-select-folder:nested").checked(),
            Some(true)
        );
        let expected = [
            "nested/child/file.txt".to_string(),
            "nested/file.txt".to_string(),
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(view.read(cx).git.read(cx).selection, expected);
        window.scroll(
            "git-select-folder:nested",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(1600.))),
            cx,
        );
        window.click("git-select-folder:blocked", cx);
        assert_eq!(view.read(cx).git.read(cx).selection, expected);
        window.press("ctrl-a", cx);
        let allowed = view
            .read(cx)
            .git
            .read(cx)
            .snapshot
            .as_ref()
            .unwrap()
            .changes
            .iter()
            .filter(|change| !change.is_comparison() && !change.conflict && !change.submodule)
            .map(|change| change.path.clone())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(view.read(cx).git.read(cx).selection, allowed);
        window.press("escape", cx);
        assert!(view.read(cx).git.read(cx).selection.is_empty());
        window.scroll(
            "git-select-folder:blocked",
            gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-160.))),
            cx,
        );
        window.click("git-select-folder:nested", cx);
        view.read(cx).git.clone().update(cx, |panel, cx| {
            panel.set_disabled(true, cx);
        });
        window.render_frame(cx);
        window.click("git-select-folder:nested", cx);
        assert_eq!(view.read(cx).git.read(cx).selection, expected);
        for key in [
            "ctrl-a",
            "ctrl-shift-a",
            "escape",
            "ctrl-space",
            "end",
            "enter",
        ] {
            window.press(key, cx);
            assert_eq!(view.read(cx).git.read(cx).selection, expected);
        }
    })
    .unwrap();
}

#[gpui_kit::test]
async fn discard_selected_folder_requires_confirmation_and_reloads_the_open_document(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    git(directory.path(), &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Git UI fixture"),
        ("user.email", "fixture@example.test"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "disabled-hooks"),
        ("core.autocrlf", "false"),
    ] {
        git(directory.path(), &["config", key, value]);
    }
    git(directory.path(), &["add", "."]);
    git(directory.path(), &["commit", "-m", "initial"]);
    let file = directory.path().join("visual_tests/ui-test.tests.yaml");
    let original = std::fs::read_to_string(&file).unwrap();
    let original_value: serde_json::Value = serde_yaml::from_str(&original).unwrap();
    let changed = "scenarios:\n- url: https://example.test/changed\n  label: Changed\n";
    std::fs::write(&file, changed).unwrap();
    let added = directory.path().join("visual_tests/new.tests.yaml");
    std::fs::write(&added, changed).unwrap();
    git(directory.path(), &["add", "visual_tests/new.tests.yaml"]);
    std::fs::write(directory.path().join("keep.txt"), "unselected\n").unwrap();
    let snapshot = model::status(directory.path(), &Cancellation::default()).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            let document = this
                .session
                .workspace
                .as_ref()
                .unwrap()
                .open_document(this.session.open.as_ref().unwrap().entry.clone())
                .unwrap();
            this.session.install_document(document).unwrap();
            this.sync_editor(window, cx);
            assert_eq!(
                this.session.value().unwrap()["scenarios"][0]["label"],
                "Changed"
            );
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().canonicalize().unwrap());
                panel.install_snapshot(snapshot, window, cx);
                panel.message.update(cx, |message, cx| {
                    message.set_value("Keep this commit message", window, cx);
                });
            });
            this.select_view(3, window, cx);
        });
        window.render_frame(cx);
        window.click("git-select-folder:visual_tests", cx);
        window.render_frame(cx);
        assert_eq!(view.read(cx).git.read(cx).selection.len(), 2);
        window.click("git-discard", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("git-operation-confirm").is_some()
    })
    .await;
    super::super::tests::settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("git-confirm-files").visible());
        assert_eq!(
            window.find("git-operation-confirm").label(),
            Some("Discard selected files")
        );
        assert!(!view.read(cx).git.read(cx).active());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), changed);
        assert!(added.exists());
        window.click("git-operation-cancel", cx);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), changed);
        assert!(added.exists());
        assert_eq!(view.read(cx).git.read(cx).selection.len(), 2);
    })
    .unwrap();
    super::super::tests::settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("git-discard", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("git-operation-confirm").is_some()
    })
    .await;
    super::super::tests::settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("git-operation-confirm", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(15), |_, cx| {
        let workbench = view.read(cx);
        !workbench.busy && workbench.status.starts_with("Discarded changes")
    })
    .await;
    cx.update_window(handle.into(), |_, _, cx| {
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
        assert!(!added.exists());
        assert_eq!(
            std::fs::read_to_string(directory.path().join("keep.txt")).unwrap(),
            "unselected\n"
        );
        let workbench = view.read(cx);
        assert_eq!(
            workbench.session.value().unwrap()["scenarios"],
            original_value["scenarios"]
        );
        assert_eq!(workbench.main_tab, 3);
        let panel = workbench.git.read(cx);
        assert!(panel.selection.is_empty());
        assert_eq!(panel.snapshot.as_ref().unwrap().changes.len(), 1);
        assert_eq!(
            panel.message.read(cx).value().as_ref(),
            "Keep this commit message"
        );
        workbench
            .git
            .clone()
            .update(cx, |panel, _| panel.cancel_all());
    })
    .unwrap();
}

#[gpui_kit::test]
fn incoming_conflict_and_remote_failure_indicators_remain_visible(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                panel.root = Some(directory.path().canonicalize().unwrap());
                panel.install_snapshot(
                    model::Snapshot {
                        repository: Some(model::Repository {
                            root: directory.path().into(),
                            git_dir: directory.path().join(".git"),
                        }),
                        branch: "main".into(),
                        behind: 3,
                        ahead: 1,
                        ..Default::default()
                    },
                    window,
                    cx,
                );
            });
            this.select_view(3, window, cx);
        });
        window.render_frame(cx);
        assert!(window.find("git-status").visible());
        assert!(window.find("git-conflict-banner").visible());
        assert_eq!(
            window.find("git-conflict-banner").label(),
            Some("Branches diverged. Resolve in VS Code, then refresh Git status.")
        );
        assert!(view.read(cx).git.read(cx).badge().contains("3 incoming"));
        assert_eq!(
            window.find("git-incoming").label(),
            Some("3 incoming commits")
        );
        let panel = view.read(cx).git.clone();
        panel.update(cx, |panel, cx| {
            panel.snapshot.as_mut().unwrap().behind = 0;
            panel.remote_error = Some("Authentication required".into());
            assert!(panel.badge().contains("offline"));
            assert!(panel.attention());
            panel
                .snapshot
                .as_mut()
                .unwrap()
                .changes
                .push(model::Change {
                    path: "conflict.txt".into(),
                    original_path: None,
                    index: 'U',
                    worktree: 'U',
                    conflict: true,
                    untracked: false,
                    submodule: false,
                });
            panel.rebuild_tree();
            assert!(panel.badge().contains("1 conflict"));
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("git-open-vscode").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn tracking_changes_cancel_old_fetches_and_clear_stale_remote_status(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        let panel = view.read(cx).git.clone();
        panel.update(cx, |panel, cx| {
            let previous = model::Snapshot {
                branch: "main".into(),
                remote_config: "old".into(),
                ..Default::default()
            };
            panel.install_snapshot(previous.clone(), window, cx);
            panel.last_fetch = Some(std::time::Instant::now());
            panel.last_fetch_attempt = Some(std::time::Instant::now());
            panel.remote_error = Some("old remote error".into());
            let cancellation = Cancellation::default();
            panel.fetch = Some(cancellation.clone());
            let generation = panel.remote_generation;
            let current = model::Snapshot {
                remote_config: "changed".into(),
                ..previous
            };
            panel.install_snapshot(current, window, cx);
            assert!(cancellation.is_cancelled());
            assert!(
                panel.fetch.is_none()
                    && panel.last_fetch.is_none()
                    && panel.last_fetch_attempt.is_none()
                    && panel.remote_error.is_none()
            );
            assert_ne!(panel.remote_generation, generation);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn pull_refresh_reloads_or_closes_the_current_document_without_leaving_git(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = super::super::tests::editor(cx, directory.path());
    let file = directory.path().join("visual_tests/ui-test.tests.yaml");
    std::fs::write(
        &file,
        "scenarios:\n- url: https://example.test/pulled\n  label: Pulled\n",
    )
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.status = "Pull completed (fast-forward only)".into();
            this.reload_after_git_operation(window, cx);
        })
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(10), |_, cx| {
        !view.read(cx).busy
    })
    .await;
    cx.update_window(handle.into(), |_, _, cx| {
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"][0]["label"],
            "Pulled"
        );
        assert_eq!(view.read(cx).main_tab, 3);
        assert_eq!(view.read(cx).status, "Pull completed (fast-forward only)");
    })
    .unwrap();
    std::fs::remove_file(file).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| this.reload_after_git_operation(window, cx))
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(10), |_, cx| {
        !view.read(cx).busy
    })
    .await;
    cx.update_window(handle.into(), |_, _, cx| {
        assert!(view.read(cx).session.open.is_none());
        assert_eq!(view.read(cx).main_tab, 3);
    })
    .unwrap();
}

#[gpui_kit::test]
fn git_text_diff_and_comparison_panels_fit_compact_and_wide_windows(cx: &mut TestAppContext) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) =
            super::super::tests::editor_at_size(cx, directory.path(), size(px(width), px(height)));
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.git.update(cx, |panel, cx| {
                    panel.install_snapshot(
                        model::Snapshot {
                            repository: Some(model::Repository {
                                root: directory.path().into(),
                                git_dir: directory.path().join(".git"),
                            }),
                            branch: "main".into(),
                            changes: ["suite.tests.yaml", ".regressify/report.json"]
                                .into_iter()
                                .map(|path| model::Change {
                                    path: path.into(),
                                    original_path: None,
                                    index: '.',
                                    worktree: 'M',
                                    untracked: false,
                                    conflict: false,
                                    submodule: false,
                                })
                                .collect(),
                            ..Default::default()
                        },
                        window,
                        cx,
                    );
                    panel.active_file = Some("suite.tests.yaml".into());
                    panel.text_review = Some(
                        model::review::compare_text(
                            &format!("https://old.test/{}", "a".repeat(500)),
                            &format!("https://new.test/{}", "b".repeat(500)),
                            &Cancellation::default(),
                        )
                        .map_err(|error| format!("{error:#}")),
                    );
                });
                this.select_view(3, window, cx);
            });
            window.render_frame(cx);
            let collapsed = window.find("git-comparison-changes").bounds();
            assert!(collapsed.size.height <= px(40.));
            window.click("git-toggle-comparison", cx);
            window.render_frame(cx);
            let panel = window.find("git-comparison-changes").bounds();
            let toggle = window.find("git-toggle-comparison").bounds();
            let all = window.find("git-select-all-comparison").bounds();
            assert!(toggle.right() <= all.left() && all.right() <= panel.right());
            assert!(panel.top() >= window.find("git-change-count").bounds().bottom());
            assert!(panel.bottom() <= window.find("git-commit-message").bounds().top());
            let before = window.find("git-diff-before").bounds();
            let after = window.find("git-diff-after").bounds();
            let review = window.find("git-text-diff").bounds();
            assert!(before.left() >= review.left());
            assert!(before.right() <= after.left());
            assert!(after.right() <= review.right());
            assert!((before.size.width - after.size.width).abs() <= px(1.));
            assert!(window.find("git-commit").visible());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn git_large_tree_and_status_fit_compact_and_wide_windows(cx: &mut TestAppContext) {
    for (width, height) in [(1080., 720.), (1600., 1000.)] {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) =
            super::super::tests::editor_at_size(cx, directory.path(), size(px(width), px(height)));
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.git.update(cx, |panel, cx| {
                    panel.root = Some(directory.path().into());
                    panel.install_snapshot(
                        model::Snapshot {
                            repository: Some(model::Repository {
                                root: directory.path().into(),
                                git_dir: directory.path().join(".git"),
                            }),
                            branch: "feature-with-a-long-name".repeat(4),
                            ahead: 100_000,
                            behind: 100_000,
                            changes: (0..5000)
                                .map(|index| model::Change {
                                    path: format!(
                                        "folder-{index:04}/file-with-a-long-name-{index}.txt"
                                    ),
                                    original_path: None,
                                    index: '.',
                                    worktree: 'M',
                                    untracked: false,
                                    conflict: false,
                                    submodule: false,
                                })
                                .collect(),
                            ..Default::default()
                        },
                        window,
                        cx,
                    );
                    panel.status = "Repository status refreshed".into();
                });
                this.select_view(3, window, cx);
            });
            window.render_frame(cx);
            let badge = window.find("git-status").bounds();
            assert!(badge.size.width <= px(180.));
            let folder = window.find("open-workspace").bounds();
            assert!(folder.origin.x + folder.size.width <= px(width - 95.));
            assert!(window.find("git-commit-message").visible());
            let commit = window.find("git-commit").bounds();
            let discard = window.find("git-discard").bounds();
            assert!(window.find("git-commit").visible());
            assert!(window.find("git-discard").visible());
            assert!(discard.origin.x + discard.size.width <= px(width));
            assert!(discard.origin.y + discard.size.height <= px(height));
            assert!(
                discard.origin.y >= commit.origin.y + commit.size.height
                    || discard.origin.x >= commit.origin.x + commit.size.width,
                "Git actions must not overlap"
            );
            assert!(
                window.try_find(("git-file", 4999_usize)).is_none(),
                "offscreen tree rows must be virtualized"
            );
            assert_eq!(view.read(cx).git.read(cx).rows.len(), 10_000);
        })
        .unwrap();
    }
}
