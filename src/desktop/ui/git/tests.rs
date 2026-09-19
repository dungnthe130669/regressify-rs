use std::{path::Path, time::Duration};

use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, TestAppContext};

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
    std::fs::create_dir_all(directory.path().join("nested")).unwrap();
    std::fs::write(
        directory.path().join("nested/review.txt"),
        "review this change\n",
    )
    .unwrap();
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
        view.update(cx, |this, cx| {
            this.git.update(cx, |panel, cx| {
                assert!(panel.selection.contains("nested/review.txt"));
                panel.message.update(cx, |input, cx| {
                    input.set_value("reviewed change", window, cx)
                });
            });
            this.edit(
                crate::desktop::session::Session::duplicate_selected,
                window,
                cx,
            );
        });
        window.render_frame(cx);
        window.click("git-commit", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(3), |window, _| {
        window.try_find("guard-cancel").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("guard-cancel", cx);
        assert!(view.read(cx).unsaved());
        assert!(!view.read(cx).git.read(cx).active());
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
            assert!(panel.badge().contains("1 conflicts"));
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("git-open-vscode").visible());
    })
    .unwrap();
}
