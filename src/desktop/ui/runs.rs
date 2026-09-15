//! Run state is independent of document fields so log updates cannot disturb drafts.

use std::time::Duration;

use super::*;
use crate::desktop::runner::{self, RunCommand, RunManager, RunOptions, RunOutcome, RunRequest};

#[derive(Clone, Debug)]
pub(super) enum RunEvent {
    Requested,
    Finished,
    Executable(PathBuf),
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
struct ChooseCommand {
    index: usize,
}

pub(super) struct RunPanel {
    root: Option<PathBuf>,
    executable: PathBuf,
    command: RunCommand,
    options: Value,
    bindings: Vec<Binding>,
    errors: BTreeMap<String, String>,
    subscriptions: Vec<Subscription>,
    manager: RunManager,
    log: Entity<EditorState>,
    focus: FocusHandle,
    status: String,
    tab: usize,
    disabled: bool,
    confirmation: bool,
    log_text: String,
}

impl EventEmitter<RunEvent> for RunPanel {}

impl RunPanel {
    pub(super) fn new(
        executable: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut panel = Self {
            root: None,
            executable: executable
                .or_else(|| runner::default_executable().ok())
                .unwrap_or_default(),
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
        };
        panel.rebuild(window, cx);
        panel
    }

    pub(super) fn running(&self) -> bool {
        self.manager.running()
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
        self.root = Some(root);
        self.options["suites"] = json!([]);
        self.confirmation = false;
        self.rebuild(window, cx);
        cx.notify();
    }

    pub(super) fn set_suites(
        &mut self,
        suites: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.running() {
            return;
        }
        self.options["suites"] = json!(suites);
        self.rebuild(window, cx);
        cx.notify();
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
        if self.running() {
            return;
        }
        self.command = command;
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
            .filter(|field| RunOptions::applies(&field.key, self.command))
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
                                this.change(&descriptor, &input.read(cx).value().to_string(), cx);
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
                                this.change(&descriptor, &input.read(cx).value().to_string(), cx);
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
        if self.running() || self.disabled || !self.errors.is_empty() {
            return;
        }
        if self.command.needs_confirmation() && !self.confirmation {
            self.confirmation = true;
            cx.notify();
            return;
        }
        cx.emit(RunEvent::Requested);
    }

    pub(super) fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.running() || !self.errors.is_empty() {
            return;
        }
        let request = (|| -> Result<_> {
            Ok(RunRequest {
                executable: self.executable.clone(),
                root: self
                    .root
                    .clone()
                    .ok_or_else(|| anyhow!("Open a workspace first"))?,
                command: self.command,
                options: serde_json::from_value(self.options.clone())?,
            })
        })();
        match request.and_then(|request| self.manager.start(request)) {
            Ok(()) => {
                self.confirmation = false;
                self.tab = 1;
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
                                        Some(RunOutcome::Completed) => {
                                            "Command completed; visual outcomes are in Results"
                                                .into()
                                        }
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

    fn pick_executable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        cx.spawn_in(window, async move |view, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("Select the regressify CLI executable")
                .pick_file()
                .await
            {
                let _ = view.update_in(cx, |this, _, cx| {
                    this.executable = file.path().to_path_buf();
                    cx.emit(RunEvent::Executable(this.executable.clone()));
                    cx.notify();
                });
            }
        })
        .detach();
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

impl Render for RunPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.running();
        let focus = self.focus.clone();
        let scope = serde_json::from_value::<RunOptions>(self.options.clone())
            .map(|options| {
                if options.suites.is_empty() {
                    "All suites".into()
                } else {
                    options.suites.join(", ")
                }
            })
            .unwrap_or_else(|_| "Invalid options".into());
        let content = if self.tab == 0 {
            self.render_options(cx)
        } else {
            CodeEditor::new(&self.log)
                .size_full()
                .readonly(true)
                .into_any_element()
        };
        div()
            .id("run-panel")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .on_action(cx.listener(|this, action: &ChooseCommand, window, cx| {
                if let Some(command) = RunCommand::ALL.get(action.index) {
                    this.choose(*command, window, cx);
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
                    .child(
                        Button::new("choose-command")
                            .label(self.command.label())
                            .disabled(running || self.disabled)
                            .dropdown_menu(move |menu, _, _| {
                                let mut menu = menu.action_context(focus.clone());
                                for (index, command) in RunCommand::ALL.iter().enumerate() {
                                    menu = menu
                                        .menu(command.label(), Box::new(ChooseCommand { index }));
                                }
                                menu
                            }),
                    )
                    .child(
                        Button::new("start-command")
                            .primary()
                            .icon(gpui_kit::assets::IconName::Play)
                            .label("Run")
                            .disabled(
                                running
                                    || self.disabled
                                    || self.root.is_none()
                                    || !self.errors.is_empty(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.request_start(cx))),
                    )
                    .child(
                        Button::new("cancel-command")
                            .icon(gpui_kit::assets::IconName::Square)
                            .tooltip("Cancel running command")
                            .disabled(!running)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(scope)),
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
                        .child(div().flex_1().child(format!(
                                "{} will modify {}",
                                self.command.label(),
                                self.output_root()
                                    .map(|path| path.display().to_string())
                                    .unwrap_or_default()
                            )))
                        .child(Button::new("run-confirm-cancel").label("Cancel").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.confirmation = false;
                                cx.notify();
                            }),
                        ))
                        .child(
                            Button::new("run-confirm")
                                .danger()
                                .label("Confirm")
                                .on_click(cx.listener(|this, _, _, cx| this.request_start(cx))),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.executable.display().to_string()),
                    )
                    .child(
                        Button::new("select-cli")
                            .ghost()
                            .small()
                            .icon(IconName::FolderOpen)
                            .tooltip("Select CLI executable")
                            .disabled(running)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.pick_executable(window, cx)),
                            ),
                    ),
            )
            .child(
                TabBar::new("run-tabs")
                    .underline()
                    .selected_index(self.tab)
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.tab = *index;
                        cx.notify();
                    }))
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
