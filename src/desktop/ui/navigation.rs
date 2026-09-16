//! Separate suite selection from the open document's scenario outline.

use gpui_kit::component::resizable::v_resizable;

use super::*;

fn empty_list(id: &'static str, text: &'static str, cx: &App) -> AnyElement {
    div()
        .id(id)
        .role(Role::Status)
        .aria_label(text)
        .test_support()
        .px_3()
        .py_5()
        .text_sm()
        .text_center()
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

impl Workbench {
    pub(super) fn render_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut navigation = div()
            .id("navigator")
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .bg(cx.theme().muted.opacity(0.4))
            .child(
                div().px_2().py_2().child(
                    Button::new("workspace-overview")
                        .ghost()
                        .small()
                        .w_full()
                        .justify_start()
                        .icon(gpui_kit::assets::IconName::LayoutDashboard)
                        .label("Overview")
                        .selected(self.overview && self.main_tab == 0)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.overview = true;
                            this.main_tab = 0;
                            cx.notify();
                        })),
                ),
            );
        if self.session.workspace.is_none() {
            navigation = navigation.child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("RECENT WORKSPACES"),
            );
            for (index, path) in self.preferences.recent_workspaces.iter().enumerate() {
                let path = path.clone();
                let label = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                navigation = navigation.child(
                    Button::new(("recent", index))
                        .ghost()
                        .justify_start()
                        .w_full()
                        .min_w_0()
                        .accessibility_label(label.clone())
                        .tooltip(display_path(&path))
                        .child(div().flex_1().min_w_0().truncate().text_left().child(label))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.load_workspace(path.clone(), window, cx)
                        })),
                );
            }
            return navigation.into_any_element();
        }
        navigation
            .child(
                div().flex_1().min_h_0().child(
                    v_resizable("navigation-panes")
                        .child(
                            resizable_panel()
                                .size(px(280.))
                                .size_range(px(150.)..px(800.))
                                .child(self.render_suite_list(cx)),
                        )
                        .child(
                            resizable_panel()
                                .size_range(px(120.)..px(1200.))
                                .child(self.render_scenario_list(cx)),
                        ),
                ),
            )
            .child(self.render_workspace_files(cx))
            .into_any_element()
    }

    fn render_suite_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let runs = self.runs.read(cx);
        let valid = self
            .session
            .suites
            .iter()
            .filter(|suite| suite.error.is_none())
            .count();
        let mut rows = div()
            .id("suite-list")
            .test_support()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        let mut count = 0;
        for (index, entry) in self
            .session
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.kind == DocumentKind::Suite)
        {
            let current = self
                .session
                .open
                .as_ref()
                .is_some_and(|open| open.entry.path == entry.path);
            let suite = self
                .session
                .suites
                .iter()
                .find(|suite| suite.entry.path == entry.path);
            if self.selected_only && !suite.is_some_and(|suite| runs.selected(&suite.name)) {
                continue;
            }
            let matches = if current {
                self.value["scenarios"]
                    .as_array()
                    .map(|scenarios| {
                        scenarios
                            .iter()
                            .filter(|scenario| {
                                ["label", "id", "url"].iter().any(|key| {
                                    scenario[*key]
                                        .as_str()
                                        .is_some_and(|text| text.to_lowercase().contains(&query))
                                })
                            })
                            .count()
                    })
                    .unwrap_or(0)
            } else {
                suite
                    .map(|suite| {
                        suite
                            .scenarios
                            .iter()
                            .filter(|scenario| {
                                [&scenario.label, &scenario.id, &scenario.url]
                                    .iter()
                                    .any(|text| text.to_lowercase().contains(&query))
                            })
                            .count()
                    })
                    .unwrap_or(0)
            };
            if !query.is_empty() && !entry.label().to_lowercase().contains(&query) && matches == 0 {
                continue;
            }
            count += 1;
            let scenario_count = if current {
                self.value["scenarios"].as_array().map_or(0, Vec::len)
            } else {
                suite.map_or(0, |suite| suite.scenarios.len())
            };
            let error = suite.and_then(|suite| suite.error.clone());
            let detail = if error.is_some() {
                "Configuration error".into()
            } else if !query.is_empty() && matches > 0 {
                format!("{matches} matching scenarios")
            } else {
                format!("{scenario_count} scenarios")
            };
            let name = suite
                .map(|suite| suite.name.clone())
                .or_else(|| entry.suite_name())
                .unwrap_or_default();
            let selected = runs.selected(&name);
            let entry = entry.clone();
            let filename = entry
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let counts = suite.map(|suite| runs.progress().counts(Some(&suite.output_id)));
            let status_color =
                if error.is_some() || counts.as_ref().is_some_and(|counts| counts.failed > 0) {
                    cx.theme().danger
                } else if counts.as_ref().is_some_and(|counts| counts.running > 0) {
                    cx.theme().primary
                } else if counts.as_ref().is_some_and(|counts| counts.passed > 0) {
                    cx.theme().success
                } else {
                    cx.theme().muted_foreground
                };
            rows = rows.child(
                div()
                    .id(("suite-row", index))
                    .test_support()
                    .flex()
                    .items_center()
                    .w_full()
                    .h(px(52.))
                    .flex_shrink_0()
                    .min_w_0()
                    .border_l_2()
                    .border_color(if current {
                        cx.theme().primary
                    } else {
                        cx.theme().transparent
                    })
                    .when(current, |row| row.bg(cx.theme().accent))
                    .child(
                        div()
                            .w(px(30.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Checkbox::new(("sidebar-suite", index))
                                    .label("")
                                    .checked(selected)
                                    .disabled(
                                        self.busy
                                            || runs.running()
                                            || suite.is_none()
                                            || error.is_some(),
                                    )
                                    .on_change(cx.listener(move |this, checked, _, cx| {
                                        this.runs.update(cx, |panel, cx| {
                                            panel.toggle_suite(&name, *checked, cx)
                                        })
                                    })),
                            ),
                    )
                    .child(
                        Button::new(("file", index))
                            .ghost()
                            .h_full()
                            .flex_1()
                            .min_w_0()
                            .justify_start()
                            .selected(current)
                            .accessibility_label(filename)
                            .tooltip(error.map_or_else(
                                || entry.label(),
                                |error| format!("{}\n{error}", entry.label()),
                            ))
                            .child(
                                Icon::new(gpui_kit::assets::IconName::Files)
                                    .small()
                                    .text_color(status_color),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .flex_1()
                                    .min_w_0()
                                    .text_left()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(if current {
                                                FontWeight::SEMIBOLD
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .child(
                                                entry.suite_name().unwrap_or_else(|| entry.label()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .truncate()
                                            .text_color(status_color)
                                            .child(detail),
                                    ),
                            )
                            .disabled(self.busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_location(entry.clone(), Selection::Root, window, cx)
                            })),
                    ),
            );
        }
        if count == 0 {
            rows = rows.child(empty_list("navigator-empty", "No matches", cx));
        }
        div()
            .id("suite-pane")
            .test_support()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .h(px(36.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_sm()
                            .child("Test suites"),
                    )
                    .child(
                        div()
                            .id("suite-count")
                            .role(Role::Status)
                            .aria_label(format!(
                                "{count} {}",
                                if count == 1 { "suite" } else { "suites" }
                            ))
                            .test_support()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(count.to_string()),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("navigator-new-suite")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip("New suite")
                            .accessibility_label("New suite")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.guard(files::Pending::Name(files::Operation::New), window, cx)
                            })),
                    ),
            )
            .child(
                div().px_3().pb_2().child(
                    Input::new(&self.search)
                        .id("navigator-search")
                        .prefix(IconName::Search)
                        .cleanable(true)
                        .small(),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .pb_2()
                    .child(
                        TabBar::new("navigator-filter")
                            .segmented()
                            .small()
                            .selected_index(usize::from(self.selected_only))
                            .on_click(cx.listener(|this, index, _, cx| {
                                this.selected_only = *index == 1;
                                cx.notify();
                            }))
                            .child(Tab::new().label("All"))
                            .child(Tab::new().label("Selected")),
                    )
                    .child(div().flex_1())
                    .child(
                        Checkbox::new("sidebar-select-all")
                            .label("All")
                            .checked(valid > 0 && runs.selected_count() == valid)
                            .disabled(self.busy || runs.running() || valid == 0)
                            .on_change(cx.listener(|this, checked, _, cx| {
                                this.runs
                                    .update(cx, |panel, cx| panel.select_all(*checked, cx))
                            })),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_scenario_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.session.open.as_ref();
        let viewport = open.is_some_and(|open| open.entry.kind == DocumentKind::Viewports);
        let suite = open.is_some_and(|open| open.entry.kind == DocumentKind::Suite);
        let items = if suite {
            self.value["scenarios"].as_array()
        } else if viewport {
            self.value.as_array()
        } else {
            None
        };
        let items = items.map(Vec::as_slice).unwrap_or_default();
        let query = self.scenario_search.read(cx).value().trim().to_lowercase();
        let owner = open
            .filter(|_| suite || viewport)
            .map(|open| {
                open.entry
                    .suite_name()
                    .unwrap_or_else(|| open.entry.label())
            })
            .unwrap_or_else(|| "No suite selected".into());
        let mut rows = div()
            .id("scenario-list")
            .test_support()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        let mut count = 0;
        for (index, item) in items.iter().enumerate() {
            let label = item["label"]
                .as_str()
                .filter(|label| !label.trim().is_empty())
                .or_else(|| item["url"].as_str().filter(|url| !url.is_empty()))
                .map(str::to_string)
                .unwrap_or_else(|| {
                    format!(
                        "{} {}",
                        if viewport { "Viewport" } else { "Scenario" },
                        index + 1
                    )
                });
            let url = item["url"].as_str().unwrap_or_default();
            if !query.is_empty()
                && ![label.as_str(), url, item["id"].as_str().unwrap_or_default()]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&query))
            {
                continue;
            }
            count += 1;
            let selection = if viewport {
                Selection::Viewport(index)
            } else {
                Selection::Scenario(index)
            };
            let selected = self.session.selection == selection
                || (!viewport && self.session.selection.scenario() == Some(index));
            let detail = if viewport {
                format!("{} x {}", item["width"], item["height"])
            } else {
                url.to_string()
            };
            rows = rows.child(
                Button::new(("scenario", index))
                    .ghost()
                    .h(px(54.))
                    .flex_shrink_0()
                    .w_full()
                    .min_w_0()
                    .justify_start()
                    .selected(selected)
                    .accessibility_label(label.clone())
                    .tooltip(format!(
                        "{label}\n{detail}\n{}",
                        item["id"].as_str().unwrap_or_default()
                    ))
                    .child(
                        div()
                            .w(px(22.))
                            .flex_shrink_0()
                            .text_center()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child((index + 1).to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .text_left()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(if selected {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(detail),
                            ),
                    )
                    .disabled(self.busy)
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.select(selection, window, cx)),
                    ),
            );
        }
        if count == 0 {
            rows = rows.child(empty_list(
                "scenario-empty",
                if suite || viewport {
                    "No matching items"
                } else {
                    "Select a test suite"
                },
                cx,
            ));
        }
        div()
            .id("scenario-pane")
            .test_support()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .h(px(36.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_sm()
                            .child(if viewport { "Viewports" } else { "Scenarios" }),
                    )
                    .child(
                        div()
                            .id("scenario-count")
                            .role(Role::Status)
                            .aria_label(format!("{count} of {}", items.len()))
                            .test_support()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{count}/{}", items.len())),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("navigator-add-scenario")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip(if viewport {
                                "Add viewport"
                            } else {
                                "Add scenario"
                            })
                            .disabled(
                                self.busy || !(suite || viewport) || !self.field_errors.is_empty(),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if viewport {
                                    this.edit(Session::add_viewport, window, cx);
                                } else {
                                    this.edit(Session::add_scenario, window, cx);
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .id("scenario-suite")
                    .role(Role::Status)
                    .aria_label(owner.clone())
                    .test_support()
                    .px_3()
                    .pb_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(owner),
            )
            .child(
                div().px_3().pb_2().child(
                    Input::new(&self.scenario_search)
                        .id("scenario-search")
                        .prefix(IconName::Search)
                        .cleanable(true)
                        .small()
                        .disabled(!(suite || viewport)),
                ),
            )
            .child(rows)
            .into_any_element()
    }

    fn render_workspace_files(&self, cx: &mut Context<Self>) -> AnyElement {
        let entries = self
            .session
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.kind != DocumentKind::Suite)
            .collect::<Vec<_>>();
        let mut section = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("workspace-files-toggle")
                    .ghost()
                    .small()
                    .w_full()
                    .justify_start()
                    .icon(if self.workspace_files_open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .label(format!("Workspace files ({})", entries.len()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.workspace_files_open = !this.workspace_files_open;
                        cx.notify();
                    })),
            );
        if self.workspace_files_open {
            let mut files = div()
                .id("workspace-files")
                .flex()
                .flex_col()
                .max_h(px(96.))
                .overflow_y_scroll();
            for (index, entry) in entries {
                let entry = entry.clone();
                files = files.child(
                    Button::new(("file", index))
                        .ghost()
                        .small()
                        .w_full()
                        .min_w_0()
                        .justify_start()
                        .selected(
                            self.session
                                .open
                                .as_ref()
                                .is_some_and(|open| open.entry.path == entry.path),
                        )
                        .icon(gpui_kit::assets::IconName::FileCog)
                        .accessibility_label(entry.label())
                        .tooltip(entry.label())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_left()
                                .child(entry.label()),
                        )
                        .disabled(self.busy)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_location(entry.clone(), Selection::Root, window, cx)
                        })),
                );
            }
            section = section.child(files);
        }
        section.into_any_element()
    }
}
