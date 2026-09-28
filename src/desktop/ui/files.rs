use super::*;

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, schemars::JsonSchema)]
pub(super) enum Operation {
    New,
    Duplicate,
    Rename,
    SaveAs,
    Reload,
    Delete,
    Config,
    Viewports,
    Browse,
    Refresh,
    Theme,
    OpenVSCode,
    OpenFolder,
    CloseWorkspace,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
pub(super) struct FileCommand {
    pub(super) operation: Operation,
}

#[derive(Clone)]
pub(super) enum Pending {
    Workspace(PathBuf),
    Location(Entry, Selection),
    Name(Operation),
    SuiteOperation(Entry, Operation),
    Reload,
    Close,
    Update,
    Run(Box<runs::RunIntent>),
    Git(Box<crate::desktop::git::Request>),
    CloseWorkspace,
}

#[derive(Default)]
pub(super) struct ModalState {
    busy: bool,
    blocked: bool,
    status: String,
}

impl Workbench {
    pub(super) fn choose_field_file(
        &mut self,
        key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.session.workspace.clone() else {
            return;
        };
        let selection = self.session.selection;
        let document_path = self
            .session
            .open
            .as_ref()
            .map(|open| open.entry.path.clone());
        cx.spawn_in(window, async move |view, cx| {
            let picker = rfd::AsyncFileDialog::new().set_directory(&workspace.root);
            let files = if key == "file" {
                picker.pick_files().await
            } else {
                picker.pick_file().await.map(|file| vec![file])
            };
            let Some(files) = files else {
                return;
            };
            let paths = files
                .iter()
                .map(|file| workspace.relative_file(file.path(), key == "file"))
                .collect::<Result<Vec<_>>>();
            let _ = view.update_in(cx, |this, window, cx| {
                if this.session.workspace.as_ref().map(|active| &active.root)
                    != Some(&workspace.root)
                    || this.session.selection != selection
                    || this.session.open.as_ref().map(|open| &open.entry.path)
                        != document_path.as_ref()
                {
                    this.status =
                        "Selection changed while choosing a file; no field was modified".into();
                    cx.notify();
                    return;
                }
                match paths {
                    Ok(paths) => {
                        let value = if paths.len() == 1 {
                            json!(paths[0])
                        } else {
                            json!(paths)
                        };
                        this.edit(|session| session.set_field(&key, Some(value)), window, cx);
                    }
                    Err(error) => {
                        this.status = format!("{error:#}");
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn sync_modal(&self, cx: &mut Context<Self>) {
        let disabled = self.busy || self.runs.read(cx).running();
        self.git
            .update(cx, |panel, cx| panel.set_disabled(disabled, cx));
        self.results
            .update(cx, |panel, cx| panel.set_disabled(disabled, cx));
        self.runs
            .update(cx, |panel, cx| panel.set_disabled(self.busy, cx));
        self.modal.update(cx, |state, cx| {
            state.busy = self.busy;
            state.blocked = self.diagnostic.is_some() || !self.field_errors.is_empty();
            state.status.clone_from(&self.status);
            cx.notify();
        });
    }

    pub(super) fn file_operation_enabled(&self, operation: Operation, cx: &App) -> bool {
        let open = self.session.open.as_ref();
        let workspace = self.session.workspace.is_some();
        let suite = open.is_some_and(|open| open.entry.kind == DocumentKind::Suite);
        !self.busy
            && match operation {
                Operation::Duplicate => suite,
                Operation::Rename => suite && !self.runs.read(cx).running(),
                Operation::Delete => suite && !self.unsaved() && !self.runs.read(cx).running(),
                Operation::SaveAs => {
                    open.is_some() && self.diagnostic.is_none() && self.field_errors.is_empty()
                }
                Operation::Reload => open.is_some(),
                Operation::CloseWorkspace => workspace && !self.runs.read(cx).running(),
                Operation::Theme => true,
                _ => workspace,
            }
    }

    pub(super) fn file_menu(&self, cx: &App) -> impl IntoElement {
        let focus = self.focus.clone();
        let entries = menu_entries()
            .into_iter()
            .map(|(label, action)| {
                let enabled = self.file_operation_enabled(action.operation, cx);
                (label, action, enabled)
            })
            .collect::<Vec<_>>();
        Button::new("file-menu")
            .ghost()
            .xsmall()
            .label("File")
            .disabled(self.busy)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.action_context(focus.clone());
                for (label, action, enabled) in &entries {
                    menu = menu.menu_with_disabled(*label, Box::new(action.clone()), !enabled);
                }
                menu
            })
    }

    pub(super) fn file_command(
        &mut self,
        command: &FileCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        match command.operation {
            Operation::New | Operation::Duplicate => {
                self.guard(Pending::Name(command.operation), window, cx)
            }
            Operation::Rename | Operation::Delete => {
                if let Some(entry) = self.session.open.as_ref().map(|open| open.entry.clone()) {
                    self.begin_suite_operation(entry, command.operation, window, cx);
                }
            }
            Operation::SaveAs => self.name_dialog(Operation::SaveAs, window, cx),
            Operation::Reload => self.guard(Pending::Reload, window, cx),
            Operation::Config => self.create_config(false, window, cx),
            Operation::Viewports => self.create_config(true, window, cx),
            Operation::Browse => self.browse_document(window, cx),
            Operation::Refresh => self.refresh_files(window, cx),
            Operation::OpenVSCode | Operation::OpenFolder => {
                let Some(workspace) = &self.session.workspace else {
                    return;
                };
                let result = if command.operation == Operation::OpenVSCode {
                    vscode_workspace_url(&workspace.root)
                        .and_then(|url| open::that_detached(url.as_str()).map_err(Into::into))
                } else {
                    open::that_detached(&workspace.root).map_err(Into::into)
                };
                if let Err(error) = result {
                    self.status = format!("Cannot open workspace externally: {error:#}");
                    cx.notify();
                }
            }
            Operation::CloseWorkspace => self.guard(Pending::CloseWorkspace, window, cx),
            Operation::Theme => {
                let preference = if cx.theme().mode.is_dark() {
                    crate::desktop::workspace::ThemePreference::Light
                } else {
                    crate::desktop::workspace::ThemePreference::Dark
                };
                self.change_theme(&design::ChooseTheme { preference }, window, cx);
            }
        }
    }

    pub(super) fn guard(&mut self, pending: Pending, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if !self.unsaved() {
            self.perform_pending(pending, window, cx);
            return;
        }
        let view = cx.entity();
        self.sync_modal(cx);
        let modal = self.modal.clone();
        let target = self
            .session
            .open
            .as_ref()
            .map(|open| open.entry.label())
            .unwrap_or_default();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = modal.read(cx);
            let busy = state.busy;
            let blocked = state.blocked;
            let save = pending.clone();
            let discard = pending.clone();
            let run = matches!(pending, Pending::Run(_));
            dialog
                .title(if run {
                    "Save changes before running?"
                } else {
                    "Unsaved changes"
                })
                .close_button(false)
                .keyboard(!busy)
                .child(target.clone())
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(state.status.clone()),
                )
                .footer(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("guard-cancel")
                                .label("Cancel")
                                .disabled(busy)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("guard-discard")
                                .label(if run { "Run saved version" } else { "Discard" })
                                .disabled(busy)
                                .on_click(window.listener_for(
                                    &view,
                                    move |this, _, window, cx| match this.session.discard() {
                                        Ok(()) => {
                                            this.field_errors.clear();
                                            this.sync_editor(window, cx);
                                            window.close_dialog(cx);
                                            this.perform_pending(discard.clone(), window, cx);
                                        }
                                        Err(error) => {
                                            this.status = format!("{error:#}");
                                            cx.notify();
                                        }
                                    },
                                )),
                        )
                        .child(
                            Button::new("guard-save")
                                .primary()
                                .label(if run { "Save and continue" } else { "Save" })
                                .disabled(busy || blocked)
                                .on_click(window.listener_for(
                                    &view,
                                    move |this, _, window, cx| {
                                        this.save_and_continue(Some(save.clone()), window, cx);
                                    },
                                )),
                        ),
                )
        });
    }

    pub(super) fn perform_pending(
        &mut self,
        pending: Pending,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pending {
            Pending::Workspace(path) => self.load_workspace(path, window, cx),
            Pending::Location(entry, selection) => self.open_location(entry, selection, window, cx),
            Pending::Name(operation) => self.name_dialog(operation, window, cx),
            Pending::SuiteOperation(entry, operation) => {
                if self.busy || self.runs.read(cx).running() {
                    return;
                }
                if self
                    .session
                    .open
                    .as_ref()
                    .is_some_and(|open| open.entry.path == entry.path)
                {
                    self.finish_suite_operation(operation, window, cx);
                } else if let Some(workspace) = self.session.workspace.clone() {
                    self.task(
                        "Loading suite...",
                        move || workspace.open_document(entry),
                        move |this, open, window, cx| {
                            this.session.open = Some(open);
                            this.session.selection = Selection::Root;
                            this.tab = 0;
                            this.sync_editor(window, cx);
                            this.finish_suite_operation(operation, window, cx);
                        },
                        window,
                        cx,
                    );
                }
            }
            Pending::Reload => {
                if let Some(entry) = self.session.open.as_ref().map(|open| open.entry.clone()) {
                    self.load_document(entry, window, cx);
                }
            }
            Pending::Close => self.finish_close(cx),
            Pending::Update => self.finish_update(cx),
            Pending::Run(intent) => self.start_selected_run(*intent, window, cx),
            Pending::Git(request) => self.confirm_git(*request, window, cx),
            Pending::CloseWorkspace => self.close_workspace(window, cx),
        }
    }

    pub(super) fn begin_suite_operation(
        &mut self,
        entry: Entry,
        operation: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy
            || self.runs.read(cx).running()
            || entry.kind != DocumentKind::Suite
            || !matches!(operation, Operation::Rename | Operation::Delete)
        {
            return;
        }
        self.suite_menu = None;
        if self.session.open.is_some()
            && let Err(error) = self.flush_drafts(window, cx)
        {
            self.status = format!("{error:#}");
            cx.notify();
            return;
        }
        self.guard(Pending::SuiteOperation(entry, operation), window, cx);
    }

    fn finish_suite_operation(
        &mut self,
        operation: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match operation {
            Operation::Rename => self.name_dialog(operation, window, cx),
            Operation::Delete => self.confirm_remove(true, window, cx),
            _ => {}
        }
    }

    fn name_dialog(&mut self, operation: Operation, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.workspace.is_none() {
            self.status = "Open a workspace first".into();
            cx.notify();
            return;
        }
        let (title, initial) = match operation {
            Operation::New => ("New suite", "new-suite.tests.yaml".to_string()),
            Operation::Duplicate => ("Duplicate suite", "suite-copy.tests.yaml".to_string()),
            Operation::Rename => (
                "Rename suite",
                self.session
                    .open
                    .as_ref()
                    .and_then(|open| open.entry.path.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            Operation::SaveAs => (
                "Save As",
                self.session
                    .open
                    .as_ref()
                    .map(|open| open.entry.label())
                    .unwrap_or_default(),
            ),
            _ => return,
        };
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Filename")
                .default_value(initial)
        });
        let display_name = (operation == Operation::Rename).then(|| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Display name")
                    .default_value(self.value["displayName"].as_str().unwrap_or_default())
            })
        });
        let view = cx.entity();
        self.status = title.into();
        self.sync_modal(cx);
        let modal = self.modal.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let name = name.clone();
            let display_name = display_name.clone();
            let state = modal.read(cx);
            let busy = state.busy;
            dialog
                .title(title)
                .close_button(false)
                .keyboard(!busy)
                .when(operation == Operation::Rename, |dialog| {
                    dialog.child(div().text_sm().child("Filename"))
                })
                .child(Input::new(&name).id("file-name").disabled(busy))
                .when_some(display_name.as_ref(), |dialog, display_name| {
                    dialog.child(div().text_sm().child("Display name")).child(
                        Input::new(display_name)
                            .id("suite-display-name")
                            .disabled(busy),
                    )
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(state.status.clone()),
                )
                .footer(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("file-cancel")
                                .label("Cancel")
                                .disabled(busy)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("file-confirm")
                                .primary()
                                .label(if operation == Operation::New {
                                    "Create"
                                } else {
                                    "Save"
                                })
                                .disabled(busy)
                                .on_click(window.listener_for(
                                    &view,
                                    move |this, _, window, cx| {
                                        this.commit_named_file(
                                            operation,
                                            name.read(cx).value().to_string(),
                                            display_name
                                                .as_ref()
                                                .map(|input| input.read(cx).value().to_string()),
                                            window,
                                            cx,
                                        );
                                    },
                                )),
                        ),
                )
        });
    }

    fn snapshot(&self) -> Result<OpenDocument> {
        let open = self
            .session
            .open
            .as_ref()
            .ok_or_else(|| anyhow!("Open a document first"))?;
        let mut document = Document::new(
            open.document.kind,
            open.document.format,
            open.document.saved_source().into(),
        )?;
        document.set_source(open.document.source().into())?;
        Ok(OpenDocument {
            entry: open.entry.clone(),
            document,
        })
    }

    fn commit_named_file(
        &mut self,
        operation: Operation,
        name: String,
        display_name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.session.workspace.clone() else {
            return;
        };
        if !self.field_errors.is_empty() {
            self.status = "Correct invalid fields before saving".into();
            cx.notify();
            return;
        }
        let snapshot = if operation == Operation::New {
            None
        } else {
            match self.snapshot() {
                Ok(open) => Some(open),
                Err(error) => {
                    self.status = format!("{error:#}");
                    cx.notify();
                    return;
                }
            }
        };
        if operation == Operation::Rename && self.runs.read(cx).running() {
            return;
        }
        let previous = self
            .session
            .open
            .as_ref()
            .and_then(|open| {
                self.session
                    .suites
                    .iter()
                    .find(|suite| suite.entry.path == open.entry.path)
            })
            .cloned();
        let outputs = self.runs.read(cx).suite_output_roots();
        self.task(
            "Writing file...",
            move || {
                let open = match operation {
                    Operation::New => workspace.create_suite(&name)?,
                    Operation::Duplicate => workspace.duplicate_suite(
                        &snapshot.ok_or_else(|| anyhow!("Open a suite first"))?,
                        &name,
                    )?,
                    Operation::SaveAs => workspace.save_as(
                        &snapshot.ok_or_else(|| anyhow!("Open a document first"))?,
                        std::path::Path::new(&name),
                    )?,
                    Operation::Rename => {
                        let mut open = snapshot.ok_or_else(|| anyhow!("Open a suite first"))?;
                        workspace.rename_suite_in_outputs(
                            &mut open,
                            &name,
                            display_name.as_deref(),
                            &outputs,
                        )?;
                        open
                    }
                    _ => return Err(anyhow!("Unsupported file operation")),
                };
                Ok((open, workspace.entries()?))
            },
            move |this, (open, entries), window, cx| {
                if operation == Operation::Rename
                    && let Some(previous) = previous
                {
                    let mut updated = previous.clone();
                    updated.entry = open.entry.clone();
                    updated.name = open.entry.suite_name().unwrap_or_default();
                    let value = open.document.value().unwrap_or_default();
                    updated.output_id = value["id"].as_str().unwrap_or(&updated.name).into();
                    updated.display_name = value["displayName"].as_str().map(str::to_owned);
                    this.runs.update(cx, |panel, cx| {
                        panel.suite_changed(&previous, Some(&updated), cx)
                    });
                }
                this.session.open = Some(open);
                this.overview = false;
                this.main_tab = 0;
                this.session.entries = entries;
                this.session.selection = Selection::Root;
                this.suite_list.current = this
                    .session
                    .open
                    .as_ref()
                    .map(|open| open.entry.path.clone());
                this.tab = if this
                    .session
                    .open
                    .as_ref()
                    .is_some_and(|open| open.document.format == Format::Text)
                {
                    1
                } else {
                    0
                };
                this.sync_editor(window, cx);
                this.refresh_results(window, cx);
                this.status = "File saved".into();
                window.close_dialog(cx);
            },
            window,
            cx,
        );
    }

    pub(super) fn confirm_remove(
        &mut self,
        file: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.session.open.is_none() || (file && self.runs.read(cx).running()) {
            return;
        }
        if file && self.unsaved() {
            self.status = "Save or reload changes before deleting a suite".into();
            cx.notify();
            return;
        }
        let title = if file {
            "Delete suite and its data?"
        } else {
            "Remove selected item?"
        };
        let target = if file {
            self.session
                .open
                .as_ref()
                .map(|open| open.entry.label())
                .unwrap_or_default()
        } else {
            self.value
                .pointer(&self.session.selection.path())
                .and_then(|value| value.get("label"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| self.session.selection.path())
        };
        let view = cx.entity();
        self.sync_modal(cx);
        let modal = self.modal.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let busy = modal.read(cx).busy;
            dialog
                .title(title)
                .close_button(false)
                .keyboard(!busy)
                .child(div().id("remove-target").role(Role::Status).aria_label(title).test_support().child(target.clone()))
                .when(file, |dialog| dialog.child("Reference images, test runs, and local reports for this suite will also be deleted."))
                .footer(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("remove-cancel")
                                .label("Cancel")
                                .disabled(busy)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("remove-confirm")
                                .danger()
                                .label(if file { "Delete" } else { "Remove" })
                                .disabled(busy)
                                .on_click(window.listener_for(
                                    &view,
                                    move |this, _, window, cx| {
                                        if !file {
                                            this.edit(Session::remove_selected, window, cx);
                                            window.close_dialog(cx);
                                            return;
                                        }
                                        let Some(workspace) = this.session.workspace.clone() else {
                                            return;
                                        };
                                        if this.runs.read(cx).running() { return; }
                                        let outputs = this.runs.read(cx).suite_output_roots();
                                        let snapshot = match this.snapshot() {
                                            Ok(snapshot) => snapshot,
                                            Err(error) => {
                                                this.status = format!("{error:#}");
                                                cx.notify();
                                                return;
                                            }
                                        };
                                        let previous = this.session.suites.iter().find(|suite| suite.entry.path == snapshot.entry.path).cloned();
                                        this.task(
                                            "Deleting suite...",
                                            move || {
                                                workspace.delete_suite_in_outputs(&snapshot, &outputs)?;
                                                workspace.entries()
                                            },
                                            move |this, entries, window, cx| {
                                                if let Some(previous) = previous {
                                                    this.runs.update(cx, |panel, cx| panel.suite_changed(&previous, None, cx));
                                                }
                                                this.session.open = None;
                                                this.session.entries = entries;
                                                this.session.selection = Selection::Root;
                                                this.suite_list.current = None;
                                                this.sync_editor(window, cx);
                                                this.refresh_results(window, cx);
                                                this.status = "Suite deleted".into();
                                                window.close_dialog(cx);
                                            },
                                            window,
                                            cx,
                                        );
                                    },
                                )),
                        ),
                )
        });
    }

    fn create_config(&mut self, viewports: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.unsaved() {
            self.status = "Save the current document first".into();
            cx.notify();
            return;
        }
        let Some(workspace) = self.session.workspace.clone() else {
            return;
        };
        self.task(
            "Creating configuration...",
            move || {
                let (path, kind, source) = if viewports {
                    (
                        "common/_viewports.yaml",
                        DocumentKind::Viewports,
                        "- label: desktop\n  width: 1280\n  height: 720\n",
                    )
                } else {
                    ("regressify.yaml", DocumentKind::Workspace, "{}\n")
                };
                let open = workspace.create_document(std::path::Path::new(path), kind, source)?;
                Ok((open, workspace.entries()?))
            },
            |this, (open, entries), window, cx| {
                this.session.open = Some(open);
                this.session.entries = entries;
                this.session.selection = Selection::Root;
                this.tab = 0;
                this.sync_editor(window, cx);
                this.status = "Configuration created".into();
            },
            window,
            cx,
        );
    }

    fn browse_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.session.workspace.clone() else {
            return;
        };
        cx.spawn_in(window, async move |view, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_directory(&workspace.root)
                .add_filter("Configuration", &["yaml", "yml", "json"])
                .pick_file()
                .await
            {
                let path = file.path().canonicalize().and_then(|path| {
                    path.strip_prefix(&workspace.root)
                        .map(std::path::Path::to_path_buf)
                        .map_err(std::io::Error::other)
                });
                let _ = view.update_in(cx, |this, window, cx| match path {
                    Ok(path) => {
                        let label = path.to_string_lossy();
                        let kind = if label.contains(".tests.") {
                            DocumentKind::Suite
                        } else if label.contains("viewports") {
                            DocumentKind::Viewports
                        } else if matches!(label.as_ref(), "regressify.yaml" | "regressify.yml") {
                            DocumentKind::Workspace
                        } else {
                            DocumentKind::Data
                        };
                        let entry = Entry {
                            path,
                            kind,
                            editable: true,
                        };
                        if !this
                            .session
                            .entries
                            .iter()
                            .any(|existing| existing.path == entry.path)
                        {
                            this.session.entries.push(entry.clone());
                        }
                        this.load_document(entry, window, cx);
                    }
                    Err(error) => {
                        this.status = format!("Select a file inside the workspace: {error}");
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn refresh_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.session.workspace.clone() else {
            return;
        };
        self.task(
            "Refreshing files...",
            move || workspace.entries(),
            |this, entries, _, _| {
                this.session.entries = entries;
                this.status = "Files refreshed".into();
            },
            window,
            cx,
        );
    }
}

pub(super) fn vscode_workspace_url(root: &std::path::Path) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse("vscode://file/")?;
    url.set_path(&display_path(root));
    Ok(url)
}

pub(super) fn folder_viewer_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "Open in File Explorer"
    } else if cfg!(target_os = "macos") {
        "Open in Finder"
    } else {
        "Open in File Manager"
    }
}

pub(super) fn menu_entries() -> Vec<(&'static str, FileCommand)> {
    [
        ("New suite", Operation::New),
        ("Duplicate suite", Operation::Duplicate),
        ("Rename suite", Operation::Rename),
        ("Save As", Operation::SaveAs),
        ("Reload from disk", Operation::Reload),
        ("Delete suite", Operation::Delete),
        ("Create workspace config", Operation::Config),
        ("Create viewport file", Operation::Viewports),
        ("Open configuration file", Operation::Browse),
        ("Open in VS Code", Operation::OpenVSCode),
        (folder_viewer_label(), Operation::OpenFolder),
        ("Refresh files", Operation::Refresh),
        ("Toggle light/dark theme", Operation::Theme),
        ("Close workspace", Operation::CloseWorkspace),
    ]
    .into_iter()
    .map(|(label, operation)| (label, FileCommand { operation }))
    .collect()
}
