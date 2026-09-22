use super::*;
use std::sync::{Arc, LazyLock};

static APPLICATION_LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../../../assets/icon.png").to_vec(),
    ))
});

impl Workbench {
    pub(super) fn show_about(
        &mut self,
        _: &ShowAbout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog
                .title("About")
                .child(
                    div()
                        .id("about-application")
                        .role(Role::Heading)
                        .aria_label("Regressify Desktop")
                        .test_support()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Regressify Desktop"),
                )
                .child(
                    div()
                        .id("about-version")
                        .role(Role::Status)
                        .aria_label(format!("Version: {}", env!("CARGO_PKG_VERSION")))
                        .test_support()
                        .child(format!("Version: {}", env!("CARGO_PKG_VERSION"))),
                )
                .child(
                    div()
                        .id("about-author")
                        .role(Role::Status)
                        .aria_label("Author: Tuyen Pham")
                        .test_support()
                        .child("Author: Tuyen Pham"),
                )
                .child(
                    div().flex().items_center().gap_1().child("Website:").child(
                        Button::new("about-website")
                            .ghost()
                            .small()
                            .label("https://tuyen.blog/")
                            .accessibility_label("https://tuyen.blog/")
                            .on_click(|_, _, cx| cx.open_url("https://tuyen.blog/")),
                    ),
                )
                .footer(
                    Button::new("about-close")
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
        });
    }

    pub(super) fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let root = self
            .session
            .workspace
            .as_ref()
            .map(|workspace| display_path(&workspace.root))
            .unwrap_or_else(|| "No workspace open".into());
        let counts = self.runs.read(cx).progress().counts(None);
        let workspace_label = self
            .session
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.root.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "No workspace".into());
        TitleBar::new()
            .h(px(38.))
            .bg(cx.theme().title_bar)
            .on_close_window(cx.listener(|this, _, window, cx| {
                if this.git.read(cx).active() {
                    this.git.update(cx, |panel, cx| panel.cancel_operation(cx));
                } else if this.runs.read(cx).running() {
                    this.runs.update(cx, |panel, cx| panel.cancel(cx));
                } else if !this.busy {
                    this.guard(files::Pending::Close, window, cx);
                }
            }))
            .child(
                div()
                    .id("workspace-titlebar")
                    .test_support()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .gap_2()
                    .pr_2()
                    .child(
                        div()
                            .id("application-logo")
                            .role(Role::Image)
                            .aria_label("Regressify logo")
                            .test_support()
                            .size(px(22.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                img(Arc::clone(&APPLICATION_LOGO))
                                    .size_full()
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_sm()
                            .child("Regressify"),
                    )
                    .child(
                        div()
                            .id("application-version")
                            .role(Role::Status)
                            .aria_label(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .test_support()
                            .text_xs()
                            .flex_shrink_0()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                    )
                    .child(design::titlebar_control(self.file_menu(cx)))
                    .child(design::titlebar_control({
                        let focus = self.focus.clone();
                        Button::new("help-menu")
                            .ghost()
                            .xsmall()
                            .label("Help")
                            .dropdown_menu(move |menu, _, _| {
                                menu.action_context(focus.clone())
                                    .menu("About", Box::new(ShowAbout))
                            })
                    }))
                    .child(design::titlebar_control(
                        Button::new("toggle-navigation")
                            .ghost()
                            .xsmall()
                            .icon(if self.preferences.navigator_hidden {
                                gpui_kit::assets::IconName::PanelLeftOpen
                            } else {
                                gpui_kit::assets::IconName::PanelLeftClose
                            })
                            .tooltip("Toggle navigator")
                            .accessibility_label(if self.preferences.navigator_hidden {
                                "Show navigator"
                            } else {
                                "Hide navigator"
                            })
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_navigator(&ToggleNavigator, window, cx)
                            })),
                    ))
                    .child(design::titlebar_control(
                        div().id("titlebar-navigation").test_support().child(
                            TabBar::new("workbench-tabs")
                                .underline()
                                .small()
                                .selected_index(self.main_tab)
                                .on_click(cx.listener(|this, index, window, cx| {
                                    this.select_view(*index, window, cx)
                                }))
                                .child(Tab::new().label("Editor"))
                                .child(Tab::new().label("Runs"))
                                .child(Tab::new().label("Results")),
                        ),
                    ))
                    .child(
                        div().flex_1().min_w_0().child(
                            div()
                                .id("workspace-path")
                                .role(Role::Status)
                                .aria_label(root.clone())
                                .test_support()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(workspace_label),
                        ),
                    )
                    .when(self.runs.read(cx).running(), |header| {
                        header.child(design::titlebar_control(
                            Button::new("view-running")
                                .ghost()
                                .small()
                                .icon(gpui_kit::assets::IconName::Activity)
                                .label(format!("{} / {}", counts.running, counts.complete()))
                                .tooltip("View running command")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.main_tab = 1;
                                    cx.notify();
                                })),
                        ))
                    })
                    .child(design::titlebar_control(
                        div()
                            .id("status")
                            .role(Role::Status)
                            .aria_label(self.status.clone())
                            .test_support()
                            .child(
                                Button::new("application-status")
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit::assets::IconName::Info)
                                    .accessibility_label("Application status")
                                    .tooltip(format!(
                                        "{}\n{}",
                                        self.status,
                                        if self.busy {
                                            "Working"
                                        } else if self.unsaved() {
                                            "Modified"
                                        } else {
                                            "Ready"
                                        }
                                    )),
                            ),
                    ))
                    .child(design::titlebar_control(
                        Button::new("open-commands")
                            .ghost()
                            .xsmall()
                            .icon(gpui_kit::assets::IconName::Command)
                            .accessibility_label("Commands")
                            .tooltip("Commands")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_commands(&OpenCommands, window, cx)
                            })),
                    ))
                    .child(design::titlebar_control(self.render_update_button(cx)))
                    .when(self.session.workspace.is_some(), |header| {
                        let git = self.git.read(cx);
                        header.child(design::titlebar_control(
                            Button::new("git-status")
                                .ghost()
                                .small()
                                .icon(gpui_kit::assets::IconName::GitBranch)
                                .max_w(px(180.))
                                .child(div().min_w_0().truncate().child(git.badge()))
                                .accessibility_label(format!("Git: {}", git.summary()))
                                .tooltip(git.summary())
                                .selected(self.main_tab == 3)
                                .when(git.attention(), |button| {
                                    button.text_color(cx.theme().warning)
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.select_view(3, window, cx)
                                })),
                        ))
                    })
                    .child(design::titlebar_control(self.render_deploy_controls(cx)))
                    .child(design::titlebar_control(self.render_theme_picker()))
                    .child(design::titlebar_control(
                        Button::new("open-workspace")
                            .ghost()
                            .xsmall()
                            .icon(IconName::FolderOpen)
                            .tooltip(if self.session.workspace.is_some() {
                                "Switch workspace"
                            } else {
                                "Open workspace"
                            })
                            .accessibility_label(if self.session.workspace.is_some() {
                                "Switch workspace"
                            } else {
                                "Open workspace"
                            })
                            .disabled(self.busy || self.runs.read(cx).running())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.choose_workspace(&OpenWorkspace, window, cx)
                            })),
                    )),
            )
            .into_any_element()
    }

    pub(super) fn render_overview(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(workspace) = &self.session.workspace else {
            return div()
                .flex()
                .flex_col()
                .size_full()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    Icon::new(gpui_kit::assets::IconName::Folders)
                        .size(px(48.))
                        .text_color(cx.theme().primary),
                )
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("No workspace open"),
                )
                .child(
                    Button::new("overview-open")
                        .primary()
                        .icon(IconName::FolderOpen)
                        .label("Open workspace")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.choose_workspace(&OpenWorkspace, window, cx)
                        })),
                )
                .into_any_element();
        };
        let name = workspace
            .root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let scenarios = self
            .session
            .suites
            .iter()
            .map(|suite| suite.scenarios.len())
            .sum::<usize>();
        let invalid = self
            .session
            .suites
            .iter()
            .filter(|suite| suite.error.is_some())
            .count();
        let selection_width = px(24.);
        let scenarios_width = px(90.);
        let status_width = px(100.);
        let actions_width = px(112.);
        let mut page = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .w_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("WORKSPACE"),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(name),
                            ),
                    )
                    .child(
                        Button::new("overview-new-suite")
                            .primary()
                            .icon(IconName::Plus)
                            .label("New suite")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.guard(files::Pending::Name(files::Operation::New), window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .border_y_1()
                    .border_color(cx.theme().border)
                    .py_2()
                    .child(self.overview_metric(
                        "Test suites",
                        self.session.suites.len(),
                        cx.theme().foreground,
                    ))
                    .child(self.overview_metric("Scenarios", scenarios, cx.theme().foreground))
                    .child(self.overview_metric(
                        "Selected",
                        self.runs.read(cx).selected_count(),
                        cx.theme().primary,
                    ))
                    .child(self.overview_metric(
                        "Config issues",
                        invalid,
                        if invalid == 0 {
                            cx.theme().muted_foreground
                        } else {
                            cx.theme().danger
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .justify_between()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Test suites"))
                    .child(div().flex_1())
                    .child(
                        Button::new("overview-run-reference")
                            .small()
                            .icon(gpui_kit::assets::IconName::Camera)
                            .label("Run Reference")
                            .disabled(
                                self.busy
                                    || self.runs.read(cx).selected_count() == 0
                                    || self.runs.read(cx).running(),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request_selected_command(
                                    crate::desktop::runner::RunCommand::Reference,
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("overview-run-selected")
                            .primary()
                            .icon(gpui_kit::assets::IconName::Play)
                            .label(format!(
                                "Run Test ({} scenarios)",
                                self.runs.read(cx).selected_scenario_count()
                            ))
                            .disabled(self.busy || !self.runs.read(cx).can_test_selection())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request_selected_command(
                                    crate::desktop::runner::RunCommand::Test,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .when(self.runs.read(cx).can_open_saved_selection(), |row| {
                        row.child(
                            Button::new("overview-open-report")
                                .small()
                                .icon(gpui_kit::assets::IconName::Images)
                                .label("Open Report")
                                .disabled(self.busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let names = this.runs.read(cx).selected_names();
                                    this.runs.update(cx, |panel, cx| {
                                        panel.open_saved_reports(names, cx)
                                    });
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .bg(cx.theme().muted)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().w(selection_width).flex_shrink_0())
                    .child(
                        div()
                            .id("overview-suite-header")
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .child("SUITE"),
                    )
                    .child(
                        div()
                            .id("overview-scenarios-header")
                            .test_support()
                            .w(scenarios_width)
                            .flex_shrink_0()
                            .child("SCENARIOS"),
                    )
                    .child(
                        div()
                            .id("overview-status-header")
                            .test_support()
                            .w(status_width)
                            .flex_shrink_0()
                            .child("STATUS"),
                    )
                    .child(
                        div()
                            .id("overview-actions-header")
                            .test_support()
                            .w(actions_width)
                            .flex_shrink_0(),
                    ),
            );
        let query = self.search.read(cx).value().to_lowercase();
        for (index, suite) in self.session.suites.iter().enumerate() {
            if !query.is_empty()
                && !suite.name.to_lowercase().contains(&query)
                && !suite
                    .scenarios
                    .iter()
                    .any(|scenario| scenario.label.to_lowercase().contains(&query))
            {
                continue;
            }
            let entry = suite.entry.clone();
            let name = suite.name.clone();
            let report_name = name.clone();
            let check_panel = self.runs.clone();
            let all_selected = self.runs.read(cx).all_selected(&name);
            let counts = self.runs.read(cx).progress().counts(Some(&suite.output_id));
            let status = if suite.error.is_some() {
                "Invalid"
            } else if counts.running > 0 {
                "Running"
            } else if counts.failed > 0 {
                "Failed"
            } else if counts.passed > 0 {
                "Passed"
            } else {
                "Ready"
            };
            let color = match status {
                "Invalid" | "Failed" => cx.theme().danger,
                "Running" => cx.theme().primary,
                "Passed" => cx.theme().success,
                _ => cx.theme().muted_foreground,
            };
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .pb_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_3()
                            .child(
                                div().w(selection_width).flex_shrink_0().child(
                                    design::selection_checkbox(
                                        ("overview-check", index),
                                        all_selected,
                                        self.runs.read(cx).selected(&name) && !all_selected,
                                        self.busy
                                            || self.runs.read(cx).running()
                                            || suite.error.is_some(),
                                        cx,
                                    )
                                    .accessibility_label(format!("Select suite {name}"))
                                    .on_change(
                                        move |state, _, _, cx| {
                                            check_panel.update(cx, |panel, cx| {
                                                panel.toggle_suite(
                                                    &name,
                                                    state == gpui_kit::base::CheckboxState::Checked,
                                                    cx,
                                                )
                                            })
                                        },
                                    ),
                                ),
                            )
                            .child(
                                div()
                                    .id(("overview-suite", index))
                                    .test_support()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(suite.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .truncate()
                                            .child(suite.entry.label()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                self.runs.read(cx).artifact_summary(&suite.name),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id(("overview-scenarios", index))
                                    .test_support()
                                    .w(scenarios_width)
                                    .flex_shrink_0()
                                    .child(suite.scenarios.len().to_string()),
                            )
                            .child(
                                div()
                                    .id(("overview-status", index))
                                    .test_support()
                                    .w(status_width)
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(color)
                                    .child(status),
                            )
                            .child(
                                div()
                                    .id(("overview-actions", index))
                                    .test_support()
                                    .w(actions_width)
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap_2()
                                    .when(
                                        self.runs.read(cx).can_open_saved_suite(&suite.name),
                                        |actions| {
                                            actions.child(
                                                Button::new(("overview-report", index))
                                                    .ghost()
                                                    .small()
                                                    .icon(gpui_kit::assets::IconName::Images)
                                                    .accessibility_label(format!(
                                                        "Open Report: {}",
                                                        suite.name
                                                    ))
                                                    .tooltip("Open saved report")
                                                    .disabled(self.busy)
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.runs.update(cx, |panel, cx| {
                                                                panel.open_saved_reports(
                                                                    vec![report_name.clone()],
                                                                    cx,
                                                                )
                                                            });
                                                        },
                                                    )),
                                            )
                                        },
                                    )
                                    .child(
                                        Button::new(("overview-edit", index))
                                            .ghost()
                                            .small()
                                            .icon(gpui_kit::assets::IconName::Pencil)
                                            .label("Edit")
                                            .disabled(self.busy)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.load_document(entry.clone(), window, cx)
                                            })),
                                    ),
                            ),
                    )
                    .children(suite.error.as_ref().map(|error| {
                        div()
                            .px_3()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(error.clone())
                    })),
            );
        }
        if let Some(error) = &self.session.suite_error {
            page = page.child(div().text_color(cx.theme().danger).child(error.clone()));
        }
        if self.session.suites.is_empty() {
            page = page.child(
                div()
                    .py_8()
                    .text_color(cx.theme().muted_foreground)
                    .child("No test suites"),
            );
        }
        div()
            .id("workspace-overview-scroll")
            .size_full()
            .overflow_y_scrollbar()
            .child(page)
            .into_any_element()
    }

    fn overview_metric(&self, label: &'static str, count: usize, color: Hsla) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(100.))
            .gap_1()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(count.to_string()),
            )
            .child(div().text_xs().child(label))
            .into_any_element()
    }
}
