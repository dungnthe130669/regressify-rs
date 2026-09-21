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
            .diff_text
            .contains("+review this change")
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
        window.click("git-select-folder:blocked", cx);
        assert_eq!(view.read(cx).git.read(cx).selection, expected);
        view.read(cx).git.clone().update(cx, |panel, cx| {
            panel.set_disabled(true, cx);
        });
        window.render_frame(cx);
        window.click("git-select-folder:nested", cx);
        assert_eq!(view.read(cx).git.read(cx).selection, expected);
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
