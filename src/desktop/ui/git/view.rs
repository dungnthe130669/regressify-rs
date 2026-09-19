use super::*;

pub(super) struct TreeRow {
    path: String,
    label: String,
    depth: usize,
    file: Option<usize>,
}

fn visible_name(value: &str) -> String {
    value
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

impl GitPanel {
    pub(super) fn rebuild_tree(&mut self) {
        let mut rows = Vec::new();
        if let Some(snapshot) = &self.snapshot {
            fn flatten(
                tree: &model::ChangeTree,
                prefix: &str,
                depth: usize,
                snapshot: &Snapshot,
                collapsed: &BTreeSet<String>,
                rows: &mut Vec<TreeRow>,
            ) {
                for (label, child) in &tree.folders {
                    let path = format!("{prefix}{label}");
                    rows.push(TreeRow {
                        path: path.clone(),
                        label: visible_name(label),
                        depth,
                        file: None,
                    });
                    if !collapsed.contains(&path) {
                        flatten(
                            child,
                            &format!("{path}/"),
                            depth + 1,
                            snapshot,
                            collapsed,
                            rows,
                        );
                    }
                }
                for index in &tree.files {
                    let change = &snapshot.changes[*index];
                    rows.push(TreeRow {
                        path: change.path.clone(),
                        label: visible_name(change.path.rsplit('/').next().unwrap_or(&change.path)),
                        depth,
                        file: Some(*index),
                    });
                }
            }
            flatten(
                &model::ChangeTree::new(&snapshot.changes),
                "",
                0,
                snapshot,
                &self.collapsed,
                &mut rows,
            );
        }
        self.rows = rows;
    }

    fn tree_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = &self.rows[index];
        let path = row.path.clone();
        let label = row.label.clone();
        let mut element = div()
            .id(("git-tree-row", index))
            .test_support()
            .flex()
            .items_center()
            .h(px(30.))
            .w_full()
            .min_w_0()
            .pl(px(8. + row.depth.min(12) as f32 * 14.))
            .pr_2()
            .gap_1();
        if let Some(file) = row.file {
            let change = &self
                .snapshot
                .as_ref()
                .expect("tree requires snapshot")
                .changes[file];
            let selected = self.selection.contains(&path);
            let panel = cx.entity();
            let checkbox_path = path.clone();
            let conflict = change.conflict;
            let status = change.label();
            let staged = change.index != '.' && !change.untracked;
            let tooltip = format!(
                "{}\n{}{}{}",
                visible_name(&path),
                status,
                if staged { " | staged" } else { "" },
                change
                    .original_path
                    .as_ref()
                    .map(|original| format!("\nFrom {}", visible_name(original)))
                    .unwrap_or_default()
            );
            element = element
                .when(self.active_file.as_ref() == Some(&path), |row| {
                    row.bg(cx.theme().accent)
                })
                .child(
                    design::selection_checkbox(
                        ("git-select", file),
                        selected,
                        false,
                        self.disabled || self.active() || conflict || change.submodule,
                        cx,
                    )
                    .accessibility_label(format!("Select {}", visible_name(&path)))
                    .on_change(move |state, _, _, cx| {
                        panel.update(cx, |this, cx| {
                            if state == gpui_kit::base::CheckboxState::Checked {
                                this.selection.insert(checkbox_path.clone());
                            } else {
                                this.selection.remove(&checkbox_path);
                            }
                            cx.notify();
                        })
                    }),
                )
                .child(
                    Button::new(("git-file", file))
                        .ghost()
                        .xsmall()
                        .flex_1()
                        .min_w_0()
                        .justify_start()
                        .accessibility_label(format!("Review {}", visible_name(&path)))
                        .tooltip(tooltip)
                        .child(
                            Icon::new(if conflict {
                                gpui_kit::assets::IconName::TriangleAlert
                            } else {
                                gpui_kit::assets::IconName::FileDiff
                            })
                            .size_3(),
                        )
                        .child(div().min_w_0().truncate().child(label))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.review(path.clone(), window, cx)
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .flex_shrink_0()
                        .text_color(if conflict {
                            cx.theme().danger
                        } else if change.untracked {
                            cx.theme().success
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(if conflict {
                            "!".into()
                        } else if change.untracked {
                            "U".into()
                        } else {
                            format!("{}{}", change.index, change.worktree)
                        }),
                );
        } else {
            let collapsed = self.collapsed.contains(&path);
            element = element.child(
                Button::new(SharedString::from(format!("git-folder:{path}")))
                    .ghost()
                    .xsmall()
                    .w_full()
                    .justify_start()
                    .icon(if collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .accessibility_label(format!("Folder {}", visible_name(&path)))
                    .tooltip(visible_name(&path))
                    .child(Icon::new(gpui_kit::assets::IconName::Folder).size_3())
                    .child(div().truncate().child(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.collapsed.remove(&path) {
                            this.collapsed.insert(path.clone());
                        }
                        this.rebuild_tree();
                        cx.notify();
                    })),
            );
        }
        element.into_any_element()
    }

    fn render_changes(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self
            .snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.changes.len());
        let allowed = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .changes
                    .iter()
                    .filter(|change| !change.conflict && !change.submodule)
                    .map(|change| change.path.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let all = !allowed.is_empty() && allowed.iter().all(|path| self.selection.contains(path));
        let disabled = self.disabled || self.active() || self.error.is_some();
        let message_empty = self.message.read(cx).value().trim().is_empty();
        let blocked = self.snapshot.as_ref().is_none_or(|snapshot| {
            snapshot.conflicts() > 0 || snapshot.operation.is_some() || snapshot.detached
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_3()
                    .h(px(38.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .id("git-change-count")
                            .role(Role::Status)
                            .aria_label(format!("{count} changed files"))
                            .test_support()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("Changes ({count})")),
                    )
                    .child(
                        Checkbox::new("git-select-all")
                            .label("All")
                            .checked(all)
                            .disabled(disabled || allowed.is_empty())
                            .on_click(cx.listener(move |this, checked, _, cx| {
                                if *checked {
                                    this.selection.extend(allowed.iter().cloned());
                                } else {
                                    this.selection.clear();
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when(count == 0, |tree| {
                        tree.child(
                            div()
                                .p_3()
                                .text_color(cx.theme().muted_foreground)
                                .child("Working tree clean"),
                        )
                    })
                    .when(count > 0, |tree| {
                        tree.child(
                            uniform_list(
                                "git-change-tree",
                                self.rows.len(),
                                cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .map(|index| this.tree_row(index, cx))
                                        .collect::<Vec<_>>()
                                }),
                            )
                            .size_full(),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} file(s) selected", self.selection.len())),
                    )
                    .child(
                        div().id("git-commit-message").test_support().child(
                            Textarea::new(&self.message)
                                .aria_label("Git commit message")
                                .h(px(80.))
                                .disabled(disabled),
                        ),
                    )
                    .child(
                        Button::new("git-commit")
                            .small()
                            .primary()
                            .icon(gpui_kit::assets::IconName::GitCommitHorizontal)
                            .label("Commit selected files")
                            .disabled(
                                disabled || blocked || message_empty || self.selection.is_empty(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request(
                                    model::Action::Commit {
                                        paths: this.selection.iter().cloned().collect(),
                                        message: this.message.read(cx).value().to_string(),
                                    },
                                    cx,
                                );
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_review(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .h(px(38.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div().flex_1().min_w_0().truncate().child(
                            self.active_file
                                .as_deref()
                                .map(visible_name)
                                .unwrap_or_else(|| "Diff review".into()),
                        ),
                    )
                    .child(
                        Button::new("git-open-file")
                            .ghost()
                            .xsmall()
                            .icon(gpui_kit::assets::IconName::ExternalLink)
                            .tooltip("Open file in VS Code")
                            .accessibility_label("Open file in VS Code")
                            .disabled(self.active_file.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.open_vscode(true, cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when(self.active_file.is_none(), |content| {
                        content.child(
                            div()
                                .p_4()
                                .text_color(cx.theme().muted_foreground)
                                .child("No file selected"),
                        )
                    })
                    .when(self.active_file.is_some(), |content| {
                        content.child(
                            CodeEditor::new(&self.diff)
                                .aria_label("Git diff")
                                .readonly(true)
                                .size_full(),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl Render for GitPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.disabled || self.active() || self.error.is_some();
        let tracked = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.repository.is_some());
        let snapshot = self.snapshot.as_ref();
        let conflicts = snapshot.map_or(0, Snapshot::conflicts);
        let diverged = snapshot.is_some_and(|snapshot| snapshot.ahead > 0 && snapshot.behind > 0);
        let operation = snapshot.and_then(|snapshot| snapshot.operation.as_deref());
        let blocked = conflicts > 0 || operation.is_some();
        let branch = snapshot
            .map(|snapshot| {
                if snapshot.detached {
                    "Detached HEAD".into()
                } else {
                    snapshot.branch.clone()
                }
            })
            .unwrap_or_else(|| "Git".into());
        let root = snapshot
            .and_then(|snapshot| snapshot.repository.as_ref())
            .map(|repository| display_path(&repository.root))
            .unwrap_or_default();
        let pull_disabled = disabled
            || blocked
            || diverged
            || snapshot.is_none_or(|snapshot| {
                snapshot.detached
                    || snapshot.remote.is_none()
                    || snapshot.merge_ref.is_none()
                    || !snapshot.changes.is_empty()
            });
        let push_disabled = disabled
            || blocked
            || snapshot.is_none_or(|snapshot| {
                snapshot.detached
                    || snapshot.head.is_none()
                    || snapshot.default_remote().is_none()
                    || snapshot.behind > 0
            });
        div()
            .id("git-panel")
            .test_support()
            .size_full()
            .flex()
            .flex_col()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Icon::new(gpui_kit::assets::IconName::GitBranch).size_4())
                    .child(
                        div()
                            .id("git-branch")
                            .role(Role::Status)
                            .aria_label(branch.clone())
                            .test_support()
                            .max_w(px(220.))
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(branch),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("git-refresh")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::RefreshCw)
                            .tooltip("Refresh local changes")
                            .accessibility_label("Refresh Git status")
                            .disabled(self.scan.is_some() || self.active())
                            .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx))),
                    )
                    .child(
                        Button::new("git-fetch")
                            .small()
                            .icon(gpui_kit::assets::IconName::CloudDownload)
                            .label("Fetch")
                            .tooltip("Check remote commits without changing files")
                            .disabled(
                                disabled
                                    || self.fetch.is_some()
                                    || snapshot.and_then(Snapshot::default_remote).is_none(),
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.check_remote(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("git-pull")
                            .small()
                            .icon(gpui_kit::assets::IconName::ArrowDown)
                            .label("Pull")
                            .tooltip("Fast-forward only; commit local changes first")
                            .disabled(pull_disabled)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.request(model::Action::Pull, cx)),
                            ),
                    )
                    .child(
                        Button::new("git-push")
                            .small()
                            .icon(gpui_kit::assets::IconName::ArrowUp)
                            .label("Push")
                            .tooltip("Push commits without overwriting remote history")
                            .disabled(push_disabled)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.request(model::Action::Push, cx)),
                            ),
                    )
                    .child(
                        Button::new("git-open-vscode")
                            .small()
                            .icon(gpui_kit::assets::IconName::ExternalLink)
                            .label("Open in VS Code")
                            .disabled(self.root.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.open_vscode(false, cx))),
                    )
                    .when(self.active() || self.fetch.is_some(), |bar| {
                        bar.child(
                            Button::new("git-stop")
                                .small()
                                .icon(gpui_kit::assets::IconName::Square)
                                .label("Stop")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(cancel) =
                                        this.action.as_ref().or(this.fetch.as_ref())
                                    {
                                        cancel.cancel();
                                    }
                                    this.status = "Cancelling Git operation...".into();
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when(tracked, |panel| {
                panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .id("git-repository-path")
                                .role(Role::Status)
                                .aria_label(root.clone())
                                .test_support()
                                .text_xs()
                                .truncate()
                                .child(root),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .flex_wrap()
                                .gap_3()
                                .child(
                                    div()
                                        .id("git-incoming")
                                        .role(Role::Status)
                                        .aria_label(format!(
                                            "{} incoming commits",
                                            snapshot.map_or(0, |snapshot| snapshot.behind)
                                        ))
                                        .test_support()
                                        .text_color(
                                            if snapshot.is_some_and(|snapshot| snapshot.behind > 0)
                                            {
                                                cx.theme().warning
                                            } else {
                                                cx.theme().muted_foreground
                                            },
                                        )
                                        .child(format!(
                                            "{} incoming",
                                            snapshot.map_or(0, |snapshot| snapshot.behind)
                                        )),
                                )
                                .child(div().text_color(cx.theme().muted_foreground).child(
                                    format!(
                                        "{} outgoing",
                                        snapshot.map_or(0, |snapshot| snapshot.ahead)
                                    ),
                                ))
                                .child(
                                    div().text_xs().child(
                                        snapshot
                                            .and_then(|snapshot| snapshot.upstream.as_deref())
                                            .unwrap_or("No upstream")
                                            .to_string(),
                                    ),
                                ),
                        )
                        .child(
                            div()
                                .id("git-remote-status")
                                .role(Role::Status)
                                .aria_label(self.remote_status())
                                .test_support()
                                .text_xs()
                                .text_color(if self.remote_error.is_some() {
                                    cx.theme().warning
                                } else {
                                    cx.theme().muted_foreground
                                })
                                .child(self.remote_status()),
                        ),
                )
            })
            .when(blocked || diverged, |panel| {
                let warning = format!(
                    "{}{}. Resolve in VS Code, then refresh Git status.",
                    operation.unwrap_or(if diverged {
                        "Branches diverged"
                    } else {
                        "Merge conflicts"
                    }),
                    if conflicts > 0 {
                        format!(" | {conflicts} conflicted file(s)")
                    } else {
                        String::new()
                    }
                );
                panel.child(
                    div()
                        .id("git-conflict-banner")
                        .role(Role::Alert)
                        .aria_label(warning.clone())
                        .test_support()
                        .px_3()
                        .py_2()
                        .bg(cx.theme().warning.opacity(0.12))
                        .child(warning),
                )
            })
            .when_some(self.error.as_ref(), |panel, error| {
                panel.child(
                    div()
                        .p_3()
                        .text_color(cx.theme().danger)
                        .child(error.clone()),
                )
            })
            .when(
                !tracked && self.snapshot.is_some() && self.error.is_none(),
                |panel| {
                    panel.child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_3()
                            .child(Icon::new(gpui_kit::assets::IconName::GitBranch).size(px(32.)))
                            .child(
                                div()
                                    .id("git-untracked-workspace")
                                    .role(Role::Status)
                                    .aria_label("Workspace is not tracked by Git")
                                    .test_support()
                                    .child("Workspace is not tracked by Git"),
                            )
                            .child(
                                Button::new("git-init")
                                    .primary()
                                    .icon(gpui_kit::assets::IconName::Plus)
                                    .label("Initialize Git")
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request(model::Action::Initialize, cx)
                                    })),
                            ),
                    )
                },
            )
            .when(
                tracked && snapshot.is_some_and(|snapshot| snapshot.remotes.is_empty()),
                |panel| {
                    panel.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .child(
                                Input::new(&self.remote_url)
                                    .id("git-remote-url")
                                    .aria_label("Origin remote URL")
                                    .small()
                                    .disabled(disabled),
                            )
                            .child(
                                Button::new("git-add-origin")
                                    .small()
                                    .icon(gpui_kit::assets::IconName::Plus)
                                    .label("Add origin")
                                    .disabled(
                                        disabled
                                            || self.remote_url.read(cx).value().trim().is_empty(),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request(
                                            model::Action::AddOrigin {
                                                url: this.remote_url.read(cx).value().trim().into(),
                                            },
                                            cx,
                                        )
                                    })),
                            ),
                    )
                },
            )
            .when(tracked, |panel| {
                panel.child(
                    div().flex_1().min_h_0().child(
                        h_resizable("git-review-layout")
                            .child(
                                resizable_panel()
                                    .size(px(310.))
                                    .size_range(px(230.)..px(540.))
                                    .child(self.render_changes(cx)),
                            )
                            .child(resizable_panel().child(self.render_review(cx))),
                    ),
                )
            })
            .when(!self.status.is_empty(), |panel| {
                panel.child(
                    div()
                        .id("git-operation-status")
                        .role(Role::Status)
                        .aria_label(self.status.clone())
                        .test_support()
                        .max_h(px(100.))
                        .overflow_y_scrollbar()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .px_3()
                        .py_2()
                        .text_sm()
                        .child(self.status.clone()),
                )
            })
    }
}
