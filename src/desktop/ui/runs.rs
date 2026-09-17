//! Run state is independent of document fields so log updates cannot disturb drafts.

use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use super::*;
use crate::desktop::runner::{RunCommand, RunManager, RunOptions, RunOutcome, RunRequest};
use crate::desktop::workspace::{SuiteSelection, SuiteSummary};

mod activity;

#[derive(Clone, Debug)]
pub(super) enum RunEvent {
    Requested,
    Finished,
    SelectionChanged,
    ProgressChanged,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
struct RequestCommand {
    index: usize,
}

pub(super) struct RunPanel {
    root: Option<PathBuf>,
    command: RunCommand,
    options: Value,
    bindings: Vec<Binding>,
    errors: BTreeMap<String, String>,
    subscriptions: Vec<Subscription>,
    pub(super) manager: RunManager,
    log: Entity<EditorState>,
    focus: FocusHandle,
    status: String,
    tab: usize,
    disabled: bool,
    confirmation: bool,
    log_text: String,
    suites: Vec<SuiteSummary>,
    selection: SuiteSelection,
    catalog_error: Option<String>,
    collapsed: BTreeSet<String>,
    failed_only: bool,
    started: Option<Instant>,
}

impl EventEmitter<RunEvent> for RunPanel {}

impl RunPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            root: None,
            command: RunCommand::Test,
            options: serde_json::to_value(RunOptions::default()).unwrap_or(Value::Null),
            bindings: Vec::new(),
            errors: BTreeMap::new(),
            subscriptions: Vec::new(),
            manager: RunManager::default(),
            log: cx.new(|cx| EditorState::new(window, cx).line_number(false)),
            focus: cx.focus_handle(),
            status: "No command running".into(),
            tab: 0,
            disabled: false,
            confirmation: false,
            log_text: String::new(),
            suites: Vec::new(),
            selection: SuiteSelection::default(),
            catalog_error: None,
            collapsed: BTreeSet::new(),
            failed_only: false,
            started: None,
        };
        panel.rebuild(window, cx);
        panel
    }

    pub(super) fn running(&self) -> bool {
        self.manager.running()
    }
    pub(super) fn progress(&self) -> &crate::progress::LiveProgress {
        &self.manager.progress
    }
    pub(super) fn requires_suites(&self) -> bool {
        self.command.supports_suites()
    }
    pub(super) fn cancel(&mut self, cx: &mut Context<Self>) {
        self.manager.cancel();
        self.status = "Cancelling process tree...".into();
        cx.notify();
    }
    pub(super) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    pub(super) fn set_workspace(
        &mut self,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear(window, cx);
        self.root = Some(root);
        self.confirmation = false;
        self.rebuild(window, cx);
        cx.notify();
    }

    pub(super) fn set_suites(
        &mut self,
        suites: Vec<String>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        anyhow::ensure!(
            !self.running() && !self.disabled,
            "Another operation is already running"
        );
        self.flush_options(cx)?;
        let mut selection = SuiteSelection::default();
        for suite in self
            .suites
            .iter()
            .filter(|suite| suites.contains(&suite.name))
        {
            selection.toggle(suite, true)?;
        }
        anyhow::ensure!(
            selection.len() == suites.len(),
            "Selected suite is no longer available"
        );
        self.selection = selection;
        self.confirmation = false;
        cx.emit(RunEvent::SelectionChanged);
        cx.notify();
        Ok(())
    }

    pub(super) fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        self.root = None;
        self.options = serde_json::to_value(RunOptions::default()).unwrap_or(Value::Null);
        self.manager = RunManager::default();
        self.command = RunCommand::Test;
        self.selection.clear();
        self.suites.clear();
        self.catalog_error = None;
        self.log_text.clear();
        self.collapsed.clear();
        self.failed_only = false;
        self.started = None;
        self.log
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.status = "No command running".into();
        self.tab = 0;
        self.confirmation = false;
        self.rebuild(window, cx);
        cx.notify();
    }

    pub(super) fn update_catalog(
        &mut self,
        suites: Vec<SuiteSummary>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.running() {
            return;
        }
        self.selection.reconcile(&suites);
        self.suites = suites;
        self.catalog_error = error;
        self.confirmation = false;
        cx.emit(RunEvent::SelectionChanged);
        cx.notify();
    }

    pub(super) fn selected(&self, name: &str) -> bool {
        self.selection.contains(name)
    }
    pub(super) fn selected_count(&self) -> usize {
        self.selection.len()
    }
    pub(super) fn selected_names(&self) -> Vec<String> {
        self.suites
            .iter()
            .filter(|suite| self.selection.contains(&suite.name))
            .map(|suite| suite.name.clone())
            .collect()
    }
    pub(super) fn toggle_suite(&mut self, name: &str, checked: bool, cx: &mut Context<Self>) {
        if self.running() || self.disabled {
            return;
        }
        if let Some(suite) = self.suites.iter().find(|suite| suite.name == name) {
            if let Err(error) = self.selection.toggle(suite, checked) {
                self.status = format!("{error:#}");
            }
            self.confirmation = false;
            cx.emit(RunEvent::SelectionChanged);
            cx.notify();
        }
    }

    pub(super) fn select_all(&mut self, selected: bool, cx: &mut Context<Self>) {
        if self.running() || self.disabled {
            return;
        }
        if selected {
            self.selection.select_all(&self.suites);
        } else {
            self.selection.clear();
        }
        self.confirmation = false;
        cx.emit(RunEvent::SelectionChanged);
        cx.notify();
    }

    fn can_start(&self) -> bool {
        self.can_request(self.command)
    }

    fn can_request(&self, command: RunCommand) -> bool {
        !self.running()
            && !self.disabled
            && self.errors.is_empty()
            && self.root.is_some()
            && (!command.supports_suites()
                || (!self.selection.is_empty() && self.catalog_error.is_none()))
    }

    fn flush_options(&mut self, cx: &App) -> Result<()> {
        let mut options = self.options.clone();
        for binding in &self.bindings {
            let text = match &binding.control {
                Control::Input(input) => input.read(cx).value().to_string(),
                Control::Textarea(input) => input.read(cx).value().to_string(),
                _ => continue,
            };
            if text == binding.field.display(&options[&binding.field.key])
                && !self.errors.contains_key(&binding.field.key)
            {
                continue;
            }
            match binding.field.parse(&text) {
                Ok(value) => {
                    options[&binding.field.key] = value;
                    self.errors.remove(&binding.field.key);
                }
                Err(error) => {
                    self.errors
                        .insert(binding.field.key.clone(), format!("{error:#}"));
                }
            }
        }
        anyhow::ensure!(
            self.errors.is_empty(),
            "Correct run options before continuing"
        );
        serde_json::from_value::<RunOptions>(options.clone())?;
        self.options = options;
        Ok(())
    }

    pub(super) fn output_root(&self) -> Result<PathBuf> {
        let options: RunOptions = serde_json::from_value(self.options.clone())?;
        Ok(options.output_directory(
            self.root
                .as_ref()
                .ok_or_else(|| anyhow!("Open a workspace first"))?,
        ))
    }

    pub(super) fn choose(
        &mut self,
        command: RunCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.running() || self.disabled {
            return;
        }
        if let Err(error) = self.flush_options(cx) {
            self.status = format!("{error:#}");
            self.tab = 2;
            cx.notify();
            return;
        }
        self.command = command;
        self.tab = if command.supports_suites() { 0 } else { 2 };
        self.confirmation = false;
        self.rebuild(window, cx);
        cx.notify();
    }

    fn rebuild(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bindings.clear();
        self.subscriptions.clear();
        self.errors.clear();
        let Ok(fields) = schema::fields(&schema::Scope::Run) else {
            return;
        };
        for field in fields
            .into_iter()
            .filter(|field| field.key != "suites" && RunOptions::applies(&field.key, self.command))
        {
            let text = field.display(&self.options[&field.key]);
            let control = match &field.kind {
                FieldKind::Boolean => Control::Boolean,
                FieldKind::List { .. } => {
                    let input =
                        cx.new(|cx| TextareaState::new(window, cx).rows(2).default_value(text));
                    let descriptor = field.clone();
                    self.subscriptions.push(cx.subscribe(
                        &input,
                        move |this, input, event: &InputEvent, cx| {
                            if matches!(event, InputEvent::Change) {
                                this.change(&descriptor, input.read(cx).value().as_ref(), cx);
                            }
                        },
                    ));
                    Control::Textarea(input)
                }
                _ => {
                    let input = cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(text)
                            .masked(field.key == "proxy")
                    });
                    let descriptor = field.clone();
                    self.subscriptions.push(cx.subscribe(
                        &input,
                        move |this, input, event: &InputEvent, cx| {
                            if matches!(event, InputEvent::Change) {
                                this.change(&descriptor, input.read(cx).value().as_ref(), cx);
                            }
                        },
                    ));
                    Control::Input(input)
                }
            };
            self.bindings.push(Binding { field, control });
        }
    }

    fn change(&mut self, field: &Field, text: &str, cx: &mut Context<Self>) {
        if self.running() || self.disabled {
            return;
        }
        match field.parse(text) {
            Ok(value) => {
                self.options[&field.key] = value;
                self.errors.remove(&field.key);
            }
            Err(error) => {
                self.errors.insert(field.key.clone(), format!("{error:#}"));
            }
        }
        self.confirmation = false;
        cx.notify();
    }

    fn request_start(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.flush_options(cx) {
            self.status = format!("{error:#}");
            self.tab = 2;
            cx.notify();
            return;
        }
        if !self.can_start() {
            return;
        }
        if self.command.needs_confirmation() && !self.confirmation {
            self.confirmation = true;
            cx.notify();
            return;
        }
        cx.emit(RunEvent::Requested);
    }

    pub(super) fn request_command(
        &mut self,
        command: RunCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.running() || self.disabled {
            return;
        }
        self.choose(command, window, cx);
        if self.command != command || !self.errors.is_empty() {
            return;
        }
        self.tab = 0;
        self.confirmation = false;
        self.request_start(cx);
    }

    pub(super) fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.flush_options(cx) {
            self.status = format!("{error:#}");
            cx.notify();
            return;
        }
        if !self.can_start() {
            return;
        }
        let request = (|| -> Result<_> {
            let mut options: RunOptions = serde_json::from_value(self.options.clone())?;
            if self.command.supports_suites() {
                options.suites = self.selection.filters(
                    &self.suites,
                    matches!(self.command, RunCommand::Compare | RunCommand::Approve),
                )?;
            }
            Ok(RunRequest {
                root: self
                    .root
                    .clone()
                    .ok_or_else(|| anyhow!("Open a workspace first"))?,
                command: self.command,
                options,
            })
        })();
        match request.and_then(|request| self.manager.start(request)) {
            Ok(()) => {
                self.confirmation = false;
                self.tab = 0;
                self.started = Some(Instant::now());
                self.collapsed.clear();
                self.status = format!("Running {}", self.command.label());
                self.log_text.clear();
                self.log
                    .update(cx, |state, cx| state.set_value("", window, cx));
                cx.spawn_in(window, async move |view, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                        let running = view
                            .update_in(cx, |this, window, cx| {
                                let finished = this.manager.poll();
                                cx.emit(RunEvent::ProgressChanged);
                                let text = this
                                    .manager
                                    .logs
                                    .iter()
                                    .map(|line| {
                                        if line.stderr {
                                            format!("[stderr] {}", line.text)
                                        } else {
                                            line.text.clone()
                                        }
                                    })
                                    .collect::<String>();
                                if text != this.log_text {
                                    this.log.update(cx, |state, cx| {
                                        state.set_value(text.clone(), window, cx)
                                    });
                                    this.log_text = text;
                                }
                                if finished {
                                    this.status = match &this.manager.outcome {
                                        Some(RunOutcome::Completed) => "Completed".into(),
                                        Some(RunOutcome::Cancelled) => "Cancelled".into(),
                                        Some(RunOutcome::Failed(error)) => error.clone(),
                                        None => "Command ended without a result".into(),
                                    };
                                    cx.emit(RunEvent::Finished);
                                }
                                cx.notify();
                                this.running()
                            })
                            .unwrap_or(false);
                        if !running {
                            break;
                        }
                    }
                })
                .detach();
            }
            Err(error) => self.status = format!("{error:#}"),
        }
        cx.notify();
    }

    fn render_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.running() || self.disabled;
        let mut fields = div().flex().flex_col().gap_4().p_5().max_w(px(820.));
        for binding in &self.bindings {
            let field = &binding.field;
            let id = SharedString::from(format!("run-field-{}", field.key));
            let control = match &binding.control {
                Control::Input(input) => Input::new(input)
                    .id(id)
                    .disabled(disabled)
                    .into_any_element(),
                Control::Textarea(input) => div()
                    .id(id)
                    .child(Textarea::new(input).disabled(disabled).h(px(72.)))
                    .into_any_element(),
                _ => {
                    let descriptor = field.clone();
                    Checkbox::new(id)
                        .label(field.label.clone())
                        .checked(self.options[&field.key].as_bool().unwrap_or(false))
                        .disabled(disabled)
                        .on_change(cx.listener(move |this, checked, _, cx| {
                            this.change(&descriptor, if *checked { "true" } else { "false" }, cx)
                        }))
                        .into_any_element()
                }
            };
            let key = field.key.clone();
            fields = fields.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(field.label.clone())
                            .when(key == "fetchBrowser", |row| {
                                row.child(
                                    Button::new("fetch-auto")
                                        .ghost()
                                        .xsmall()
                                        .label("Auto")
                                        .disabled(disabled)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.options["fetchBrowser"] = Value::Null;
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .child(control)
                    .children(self.errors.get(&key).map(|error| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(error.clone())
                    })),
            );
        }
        div()
            .id("run-options-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(fields)
            .into_any_element()
    }
}

