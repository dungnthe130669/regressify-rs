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

fn review_value(id: String, value: &str, background: Hsla) -> AnyElement {
    div()
        .id(SharedString::from(id.clone()))
        .role(Role::Status)
        .aria_label(value.to_string())
        .test_support()
        .flex_1()
        .min_w_0()
        .bg(background)
        .child(
            div()
                .id(SharedString::from(format!("{id}-scroll")))
                .w_full()
                .min_w_0()
                .p_2()
                .text_sm()
                .whitespace_normal()
                .overflow_x_scrollbar()
                .child(value.to_string()),
        )
        .into_any_element()
}

impl GitPanel {
    fn select_tree(&mut self, comparison: bool, selected: bool, cx: &mut Context<Self>) {
        if self.disabled || self.active() || self.error.is_some() {
            return;
        }
        if let Some(snapshot) = &self.snapshot {
            for change in &snapshot.changes {
                if change.is_comparison() == comparison && !change.conflict && !change.submodule {
                    if selected {
                        self.selection.insert(change.path.clone());
                    } else {
                        self.selection.remove(&change.path);
                    }
                }
            }
        }
        cx.notify();
    }

    fn tree_cursor(&self, comparison: bool) -> Option<usize> {
        let rows = if comparison {
            &self.comparison_rows
        } else {
            &self.rows
        };
        self.tree_current[usize::from(comparison)]
            .as_ref()
            .or(self.active_file.as_ref())
            .and_then(|path| rows.iter().position(|row| &row.path == path))
    }

    fn activate_tree(
        &mut self,
        comparison: bool,
        toggle: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.active() || self.error.is_some() {
            return;
        }
        let rows = if comparison {
            &self.comparison_rows
        } else {
            &self.rows
        };
        let Some(row) = rows.get(self.tree_cursor(comparison).unwrap_or(0)) else {
            return;
        };
        let path = row.path.clone();
        self.tree_current[usize::from(comparison)] = Some(path.clone());
        if toggle {
            let prefix = format!("{path}/");
            let allowed = self
                .snapshot
                .as_ref()
                .into_iter()
                .flat_map(|snapshot| &snapshot.changes)
                .filter(|change| {
                    change.is_comparison() == comparison
                        && !change.conflict
                        && !change.submodule
                        && if row.file.is_some() {
                            change.path == path
                        } else {
                            change.path.starts_with(&prefix)
                        }
                })
                .map(|change| change.path.clone())
                .collect::<Vec<_>>();
            let checked = !allowed.iter().all(|path| self.selection.contains(path));
            for path in allowed {
                if checked {
                    self.selection.insert(path);
                } else {
                    self.selection.remove(&path);
                }
            }
        } else if row.file.is_some() {
            self.review(path, window, cx);
        } else {
            let key = format!("{comparison}:{path}");
            if !self.collapsed.remove(&key) {
                self.collapsed.insert(key);
            }
            self.rebuild_tree();
        }
        cx.notify();
    }

