//! Search existing workbench actions without creating another execution path.

use gpui_kit::component::command::{Command as KitCommand, CommandItem, CommandState};
use std::{cell::Cell, rc::Rc};

use super::*;
use crate::desktop::workspace::ThemePreference;

struct Command {
    title: &'static str,
    group: &'static str,
    icon: gpui_kit::assets::IconName,
    action: Box<dyn Action>,
    enabled: bool,
}

impl Workbench {
    pub(super) fn open_commands(
        &mut self,
        _: &OpenCommands,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        use gpui_kit::assets::IconName as Icon;
        let workspace = self.session.workspace.is_some();
        let open = self.session.open.as_ref();
        let suite = open.is_some_and(|open| open.entry.kind == DocumentKind::Suite);
        let scenario = self.session.selection.scenario();
        let editable = self.field_errors.is_empty() && self.diagnostic.is_none();
        let mut commands = vec![
            Command {
                title: "Landing apps",
                group: "Application",
                icon: Icon::Globe,
                action: Box::new(OpenLandingApps),
                enabled: !self.runs.read(cx).running(),
            },
            Command {
                title: "Deployment tokens",
                group: "Application",
                icon: Icon::Settings2,
                action: Box::new(OpenDeploymentTokens),
                enabled: !self.runs.read(cx).running(),
            },
            Command {
                title: "Git changes and remote status",
                group: "Workspace",
                icon: Icon::GitBranch,
                action: Box::new(OpenGit),
                enabled: workspace,
            },
            Command {
                title: "Deploy selected reports",
                group: "Workspace",
                icon: Icon::CloudUpload,
                action: Box::new(OpenDeployment),
                enabled: workspace
                    && !self.runs.read(cx).running()
                    && self.runs.read(cx).selected_count() > 0,
            },
            Command {
                title: "Deployment targets",
                group: "Workspace",
                icon: Icon::Settings2,
                action: Box::new(OpenDeployment),
                enabled: workspace && !self.runs.read(cx).running(),
            },
            Command {
                title: "Run Test for current scenario",
                group: "Run",
                icon: Icon::Play,
                action: Box::new(RunScenarioCommand {
                    index: 0,
                    scenario: scenario.unwrap_or(0),
                }),
                enabled: suite && scenario.is_some() && self.can_test_open_suite(cx),
            },
            Command {
                title: "Run Reference for current scenario",
                group: "Run",
                icon: Icon::Camera,
                action: Box::new(RunScenarioCommand {
                    index: 1,
                    scenario: scenario.unwrap_or(0),
                }),
                enabled: suite && scenario.is_some() && !self.runs.read(cx).running(),
            },
            Command {
                title: "Compare Results for current scenario",
                group: "Run",
                icon: Icon::Images,
                action: Box::new(RunScenarioCommand {
                    index: 2,
                    scenario: scenario.unwrap_or(0),
                }),
                enabled: suite && scenario.is_some() && !self.runs.read(cx).running(),
            },
            Command {
                title: "Approve Results for current scenario",
                group: "Run",
                icon: Icon::Check,
                action: Box::new(RunScenarioCommand {
                    index: 3,
                    scenario: scenario.unwrap_or(0),
                }),
                enabled: suite && scenario.is_some() && !self.runs.read(cx).running(),
            },
            Command {
                title: "Run Test for open suite",
                group: "Run",
                icon: Icon::Play,
                action: Box::new(RunSuiteCommand { index: 0 }),
                enabled: suite && self.can_test_open_suite(cx),
            },
            Command {
                title: "Run Reference for open suite",
                group: "Run",
                icon: Icon::Camera,
                action: Box::new(RunSuiteCommand { index: 1 }),
                enabled: suite && !self.runs.read(cx).running(),
            },
            Command {
                title: "Approve Results for open suite",
                group: "Run",
                icon: Icon::Check,
                action: Box::new(RunSuiteCommand { index: 3 }),
                enabled: suite && !self.runs.read(cx).running(),
            },
            Command {
                title: "Save document",
                group: "File",
                icon: Icon::Save,
                action: Box::new(SaveDocument),
                enabled: open.is_some() && editable && self.unsaved(cx),
            },
            Command {
                title: "Undo document edit",
                group: "Edit",
                icon: Icon::Undo,
                action: Box::new(UndoDocument),
                enabled: open.is_some_and(|open| open.document.can_undo()),
            },
            Command {
                title: "Redo document edit",
                group: "Edit",
                icon: Icon::Redo,
                action: Box::new(RedoDocument),
                enabled: open.is_some_and(|open| open.document.can_redo()),
            },
            Command {
                title: "Open workspace",
                group: "Workspace",
                icon: Icon::FolderOpen,
                action: Box::new(OpenWorkspace),
                enabled: !self.runs.read(cx).running(),
            },
            Command {
                title: "Find suite",
                group: "Navigation",
                icon: Icon::Search,
                action: Box::new(FocusNavigator),
                enabled: workspace,
            },
            Command {
                title: "Find setting",
                group: "Navigation",
                icon: Icon::Settings,
                action: Box::new(FocusSettings),
                enabled: open.is_some(),
            },
            Command {
                title: "Toggle navigator",
                group: "View",
                icon: Icon::PanelLeft,
                action: Box::new(ToggleNavigator),
                enabled: true,
            },
            Command {
                title: "Show editor",
                group: "View",
                icon: Icon::FileCode,
                action: Box::new(SelectView { index: 0 }),
                enabled: true,
            },
            Command {
                title: "Show runs",
                group: "View",
                icon: Icon::Activity,
                action: Box::new(SelectView { index: 1 }),
                enabled: workspace,
            },
            Command {
                title: "Show results",
                group: "View",
                icon: Icon::Images,
                action: Box::new(SelectView { index: 2 }),
                enabled: workspace,
            },
            Command {
                title: "Add scenario",
                group: "Edit",
                icon: Icon::Plus,
                action: Box::new(AddScenario),
                enabled: suite && editable,
            },
        ];
        for (title, command) in files::menu_entries() {
            use files::Operation;
            if command.operation == Operation::Theme {
                continue;
            }
            let enabled = self.file_operation_enabled(command.operation, cx);
            commands.push(Command {
                title,
                group: "File",
                icon: Icon::FileCog,
                action: Box::new(command),
                enabled,
            });
        }
        for (title, preference, icon) in [
            ("System theme", ThemePreference::System, Icon::Monitor),
            ("Light theme", ThemePreference::Light, Icon::Sun),
            ("Dark theme", ThemePreference::Dark, Icon::Moon),
        ] {
            commands.push(Command {
                title,
                group: "Appearance",
                icon,
                action: Box::new(design::ChooseTheme { preference }),
                enabled: true,
            });
        }
        let commands = Rc::new(commands);
        let focus = self.focus.clone();
        let state = cx.new(|cx| CommandState::new(window, cx));
        let content = state.clone();
        let confirmed = Rc::new(Cell::new(false));
        window.open_dialog(cx, move |dialog, _, _| {
            let items = commands.iter().map(|command| {
                let title = command.title;
                let group = command.group;
                let icon = command.icon;
                CommandItem::new()
                    .label(title)
                    .keywords([group])
                    .disabled(!command.enabled)
                    .child(move |_, cx| {
                        div()
                            .id(SharedString::from(format!("command:{title}")))
                            .role(Role::ListBoxOption)
                            .aria_label(title)
                            .test_support()
                            .flex()
                            .items_center()
                            .gap_3()
                            .w_full()
                            .min_w_0()
                            .child(gpui_kit::component::Icon::new(icon).small())
                            .child(div().flex_1().min_w_0().truncate().child(title))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(group),
                            )
                    })
            });
            let palette = KitCommand::new(&content)
                .items(items)
                .placeholder("Search commands")
                .bordered(false)
                .max_h(px(300.))
                .empty(|_, _, cx| {
                    div()
                        .id("commands-empty")
                        .role(Role::Status)
                        .aria_label("No matching commands")
                        .test_support()
                        .child(
                            gpui_kit::component::empty::Empty::new()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("No matching commands"),
                        )
                });
            let commands = commands.clone();
            let focus = focus.clone();
            let confirmed = confirmed.clone();
            dialog.title("Commands").w(px(560.)).child(
                div()
                    .id("command-list")
                    .test_support()
                    .h(px(350.))
                    .child(palette.on_confirm(move |index, window, cx| {
                        let Some(command) =
                            commands.get(index.row).filter(|command| command.enabled)
                        else {
                            return;
                        };
                        if confirmed.replace(true) {
                            return;
                        }
                        // Actionless items defer dispatch until the dialog releases its focus.
                        let action = command.action.boxed_clone();
                        let focus = focus.clone();
                        window.close_dialog(cx);
                        window.defer(cx, move |window, cx| {
                            window.focus(&focus, cx);
                            window.dispatch_action(action, cx);
                        });
                    })),
            )
        });
        state.update(cx, |state, cx| state.focus(window, cx));
    }
}