impl RunPanel {
    fn render_selection(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.disabled || self.running();
        let mut list = div().flex().flex_col().gap_2().child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(format!("{} suites selected", self.selection.len()))
                .child(
                    Checkbox::new("select-all-suites")
                        .label("Select all")
                        .checked(
                            !self.selection.is_empty()
                                && self.selection.len()
                                    == self
                                        .suites
                                        .iter()
                                        .filter(|suite| suite.error.is_none())
                                        .count(),
                        )
                        .disabled(disabled || self.suites.is_empty())
                        .on_change(
                            cx.listener(|this, checked, _, cx| this.select_all(*checked, cx)),
                        ),
                ),
        );
        for suite in &self.suites {
            let name = suite.name.clone();
            list = list.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .py_1()
                    .child(
                        Checkbox::new(SharedString::from(format!("suite-check-{name}")))
                            .label(format!(
                                "{}  ({} scenarios)",
                                suite.name,
                                suite.scenarios.len()
                            ))
                            .checked(self.selection.contains(&name))
                            .disabled(disabled || suite.error.is_some())
                            .on_change(cx.listener(move |this, checked, _, cx| {
                                this.toggle_suite(&name, *checked, cx)
                            })),
                    )
                    .children(suite.error.as_ref().map(|error| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(error.clone())
                    })),
            );
        }
        if let Some(error) = &self.catalog_error {
            list = list.child(div().text_color(cx.theme().danger).child(error.clone()));
        }
        list.into_any_element()
    }
}

