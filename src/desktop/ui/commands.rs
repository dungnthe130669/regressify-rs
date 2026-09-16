//! Search existing workbench actions without creating another execution path.

use gpui_kit::component::list::{List, ListDelegate, ListItem, ListState};

use super::*;
use crate::desktop::workspace::ThemePreference;

struct Command {
    title: &'static str,
    group: &'static str,
    icon: gpui_kit::assets::IconName,
    action: Box<dyn Action>,
    enabled: bool,
}

struct CommandList {
    commands: Vec<Command>,
    filtered: Vec<usize>,
    selected: Option<IndexPath>,
    focus: FocusHandle,
}

impl ListDelegate for CommandList {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.filtered.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let words = query.to_lowercase();
        self.filtered = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, command)| {
                let text = format!("{} {}", command.title, command.group).to_lowercase();
                words.split_whitespace().all(|word| text.contains(word))
            })
            .map(|(index, _)| index)
            .collect();
        self.selected = (!self.filtered.is_empty()).then_some(IndexPath::new(0));
        Task::ready(())
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let command = self.commands.get(*self.filtered.get(index.row)?)?;
        Some(
            ListItem::new(SharedString::from(format!("command:{}", command.title)))
                .aria_label(command.title)
                .h(px(42.))
                .disabled(!command.enabled)
                .child(
                    Icon::new(command.icon)
                        .small()
                        .text_color(if command.enabled {
                            cx.theme().foreground
                        } else {
                            cx.theme().muted_foreground
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .px_2()
                        .text_sm()
                        .truncate()
                        .child(command.title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(command.group),
                ),
        )
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .id("commands-empty")
            .role(Role::Status)
            .aria_label("No matching commands")
            .test_support()
            .py_6()
            .text_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child("No matching commands")
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = index;
    }

    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(command) = self
            .selected
            .and_then(|index| self.filtered.get(index.row))
            .and_then(|index| self.commands.get(*index))
            .filter(|command| command.enabled)
        else {
            return;
        };
        let action = command.action.boxed_clone();
        let focus = self.focus.clone();
        window.close_dialog(cx);
        window.defer(cx, move |window, cx| {
            window.focus(&focus, cx);
            window.dispatch_action(action, cx);
        });
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        window.close_dialog(cx);
    }
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
        let editable = self.field_errors.is_empty() && self.diagnostic.is_none();
        let mut commands = vec![
            Command {
                title: "Save document",
                group: "File",
                icon: Icon::Save,
                action: Box::new(SaveDocument),
                enabled: open.is_some() && editable && self.unsaved(),
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
            let enabled = match command.operation {
                Operation::Duplicate | Operation::Rename => suite,
                Operation::Delete => suite && !self.unsaved(),
                Operation::SaveAs => open.is_some() && editable,
                Operation::Reload => open.is_some(),
                Operation::CloseWorkspace => workspace && !self.runs.read(cx).running(),
                _ => workspace,
            };
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
        let filtered = (0..commands.len()).collect();
        let focus = self.focus.clone();
        let list = cx.new(|cx| {
            ListState::new(
                CommandList {
                    commands,
                    filtered,
                    selected: Some(IndexPath::new(0)),
                    focus,
                },
                window,
                cx,
            )
            .searchable(true)
        });
        let content = list.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title("Commands").w(px(560.)).child(
                div().id("command-list").test_support().h(px(350.)).child(
                    List::new(&content)
                        .search_placeholder("Search commands")
                        .max_h(px(350.)),
                ),
            )
        });
        list.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(0)), window, cx);
            state.focus(window, cx);
        });
    }
}