    fn tree_container(&self, comparison: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let group = usize::from(comparison);
        div()
            .id(if comparison {
                "git-comparison-files"
            } else {
                "git-files"
            })
            .key_context("RegressifyList")
            .track_focus(&self.tree_focus[group])
            .flex_1()
            .min_h_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    window.focus(&this.tree_focus[group], cx);
                }),
            )
            .on_action(cx.listener(move |this, _: &SelectAllListItems, _, cx| {
                this.select_tree(comparison, true, cx);
            }))
            .on_action(cx.listener(move |this, _: &ClearListSelection, _, cx| {
                this.select_tree(comparison, false, cx);
            }))
            .on_action(
                cx.listener(move |this, action: &lists::MoveListSelection, _, cx| {
                    if this.disabled || this.active() || this.error.is_some() {
                        return;
                    }
                    let rows = if comparison {
                        &this.comparison_rows
                    } else {
                        &this.rows
                    };
                    if let Some(index) = action.index(this.tree_cursor(comparison), rows.len()) {
                        this.tree_current[group] = Some(rows[index].path.clone());
                        this.tree_scroll[group].scroll_to_item(index, ScrollStrategy::Nearest);
                        cx.notify();
                    }
                }),
            )
            .on_action(cx.listener(move |this, _: &ToggleListItem, window, cx| {
                this.activate_tree(comparison, true, window, cx);
            }))
            .on_action(cx.listener(move |this, _: &OpenListItem, window, cx| {
                this.activate_tree(comparison, false, window, cx);
            }))
    }

    pub(super) fn rebuild_tree(&mut self) {
        let mut rows = Vec::new();
        let mut comparison_rows = Vec::new();
        if let Some(snapshot) = &self.snapshot {
            fn flatten(
                tree: &model::ChangeTree,
                prefix: &str,
                depth: usize,
                snapshot: &Snapshot,
                collapsed: &BTreeSet<String>,
                comparison: bool,
                rows: &mut Vec<TreeRow>,
            ) {
                for (label, child) in &tree.folders {
                    let path = format!("{prefix}{label}");
                    let start = rows.len();
                    rows.push(TreeRow {
                        path: path.clone(),
                        label: visible_name(label),
                        depth,
                        file: None,
                    });
                    flatten(
                        child,
                        &format!("{path}/"),
                        depth + 1,
                        snapshot,
                        collapsed,
                        comparison,
                        rows,
                    );
                    if rows.len() == start + 1 {
                        rows.pop();
                    } else if collapsed.contains(&format!("{comparison}:{path}")) {
                        rows.truncate(start + 1);
                    }
                }
                for index in &tree.files {
                    let change = &snapshot.changes[*index];
                    if change.is_comparison() != comparison {
                        continue;
                    }
                    rows.push(TreeRow {
                        path: change.path.clone(),
                        label: visible_name(change.path.rsplit('/').next().unwrap_or(&change.path)),
                        depth,
                        file: Some(*index),
                    });
                }
            }
            let tree = model::ChangeTree::new(&snapshot.changes);
            for (comparison, rows) in [(false, &mut rows), (true, &mut comparison_rows)] {
                flatten(&tree, "", 0, snapshot, &self.collapsed, comparison, rows);
            }
        }
        self.rows = rows;
        self.comparison_rows = comparison_rows;
    }

    fn tree_row(&self, index: usize, comparison: bool, cx: &mut Context<Self>) -> AnyElement {
        let row = if comparison {
            &self.comparison_rows[index]
        } else {
            &self.rows[index]
        };
        let path = row.path.clone();
        let focus_path = path.clone();
        let label = row.label.clone();
        let mut element = div()
            .id((
                if comparison {
                    "git-comparison-tree-row"
                } else {
                    "git-tree-row"
                },
                index,
            ))
            .test_support()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.tree_current[usize::from(comparison)] = Some(focus_path.clone());
                    cx.notify();
                }),
            )
            .flex()
            .items_center()
            .h(px(30.))
            .w_full()
            .min_w_0()
            .border_l_2()
            .border_color(
                if self.tree_current[usize::from(comparison)].as_ref() == Some(&path) {
                    cx.theme().primary
                } else {
                    cx.theme().transparent
                },
            )
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
                        .child(div().flex_1().min_w_0().text_left().truncate().child(label))
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
            let collapse_key = format!("{comparison}:{path}");
            let collapsed = self.collapsed.contains(&collapse_key);
            let prefix = format!("{path}/");
            let allowed = self
                .snapshot
                .as_ref()
                .expect("tree requires snapshot")
                .changes
                .iter()
                .filter(|change| {
                    change.is_comparison() == comparison
                        && change.path.starts_with(&prefix)
                        && !change.conflict
                        && !change.submodule
                })
                .map(|change| change.path.clone())
                .collect::<Vec<_>>();
            let selected = allowed
                .iter()
                .filter(|path| self.selection.contains(*path))
                .count();
            let panel = cx.entity();
            element = element.child(
                design::selection_checkbox(
                    SharedString::from(format!(
                        "{}:{path}",
                        if comparison {
                            "git-select-comparison-folder"
                        } else {
                            "git-select-folder"
                        }
                    )),
                    !allowed.is_empty() && selected == allowed.len(),
                    selected > 0 && selected < allowed.len(),
                    self.disabled || self.active() || self.error.is_some() || allowed.is_empty(),
                    cx,
                )
                .accessibility_label(format!("Select folder {}", visible_name(&path)))
                .on_change(move |state, _, _, cx| {
                    panel.update(cx, |this, cx| {
                        if state == gpui_kit::base::CheckboxState::Checked {
                            this.selection.extend(allowed.iter().cloned());
                        } else {
                            for path in &allowed {
                                this.selection.remove(path);
                            }
                        }
                        cx.notify();
                    })
                }),
            );
            element = element.child(
                Button::new(SharedString::from(format!(
                    "{}:{path}",
                    if comparison {
                        "git-comparison-folder"
                    } else {
                        "git-folder"
                    }
                )))
                .ghost()
                .xsmall()
                .flex_1()
                .min_w_0()
                .justify_start()
                .icon(if collapsed {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                })
                .accessibility_label(format!("Folder {}", visible_name(&path)))
                .tooltip(visible_name(&path))
                .child(Icon::new(gpui_kit::assets::IconName::Folder).size_3())
                .child(div().flex_1().min_w_0().text_left().truncate().child(label))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.collapsed.remove(&collapse_key) {
                        this.collapsed.insert(collapse_key.clone());
                    }
                    this.rebuild_tree();
                    cx.notify();
                })),
            );
        }
        element.into_any_element()
    }

    fn render_changes(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self.snapshot.as_ref().map_or(0, |snapshot| {
            snapshot
                .changes
                .iter()
                .filter(|change| !change.is_comparison())
                .count()
        });
        let allowed = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .changes
                    .iter()
                    .filter(|change| {
                        !change.is_comparison() && !change.conflict && !change.submodule
                    })
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
        let discard_blocked = self
            .snapshot
            .as_ref()
            .is_none_or(|snapshot| snapshot.conflicts() > 0 || snapshot.operation.is_some());
        div()
            .id("git-changes-panel")
            .role(Role::Group)
            .aria_label("Git changes")
            .test_support()
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
                                    for path in &allowed {
                                        this.selection.remove(path);
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                self.tree_container(false, cx)
                    .when(count == 0, |tree| {
                        tree.child(
                            div()
                                .p_3()
                                .text_color(cx.theme().muted_foreground)
                                .child("No file changes"),
                        )
                    })
                    .when(count > 0, |tree| {
                        tree.child(
                            uniform_list(
                                "git-change-tree",
                                self.rows.len(),
                                cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .map(|index| this.tree_row(index, false, cx))
                                        .collect::<Vec<_>>()
                                }),
                            )
                            .track_scroll(&self.tree_scroll[0])
                            .size_full(),
                        )
                    }),
            )
            .child(self.render_comparison_changes(cx))
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
                        div()
                            .id("git-commit-message")
                            .test_support()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    if !this.disabled && !this.active() {
                                        window.focus(&this.message.read(cx).focus_handle(cx), cx);
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    }
                                }),
                            )
                            .child(
                                Textarea::new(&self.message)
                                    .aria_label("Git commit message")
                                    .h(px(80.))
                                    .disabled(disabled),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("git-commit")
                                    .small()
                                    .primary()
                                    .icon(gpui_kit::assets::IconName::GitCommitHorizontal)
                                    .label("Commit selected files")
                                    .disabled(
                                        disabled
                                            || blocked
                                            || message_empty
                                            || self.selection.is_empty(),
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
                            )
                            .child(
                                Button::new("git-discard")
                                    .small()
                                    .danger()
                                    .icon(gpui_kit::assets::IconName::Trash)
                                    .label("Discard selected files")
                                    .disabled(
                                        disabled || discard_blocked || self.selection.is_empty(),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request(
                                            model::Action::Discard {
                                                paths: this.selection.iter().cloned().collect(),
                                            },
                                            cx,
                                        );
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_comparison_changes(&self, cx: &mut Context<Self>) -> AnyElement {
        let changes = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .changes
                    .iter()
                    .filter(|change| change.is_comparison())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let selected = changes
            .iter()
            .filter(|change| self.selection.contains(&change.path))
            .count();
        let allowed = changes
            .iter()
            .filter(|change| !change.conflict && !change.submodule)
            .map(|change| change.path.clone())
            .collect::<Vec<_>>();
        let all = !allowed.is_empty() && allowed.iter().all(|path| self.selection.contains(path));
        div()
            .id("git-comparison-changes")
            .test_support()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .when(self.comparison_open, |panel| panel.flex_1())
            .when(!self.comparison_open, |panel| panel.flex_shrink_0())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .h(px(38.))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("git-toggle-comparison")
                            .ghost()
                            .xsmall()
                            .flex_1()
                            .min_w_0()
                            .justify_start()
                            .icon(if self.comparison_open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_left()
                                    .truncate()
                                    .child(format!("Comparison Changes ({})", changes.len())),
                            )
                            .tooltip(format!(
                                "Comparison Changes: {} files, {selected} selected",
                                changes.len()
                            ))
                            .accessibility_label("Comparison Changes")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.comparison_open = !this.comparison_open;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("git-comparison-selected")
                            .role(Role::Status)
                            .aria_label(format!("{selected} comparison files selected"))
                            .test_support()
                            .text_xs()
                            .flex_shrink_0()
                            .child(if selected > 0 {
                                selected.to_string()
                            } else {
                                String::new()
                            }),
                    )
                    .when(self.comparison_open, |header| {
                        header.child(
                            Checkbox::new("git-select-all-comparison")
                                .label("All")
                                .checked(all)
                                .disabled(
                                    self.disabled
                                        || self.active()
                                        || self.error.is_some()
                                        || allowed.is_empty(),
                                )
                                .on_click(cx.listener(move |this, checked, _, cx| {
                                    for path in &allowed {
                                        if *checked {
                                            this.selection.insert(path.clone());
                                        } else {
                                            this.selection.remove(path);
                                        }
                                    }
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when(self.comparison_open, |panel| {
                panel.child(
                    self.tree_container(true, cx)
                        .when(changes.is_empty(), |tree| {
                            tree.child(div().p_3().child("No comparison changes"))
                        })
                        .when(!changes.is_empty(), |tree| {
                            tree.child(
                                uniform_list(
                                    "git-comparison-tree",
                                    self.comparison_rows.len(),
                                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                        range
                                            .map(|index| this.tree_row(index, true, cx))
                                            .collect::<Vec<_>>()
                                    }),
                                )
                                .track_scroll(&self.tree_scroll[1])
                                .size_full(),
                            )
                        }),
                )
            })
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
                                .unwrap_or_else(|| "Review changes".into()),
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
            .when(self.active_file.is_some(), |review| {
                review.child(
                    div()
                        .px_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            TabBar::new("git-review-tabs")
                                .underline()
                                .small()
                                .selected_index(usize::from(self.source_diff))
                                .on_click(cx.listener(|this, index, _, cx| {
                                    this.source_diff = *index == 1;
                                    cx.notify();
                                }))
                                .child(Tab::new().label("Changes"))
                                .child(Tab::new().label("Source Diff")),
                        ),
                )
            })
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
                        if !self.source_diff && self.structured.as_ref().is_some_and(Result::is_ok)
                        {
                            return content.child(self.render_structured_review(cx));
                        }
                        content
                            .flex()
                            .flex_col()
                            .when(!self.source_diff, |content| {
                                content.child(
                                    div()
                                        .id("git-review-unavailable")
                                        .role(Role::Status)
                                        .test_support()
                                        .px_3()
                                        .py_2()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            self.structured
                                                .as_ref()
                                                .and_then(|result| result.as_ref().err())
                                                .cloned()
                                                .map(|error| format!("Source Diff: {error}"))
                                                .unwrap_or_else(|| "Loading changes...".into()),
                                        ),
                                )
                            })
                            .child(
                                div().flex_1().min_h_0().child(
                                    CodeEditor::new(&self.diff)
                                        .aria_label("Git diff")
                                        .readonly(true)
                                        .size_full(),
                                ),
                            )
                    }),
            )
            .into_any_element()
    }

    fn render_structured_review(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(Ok(review)) = &self.structured else {
            return div().into_any_element();
        };
        let fields = review
            .groups
            .iter()
            .map(|group| group.fields.len())
            .sum::<usize>();
        let summary = if review.groups.is_empty() {
            if review.source_only {
                "Only comments or formatting changed".into()
            } else {
                "No configuration changes".into()
            }
        } else {
            format!(
                "{} changed groups / {fields} changes / {} unchanged items",
                review.groups.len(),
                review.unchanged
            )
        };
        div()
            .id("git-structured-review")
            .test_support()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .id("git-review-summary")
                            .role(Role::Status)
                            .aria_label(summary.clone())
                            .test_support()
                            .text_sm()
                            .child(summary),
                    )
                    .when_some(review.renamed_from.as_ref(), |header, path| {
                        header.child(
                            div()
                                .text_sm()
                                .whitespace_normal()
                                .child(format!("Renamed from {}", visible_name(path))),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .mt_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Before / Last commit"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("After / Current saved version"),
                            ),
                    ),
            )
            .child(
                div()
                    .id("git-review-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .children(review.groups.iter().enumerate().map(|(index, group)| {
                        let collapsed = self.collapsed_review.contains(&index);
                        let color = match group.kind {
                            model::review::ChangeKind::Added => cx.theme().success,
                            model::review::ChangeKind::Removed => cx.theme().danger,
                            model::review::ChangeKind::Changed => cx.theme().warning,
                            model::review::ChangeKind::Moved => cx.theme().primary,
                        };
                        div()
                            .id(("git-review-group", index))
                            .test_support()
                            .w_full()
                            .min_w_0()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .py_2()
                                    .bg(cx.theme().muted.opacity(0.35))
                                    .child(
                                        Button::new(("git-review-toggle", index))
                                            .ghost()
                                            .xsmall()
                                            .min_w_0()
                                            .flex_1()
                                            .justify_start()
                                            .icon(if collapsed {
                                                IconName::ChevronRight
                                            } else {
                                                IconName::ChevronDown
                                            })
                                            .accessibility_label(group.title.clone())
                                            .tooltip(group.title.clone())
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_left()
                                                    .truncate()
                                                    .child(group.title.clone()),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if !this.collapsed_review.remove(&index) {
                                                    this.collapsed_review.insert(index);
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .flex_shrink_0()
                                            .text_color(color)
                                            .child(group.kind.label()),
                                    ),
                            )
                            .when(!collapsed, |section| {
                                section.children(group.fields.iter().enumerate().map(
                                    |(field_index, field)| {
                                        div()
                                            .id(SharedString::from(format!(
                                                "git-field-{index}-{field_index}"
                                            )))
                                            .test_support()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .px_3()
                                            .py_2()
                                            .min_w_0()
                                            .w_full()
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_2()
                                                    .min_w_0()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .whitespace_normal()
                                                            .child(field.label.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .flex_shrink_0()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(field.kind.label()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_3()
                                                    .min_w_0()
                                                    .child(review_value(
                                                        format!("git-before-{index}-{field_index}"),
                                                        &field.before,
                                                        cx.theme().danger.opacity(0.07),
                                                    ))
                                                    .child(review_value(
                                                        format!("git-after-{index}-{field_index}"),
                                                        &field.after,
                                                        cx.theme().success.opacity(0.07),
                                                    )),
                                            )
                                    },
                                ))
                            })
                    })),
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
                        .h(px(40.))
                        .flex_shrink_0()
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