impl Render for RunPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.running();
        let focus = self.focus.clone();
        let scope = if self.command.supports_suites() {
            let names = self.selected_names();
            if names.is_empty() {
                "No suites selected".into()
            } else {
                format!("Suites: {}", names.join(", "))
            }
        } else {
            "Workspace command".into()
        };
        let content = match self.tab {
            0 => self.render_activity(cx),
            1 => div()
                .id("run-suite-list")
                .size_full()
                .overflow_y_scroll()
                .p_5()
                .child(self.render_selection(cx))
                .into_any_element(),
            2 => self.render_options(cx),
            _ => CodeEditor::new(&self.log)
                .size_full()
                .readonly(true)
                .into_any_element(),
        };
        div()
            .id("run-panel")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .on_action(cx.listener(|this, action: &RequestCommand, window, cx| {
                if let Some(command) = RunCommand::ALL.get(action.index) {
                    this.request_command(*command, window, cx);
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .children(
                        [
                            (
                                "run-test",
                                RunCommand::Test,
                                gpui_kit::assets::IconName::Play,
                            ),
                            (
                                "run-reference",
                                RunCommand::Reference,
                                gpui_kit::assets::IconName::Camera,
                            ),
                            (
                                "run-approve",
                                RunCommand::Approve,
                                gpui_kit::assets::IconName::Check,
                            ),
                            (
                                "run-compare",
                                RunCommand::Compare,
                                gpui_kit::assets::IconName::Images,
                            ),
                        ]
                        .into_iter()
                        .map(|(id, command, icon)| {
                            Button::new(id)
                                .small()
                                .icon(icon)
                                .label(command.action_label())
                                .when(command == RunCommand::Test, |button| button.primary())
                                .disabled(!self.can_request(command))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.request_command(command, window, cx)
                                }))
                        }),
                    )
                    .child(
                        Button::new("more-run-actions")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::Ellipsis)
                            .accessibility_label("More actions")
                            .tooltip("More actions")
                            .disabled(running || self.disabled || self.root.is_none())
                            .dropdown_menu(move |menu, _, _| {
                                let mut menu = menu.action_context(focus.clone());
                                for (index, command) in RunCommand::ALL
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, command)| !command.supports_suites())
                                {
                                    menu = menu.menu(
                                        command.action_label(),
                                        Box::new(RequestCommand { index }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(
                        Button::new("cancel-command")
                            .icon(gpui_kit::assets::IconName::Square)
                            .label("Stop")
                            .tooltip("Cancel running command")
                            .disabled(!running)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    )
                    .child(
                        div()
                            .id("run-scope")
                            .role(Role::Status)
                            .aria_label(scope.clone())
                            .test_support()
                            .w_full()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(scope),
                    ),
            )
            .when(self.confirmation, |panel| {
                panel.child(
                    div()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .bg(cx.theme().warning.opacity(0.12))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(self.command.confirmation_message()),
                                )
                                .child(div().text_sm().child(if self.command.supports_suites() {
                                    format!("{} selected suites", self.selection.len())
                                } else {
                                    "Workspace images".into()
                                }))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate()
                                        .child(
                                            self.output_root()
                                                .map(|path| display_path(&path))
                                                .unwrap_or_default(),
                                        ),
                                ),
                        )
                        .child(Button::new("run-confirm-cancel").label("Cancel").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.confirmation = false;
                                cx.notify();
                            }),
                        ))
                        .child(
                            Button::new("run-confirm")
                                .danger()
                                .label(self.command.action_label())
                                .on_click(cx.listener(|this, _, _, cx| this.request_start(cx))),
                        ),
                )
            })
            .child(
                TabBar::new("run-tabs")
                    .underline()
                    .selected_index(self.tab)
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.tab = *index;
                        cx.notify();
                    }))
                    .child(Tab::new().label("Activity"))
                    .child(Tab::new().label("Suites"))
                    .child(Tab::new().label("Options"))
                    .child(Tab::new().label("Log")),
            )
            .child(div().flex_1().min_h_0().child(content))
            .child(
                div()
                    .id("run-status")
                    .role(Role::Status)
                    .aria_label(self.status.clone())
                    .test_support()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(self.status.clone()),
            )
            .when(self.manager.omitted_chunks > 0, |panel| {
                panel.child(div().px_4().text_xs().child(format!(
                    "{} log chunks omitted",
                    self.manager.omitted_chunks
                )))
            })
    }
}
