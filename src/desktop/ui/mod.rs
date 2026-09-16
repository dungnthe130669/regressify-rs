//! Native workbench. Domain edits stay in the headless desktop models.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use anyhow::{Result, anyhow};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{
        Editor as CodeEditor, EditorState, Input, InputEvent, InputState, Textarea, TextareaState,
    },
    menu::DropdownMenu,
    resizable::{h_resizable, resizable_panel},
    tab::{Tab, TabBar},
    *,
};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use super::{
    document::{ACTIONS, Document, DocumentKind},
    schema::{self, Field, FieldKind},
    session::{Selection, Session},
    workspace::{Entry, OpenDocument, Preferences, display_path},
};

mod choices;
mod design;
mod files;
mod overview;
mod reports;
mod runs;
mod sequence;
mod source;

#[cfg(all(test, feature = "gui-tests"))]
mod tests;

actions!(
    regressify_desktop,
    [
        SaveDocument,
        UndoDocument,
        RedoDocument,
        OpenWorkspace,
        AddScenario,
        AddViewport,
        DuplicateItem,
        MoveItemUp,
        MoveItemDown,
        FocusNavigator,
        FocusSettings,
        ToggleNavigator,
        CompleteSource
    ]
);

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
struct AddBrowserAction {
    kind: String,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
struct ChooseField {
    key: String,
    value: String,
}

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
struct SelectView {
    index: usize,
}

enum Control {
    Input(Entity<InputState>),
    Textarea(Entity<TextareaState>),
    Boolean,
    Choice,
}

struct Binding {
    field: Field,
    control: Control,
}

pub struct Workbench {
    session: Session,
    source: Entity<EditorState>,
    resolved: Entity<EditorState>,
    search: Entity<InputState>,
    bindings: Vec<Binding>,
    _subscriptions: Vec<Subscription>,
    field_subscriptions: Vec<Subscription>,
    field_errors: BTreeMap<String, String>,
    focus: FocusHandle,
    value: Value,
    effective: Value,
    workspace_value: Value,
    tab: usize,
    advanced: bool,
    busy: bool,
    status: String,
    diagnostic: Option<String>,
    source_issue_line: Option<u32>,
    source_wrap: bool,
    source_font_size: f32,
    preferences: Preferences,
    preferences_path: Option<PathBuf>,
    modal: Entity<files::ModalState>,
    runs: Entity<runs::RunPanel>,
    results: Entity<reports::ResultsPanel>,
    main_tab: usize,
    overview: bool,
    expanded_suites: BTreeSet<PathBuf>,
    selected_only: bool,
    field_search: Entity<InputState>,
    overrides_only: bool,
    collapsed_fields: BTreeSet<usize>,
    viewport_choices: choices::ViewportChoices,
}

impl Workbench {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("yaml")
                .replaceable(true)
                .line_number(true)
        });
        let resolved = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
        });
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search suites and scenarios"));
        let source_subscription = cx.subscribe_in(&source, window, |this, state, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                let text = state.read(cx).value().to_string();
                if let Some(open) = &mut this.session.open
                    && open.document.source() != text
                {
                    match open.document.set_source(text) {
                        Ok(()) => {
                            this.field_errors.clear();
                            this.refresh_value();
                            this.update_source_diagnostics(cx);
                        }
                        Err(error) => this.status = format!("{error:#}"),
                    }
                    cx.notify();
                }
            }
        });
        let search_subscription = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        let source_observer = cx.observe(&source, |_, _, cx| cx.notify());
        let field_search = cx.new(|cx| InputState::new(window, cx).placeholder("Find a setting"));
        let field_search_subscription =
            cx.subscribe(&field_search, |_, _, _: &InputEvent, cx| cx.notify());
        let preferences = Preferences::path().and_then(|path| Preferences::load(&path));
        let status = preferences
            .as_ref()
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_else(|| "Ready".into());
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let runs = cx.new(|cx| runs::RunPanel::new(window, cx));
        let run_subscription =
            cx.subscribe_in(&runs, window, |this, _, event, window, cx| match event {
                runs::RunEvent::Requested => this.guard(files::Pending::Run, window, cx),
                runs::RunEvent::Finished => {
                    this.status = "Command finished".into();
                    if let Err(error) = this.session.refresh() {
                        this.status = format!("{error:#}");
                    }
                    this.sync_suites(cx);
                    this.refresh_results(window, cx);
                    cx.notify();
                }
                runs::RunEvent::SelectionChanged | runs::RunEvent::ProgressChanged => cx.notify(),
            });
        Self {
            session: Session::default(),
            source,
            resolved,
            search,
            bindings: Vec::new(),
            _subscriptions: vec![
                source_subscription,
                search_subscription,
                run_subscription,
                field_search_subscription,
                source_observer,
            ],
            field_subscriptions: Vec::new(),
            field_errors: BTreeMap::new(),
            focus,
            value: Value::Null,
            effective: Value::Null,
            workspace_value: Value::Null,
            tab: 0,
            advanced: false,
            busy: false,
            status,
            diagnostic: None,
            source_issue_line: None,
            source_wrap: true,
            source_font_size: 14.,
            preferences: preferences.unwrap_or_default(),
            preferences_path: Preferences::path().ok(),
            modal: cx.new(|_| files::ModalState::default()),
            runs,
            results: cx.new(|cx| reports::ResultsPanel::new(window, cx)),
            main_tab: 0,
            overview: true,
            expanded_suites: BTreeSet::new(),
            selected_only: false,
            field_search,
            overrides_only: false,
            collapsed_fields: BTreeSet::new(),
            viewport_choices: choices::ViewportChoices::default(),
        }
    }

    /// Dispatch workspace loads and saves away from the UI event loop.
    fn task<T: Send + 'static>(
        &mut self,
        message: &str,
        job: impl FnOnce() -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, T, &mut Window, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = message.into();
        self.sync_modal(cx);
        let task = cx.background_executor().spawn(async move { job() });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.busy = false;
                this.sync_modal(cx);
                match result {
                    Ok(value) => done(this, value, window, cx),
                    Err(error) => this.status = format!("{error:#}"),
                }
                this.sync_modal(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn load_workspace(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.runs.read(cx).running() {
            self.status = "Cancel the active command before switching workspaces".into();
            cx.notify();
            return;
        }
        if self.unsaved() {
            self.guard(files::Pending::Workspace(path), window, cx);
            return;
        }
        self.task(
            "Opening workspace...",
            move || {
                let mut session = Session::default();
                session.connect(&path)?;
                Ok(session)
            },
            |this, session, window, cx| {
                this.session = session;
                this.main_tab = 0;
                this.overview = true;
                this.tab = 0;
                this.advanced = false;
                this.expanded_suites.clear();
                this.selected_only = false;
                this.search
                    .update(cx, |state, cx| state.set_value("", window, cx));
                this.results.update(cx, |panel, cx| panel.clear(window, cx));
                if let Some(workspace) = &this.session.workspace {
                    this.preferences.remember(workspace);
                    let root = workspace.root.clone();
                    this.runs
                        .update(cx, |panel, cx| panel.set_workspace(root, window, cx));
                    if let Err(error) = this
                        .preferences_path
                        .as_deref()
                        .ok_or_else(|| anyhow!("Cannot determine the preferences path"))
                        .and_then(|path| this.preferences.save(path))
                    {
                        this.status = format!("Preferences: {error:#}");
                    }
                }
                this.sync_editor(window, cx);
                this.status = "Workspace opened".into();
                this.refresh_results(window, cx);
            },
            window,
            cx,
        );
    }

    fn choose_workspace(&mut self, _: &OpenWorkspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        cx.spawn_in(window, async move |view, cx| {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                let _ = view.update_in(cx, |this, window, cx| {
                    this.load_workspace(folder.path().to_path_buf(), window, cx)
                });
            }
        })
        .detach();
    }

    fn load_document(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.load_document_at(entry, Selection::Root, window, cx);
    }

    fn open_location(
        &mut self,
        entry: Entry,
        selection: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .session
            .open
            .as_ref()
            .is_some_and(|open| open.entry.path == entry.path)
        {
            self.select(selection, window, cx);
        } else {
            self.load_document_at(entry, selection, window, cx);
        }
    }

    fn load_document_at(
        &mut self,
        entry: Entry,
        selection: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.unsaved() {
            self.guard(files::Pending::Location(entry, selection), window, cx);
            return;
        }
        let Some(workspace) = self.session.workspace.clone() else {
            return;
        };
        self.task(
            "Loading document...",
            move || workspace.open_document(entry),
            move |this, open, window, cx| {
                this.reset_field_view(window, cx);
                this.expanded_suites.insert(open.entry.path.clone());
                this.session.open = Some(open);
                this.overview = false;
                this.main_tab = 0;
                this.session.selection = selection;
                this.session.reconcile_selection();
                this.tab = if this
                    .session
                    .open
                    .as_ref()
                    .is_some_and(|open| open.entry.kind == DocumentKind::Data)
                {
                    1
                } else {
                    0
                };
                this.sync_editor(window, cx);
                this.status = "Document loaded".into();
            },
            window,
            cx,
        );
    }

    fn unsaved(&self) -> bool {
        self.session.dirty() || !self.field_errors.is_empty()
    }

    fn refresh_results(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.runs.read(cx).output_root() {
            Ok(root) => self
                .results
                .update(cx, |panel, cx| panel.refresh(root, window, cx)),
            Err(error) => {
                self.status = format!("{error:#}");
                cx.notify();
            }
        }
    }

    fn sync_suites(&mut self, cx: &mut Context<Self>) {
        let suites = self.session.suites.clone();
        let error = self.session.suite_error.clone();
        self.runs
            .update(cx, |panel, cx| panel.update_catalog(suites, error, cx));
    }

    fn start_selected_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        if !self.runs.read(cx).requires_suites() {
            self.runs.update(cx, |panel, cx| panel.start(window, cx));
            return;
        }
        let Some(workspace) = self.session.workspace.clone() else {
            return;
        };
        let selected_count = self.runs.read(cx).selected_count();
        self.task("Validating selected suites...", move || workspace.suite_catalog(), move |this, suites, window, cx| {
            this.session.suites = suites;
            this.session.suite_error = None;
            this.sync_suites(cx);
            if this.runs.read(cx).selected_count() != selected_count {
                this.status = "Selected suites changed or contain errors; review the selection before running".into();
                return;
            }
            this.runs.update(cx, |panel, cx| panel.start(window, cx));
        }, window, cx);
    }

    fn close_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.runs.read(cx).running() || self.busy {
            self.status =
                "Finish or cancel the active operation before closing the workspace".into();
            cx.notify();
            return;
        }
        match self.session.close() {
            Ok(()) => {
                self.runs.update(cx, |panel, cx| panel.clear(window, cx));
                self.results.update(cx, |panel, cx| panel.clear(window, cx));
                self.search
                    .update(cx, |state, cx| state.set_value("", window, cx));
                self.main_tab = 0;
                self.tab = 0;
                self.sync_editor(window, cx);
                self.status = "Workspace closed".into();
            }
            Err(error) => self.status = format!("{error:#}"),
        }
        cx.notify();
    }

    fn select_run_suite(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.runs.read(cx).running() {
            return;
        }
        let Some((workspace, entry)) = self
            .session
            .workspace
            .clone()
            .zip(self.session.open.as_ref().map(|open| open.entry.clone()))
        else {
            return;
        };
        self.task(
            "Checking selected suite...",
            move || workspace.runnable_suite(&entry),
            |this, suite, window, cx| {
                this.status = format!("Run scope: {suite}");
                this.runs
                    .update(cx, |panel, cx| panel.set_suites(vec![suite], window, cx));
                this.main_tab = 1;
                cx.notify();
            },
            window,
            cx,
        );
    }

    fn save_document(&mut self, _: &SaveDocument, window: &mut Window, cx: &mut Context<Self>) {
        self.save_and_continue(None, window, cx);
    }

    fn save_and_continue(
        &mut self,
        pending: Option<files::Pending>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        if let Err(error) = self.flush_drafts(window, cx) {
            self.status = format!("{error:#}");
            self.sync_modal(cx);
            cx.notify();
            return;
        }
        let snapshot = (|| -> Result<_> {
            let workspace = self
                .session
                .workspace
                .clone()
                .ok_or_else(|| anyhow!("Open a workspace first"))?;
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
            Ok((
                workspace,
                OpenDocument {
                    entry: open.entry.clone(),
                    document,
                },
            ))
        })();
        match snapshot {
            Ok((workspace, mut snapshot)) => self.task(
                "Saving...",
                move || {
                    workspace.save(&mut snapshot)?;
                    Ok(snapshot.document.source().to_string())
                },
                move |this, written, window, cx| {
                    if let Some(open) = &mut this.session.open {
                        open.document.acknowledge_saved(written);
                    }
                    this.status = if this.unsaved() {
                        "Saved snapshot; newer edits are still unsaved"
                    } else {
                        "Saved"
                    }
                    .into();
                    if let Some(pending) = pending
                        && !this.unsaved()
                    {
                        window.close_dialog(cx);
                        this.perform_pending(pending, window, cx);
                    }
                },
                window,
                cx,
            ),
            Err(error) => {
                self.status = format!("{error:#}");
                cx.notify();
            }
        }
    }

    fn undo_document(&mut self, _: &UndoDocument, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if let Some(open) = &mut self.session.open {
            open.document.undo();
        }
        self.session.reconcile_selection();
        self.sync_editor(window, cx);
    }

    fn redo_document(&mut self, _: &RedoDocument, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if let Some(open) = &mut self.session.open {
            open.document.redo();
        }
        self.session.reconcile_selection();
        self.sync_editor(window, cx);
    }

    fn edit(
        &mut self,
        operation: impl FnOnce(&mut Session) -> Result<()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        if let Err(error) = self.flush_drafts(window, cx) {
            self.status = format!("{error:#}");
            cx.notify();
            return;
        }
        match operation(&mut self.session) {
            Ok(()) => {
                self.status = "Unsaved changes".into();
                self.sync_editor(window, cx);
            }
            Err(error) => {
                self.status = format!("{error:#}");
                cx.notify();
            }
        }
    }

    fn flush_drafts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<()> {
        let open = self
            .session
            .open
            .as_mut()
            .ok_or_else(|| anyhow!("Open a document first"))?;
        if self.tab == 1 {
            open.document
                .set_source(self.source.read(cx).value().to_string())?;
        } else if self.tab == 0 {
            let parent = self.session.selection.path();
            let current = open.document.value()?;
            let mut edits = Vec::new();
            // Read retained controls directly: change notifications can still be queued.
            for binding in &self.bindings {
                let text = match &binding.control {
                    Control::Input(input) => input.read(cx).value().to_string(),
                    Control::Textarea(input) => input.read(cx).value().to_string(),
                    _ => continue,
                };
                let key = &binding.field.key;
                let value = current
                    .pointer(&parent)
                    .and_then(|value| value.get(key))
                    .unwrap_or(&Value::Null);
                if text == binding.field.display(value) {
                    continue;
                }
                match binding.field.parse(&text) {
                    Ok(value) => {
                        edits.push(super::document::Edit::Set {
                            parent: parent.clone(),
                            key: key.clone(),
                            value: Some(value),
                        });
                        self.field_errors.remove(key);
                    }
                    Err(error) => {
                        self.field_errors.insert(key.clone(), format!("{error:#}"));
                    }
                }
            }
            anyhow::ensure!(
                self.field_errors.is_empty(),
                "Correct invalid fields before saving"
            );
            open.document.apply_many(edits)?;
            let source = open.document.source().to_string();
            self.source
                .update(cx, |state, cx| state.set_value(source, window, cx));
        }
        self.refresh_value();
        self.refresh_effective(window, cx);
        Ok(())
    }

    fn refresh_value(&mut self) {
        self.value = self.session.value().unwrap_or(Value::Null);
        let error = self
            .session
            .open
            .as_ref()
            .and_then(|open| open.document.validate().err());
        self.source_issue_line = error.as_ref().and_then(source::error_line);
        self.diagnostic = error.map(|error| format!("{error:#}"));
    }

    fn sync_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.viewport_choices.invalidate();
        self.session.refresh_catalog();
        self.sync_suites(cx);
        let source = self
            .session
            .open
            .as_ref()
            .map(|open| open.document.source().to_string())
            .unwrap_or_default();
        if self.source.read(cx).value().as_ref() != source {
            self.source
                .update(cx, |state, cx| state.set_value(source, window, cx));
        }
        self.rebuild_fields(window, cx);
        cx.notify();
    }

    fn rebuild_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.configure_source(cx);
        self.refresh_value();
        self.update_source_diagnostics(cx);
        self.bindings.clear();
        self.field_subscriptions.clear();
        self.field_errors.clear();
        self.workspace_value = self
            .session
            .workspace_value()
            .ok()
            .flatten()
            .unwrap_or(Value::Null);
        self.refresh_effective(window, cx);
        let fields = self
            .session
            .scope()
            .and_then(|scope| scope.map(|scope| schema::fields(&scope)).transpose());
        let Ok(Some(fields)) = fields else {
            return;
        };
        for field in fields {
            let value = self
                .value
                .pointer(&self.session.selection.path())
                .and_then(|value| value.get(&field.key))
                .unwrap_or(&Value::Null);
            let text = field.display(value);
            let placeholder = field.display(&self.effective[&schema::effective_key(&field.key)]);
            let control = match &field.kind {
                FieldKind::Boolean => Control::Boolean,
                FieldKind::Choice(_) => Control::Choice,
                FieldKind::List { .. } | FieldKind::Structured => {
                    let state = cx.new(|cx| {
                        TextareaState::new(window, cx)
                            .rows(3)
                            .default_value(text)
                            .placeholder(placeholder)
                    });
                    let descriptor = field.clone();
                    self.field_subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this, state, event, window, cx| {
                            if matches!(event, InputEvent::Change) {
                                this.change_field(
                                    &descriptor,
                                    state.read(cx).value().as_ref(),
                                    window,
                                    cx,
                                );
                            }
                        },
                    ));
                    Control::Textarea(state)
                }
                _ => {
                    let state = cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(text)
                            .placeholder(placeholder)
                    });
                    let descriptor = field.clone();
                    self.field_subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this, state, event, window, cx| {
                            if matches!(event, InputEvent::Change) {
                                this.change_field(
                                    &descriptor,
                                    state.read(cx).value().as_ref(),
                                    window,
                                    cx,
                                );
                            }
                        },
                    ));
                    Control::Input(state)
                }
            };
            self.bindings.push(Binding { field, control });
        }
    }

    fn change_field(
        &mut self,
        field: &Field,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let current = self
            .value
            .pointer(&self.session.selection.path())
            .and_then(|value| value.get(&field.key))
            .unwrap_or(&Value::Null);
        // Programmatic resets emit change events too; an empty inherited control is not an override.
        if !self.field_errors.contains_key(&field.key) && field.display(current) == text {
            return;
        }
        match field
            .parse(text)
            .and_then(|value| self.session.set_field(&field.key, Some(value)))
        {
            Ok(()) => {
                self.field_errors.remove(&field.key);
                self.refresh_value();
                self.refresh_effective(window, cx);
                if let Some(open) = &self.session.open {
                    let source = open.document.source().to_string();
                    self.source
                        .update(cx, |state, cx| state.set_value(source, window, cx));
                }
                self.status = "Unsaved changes".into();
            }
            Err(error) => {
                self.field_errors
                    .insert(field.key.clone(), format!("{error:#}"));
            }
        }
        cx.notify();
    }

    fn reset_field(&mut self, field: &Field, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || field.required {
            return;
        }
        if let Err(error) = self.session.set_field(&field.key, None) {
            self.field_errors
                .insert(field.key.clone(), format!("{error:#}"));
            cx.notify();
            return;
        }
        self.field_errors.remove(&field.key);
        self.refresh_value();
        self.refresh_effective(window, cx);
        if let Some(binding) = self
            .bindings
            .iter()
            .find(|binding| binding.field.key == field.key)
        {
            match &binding.control {
                Control::Input(input) => {
                    input.update(cx, |state, cx| state.set_value("", window, cx))
                }
                Control::Textarea(input) => {
                    input.update(cx, |state, cx| state.set_value("", window, cx))
                }
                _ => {}
            }
        }
        if let Some(open) = &self.session.open {
            let source = open.document.source().to_string();
            self.source
                .update(cx, |state, cx| state.set_value(source, window, cx));
        }
        self.status = "Override removed".into();
        cx.notify();
    }

    fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if let Err(error) = self.flush_drafts(window, cx) {
            self.status = format!("{error:#}");
            cx.notify();
            return;
        }
        match self.session.select(selection) {
            Ok(()) => {
                self.reset_field_view(window, cx);
                if let Some(open) = &self.session.open {
                    self.expanded_suites.insert(open.entry.path.clone());
                }
                self.tab = 0;
                self.overview = false;
                self.main_tab = 0;
                self.rebuild_fields(window, cx);
            }
            Err(error) => self.status = format!("{error:#}"),
        }
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.session.open.as_ref();
        let focus = self.focus.clone();
        let add_scenario = open.is_some_and(|open| open.entry.kind == DocumentKind::Suite);
        let add_viewport = open.is_some_and(|open| open.entry.kind == DocumentKind::Viewports);
        let add_action = self.session.selection.scenario().is_some();
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_2()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("save")
                    .icon(gpui_kit::assets::IconName::Save)
                    .label("Save")
                    .tooltip("Save document")
                    .disabled(
                        self.busy
                            || !self.unsaved()
                            || self.diagnostic.is_some()
                            || !self.field_errors.is_empty(),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.save_document(&SaveDocument, window, cx)
                    })),
            )
            .child(
                Button::new("undo")
                    .ghost()
                    .icon(IconName::Undo)
                    .tooltip("Undo document edit")
                    .disabled(self.busy || !open.is_some_and(|open| open.document.can_undo()))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.undo_document(&UndoDocument, window, cx)
                    })),
            )
            .child(
                Button::new("redo")
                    .ghost()
                    .icon(IconName::Redo)
                    .tooltip("Redo document edit")
                    .disabled(self.busy || !open.is_some_and(|open| open.document.can_redo()))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.redo_document(&RedoDocument, window, cx)
                    })),
            )
            .child(
                Button::new("add-item")
                    .icon(IconName::Plus)
                    .label("Add")
                    .disabled(self.busy || open.is_none())
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu
                            .action_context(focus.clone())
                            .menu_with_disabled("Scenario", Box::new(AddScenario), !add_scenario)
                            .menu_with_disabled("Viewport", Box::new(AddViewport), !add_viewport)
                            .separator();
                        for action in ACTIONS {
                            menu = menu.menu_with_disabled(
                                *action,
                                Box::new(AddBrowserAction {
                                    kind: action.to_string(),
                                }),
                                !add_action,
                            );
                        }
                        menu
                    }),
            )
            .child(
                Button::new("duplicate-item")
                    .ghost()
                    .icon(IconName::Copy)
                    .tooltip("Duplicate selected item")
                    .disabled(self.busy || self.session.selection == Selection::Root)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(Session::duplicate_selected, window, cx)
                    })),
            )
            .child(
                Button::new("remove-item")
                    .ghost()
                    .icon(gpui_kit::assets::IconName::Trash)
                    .tooltip("Remove selected item")
                    .disabled(self.busy || self.session.selection == Selection::Root)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.confirm_remove(false, window, cx)),
                    ),
            )
            .child(
                Button::new("move-up")
                    .ghost()
                    .icon(IconName::ArrowUp)
                    .tooltip("Move selected item up")
                    .disabled(self.busy || self.session.selection == Selection::Root)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(|session| session.move_selected(-1), window, cx)
                    })),
            )
            .child(
                Button::new("move-down")
                    .ghost()
                    .icon(IconName::ArrowDown)
                    .tooltip("Move selected item down")
                    .disabled(self.busy || self.session.selection == Selection::Root)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.edit(|session| session.move_selected(1), window, cx)
                    })),
            )
            .into_any_element()
    }

    fn render_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let mut navigation = div()
            .id("navigator")
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .gap_1()
            .p_3()
            .bg(cx.theme().muted.opacity(0.55))
            .child(
                Button::new("workspace-overview")
                    .ghost()
                    .icon(gpui_kit::assets::IconName::LayoutDashboard)
                    .label("Overview")
                    .selected(self.overview && self.main_tab == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview = true;
                        this.main_tab = 0;
                        cx.notify();
                    })),
            )
            .child(
                Input::new(&self.search)
                    .id("navigator-search")
                    .prefix(IconName::Search)
                    .cleanable(true)
                    .small(),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .py_2()
                    .child(
                        self.session
                            .workspace
                            .as_ref()
                            .and_then(|workspace| workspace.root.file_name())
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "RECENT WORKSPACES".into()),
                    ),
            );
        if self.session.workspace.is_some() {
            navigation = navigation.child(
                TabBar::new("navigator-filter")
                    .segmented()
                    .small()
                    .selected_index(usize::from(self.selected_only))
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.selected_only = *index == 1;
                        cx.notify();
                    }))
                    .child(Tab::new().label("All files"))
                    .child(Tab::new().label("Selected suites")),
            );
            navigation = navigation.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py_2()
                    .child(format!("{} selected", self.runs.read(cx).selected_count()))
                    .child(
                        Checkbox::new("sidebar-select-all")
                            .label("All")
                            .checked(
                                self.runs.read(cx).selected_count() > 0
                                    && self.runs.read(cx).selected_count()
                                        == self
                                            .session
                                            .suites
                                            .iter()
                                            .filter(|suite| suite.error.is_none())
                                            .count(),
                            )
                            .disabled(self.busy || self.runs.read(cx).running())
                            .on_change(cx.listener(|this, checked, _, cx| {
                                this.runs
                                    .update(cx, |panel, cx| panel.select_all(*checked, cx))
                            })),
                    ),
            );
        }
        if self.runs.read(cx).running() {
            let counts = self.runs.read(cx).progress().counts(None);
            navigation =
                navigation.child(div().text_xs().text_color(cx.theme().primary).py_1().child(
                    format!(
                        "{} running  |  {} finished",
                        counts.running,
                        counts.complete()
                    ),
                ));
        }
        if self.session.workspace.is_none() {
            for (index, path) in self.preferences.recent_workspaces.iter().enumerate() {
                let path = path.clone();
                navigation = navigation.child(
                    Button::new(("recent", index))
                        .ghost()
                        .label(
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.load_workspace(path.clone(), window, cx)
                        })),
                );
            }
        }
        let mut files = div()
            .id("workspace-files")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_1();
        let mut visible_files = 0;
        for (index, entry) in self.session.entries.iter().enumerate() {
            let current = self
                .session
                .open
                .as_ref()
                .is_some_and(|open| open.entry.path == entry.path);
            let entry = entry.clone();
            let suite = self
                .session
                .suites
                .iter()
                .find(|suite| suite.entry.path == entry.path);
            if self.selected_only
                && !suite.is_some_and(|suite| self.runs.read(cx).selected(&suite.name))
            {
                continue;
            }
            let file_match = entry.label().to_lowercase().contains(&query);
            let scenario_match = suite.is_some_and(|suite| {
                suite.scenarios.iter().any(|scenario| {
                    [&scenario.label, &scenario.id, &scenario.url]
                        .iter()
                        .any(|value| value.to_lowercase().contains(&query))
                })
            });
            let draft_match = current
                && self
                    .value
                    .get("scenarios")
                    .and_then(Value::as_array)
                    .is_some_and(|scenarios| {
                        scenarios.iter().any(|scenario| {
                            ["label", "id", "url"].iter().any(|key| {
                                scenario[*key]
                                    .as_str()
                                    .is_some_and(|text| text.to_lowercase().contains(&query))
                            })
                        })
                    });
            if !query.is_empty() && !file_match && !scenario_match && !draft_match {
                continue;
            }
            visible_files += 1;
            let expanded = self.expanded_suites.contains(&entry.path) || !query.is_empty();
            let mut row = div().flex().items_center().gap_1();
            if let Some(suite) = suite {
                let path = entry.path.clone();
                row = row.child(
                    Button::new(("expand-suite", index))
                        .ghost()
                        .xsmall()
                        .icon(if expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .tooltip(if expanded {
                            "Collapse scenarios"
                        } else {
                            "Expand scenarios"
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.expanded_suites.remove(&path) {
                                this.expanded_suites.insert(path.clone());
                            }
                            cx.notify();
                        })),
                );
                let name = suite.name.clone();
                row = row.child(
                    Checkbox::new(("sidebar-suite", index))
                        .label("")
                        .checked(self.runs.read(cx).selected(&name))
                        .disabled(
                            self.busy || self.runs.read(cx).running() || suite.error.is_some(),
                        )
                        .on_change(cx.listener(move |this, checked, _, cx| {
                            this.runs
                                .update(cx, |panel, cx| panel.toggle_suite(&name, *checked, cx))
                        })),
                );
            }
            let live_status = suite.and_then(|suite| {
                let counts = self.runs.read(cx).progress().counts(Some(&suite.output_id));
                if counts.running > 0 {
                    Some(("Running", cx.theme().primary))
                } else if counts.failed > 0 {
                    Some(("Failed", cx.theme().danger))
                } else if counts.passed > 0 {
                    Some(("Passed", cx.theme().success))
                } else if counts.queued > 0 {
                    Some(("Queued", cx.theme().muted_foreground))
                } else {
                    None
                }
            });
            files = files.child(
                row.child(
                    Button::new(("file", index))
                        .ghost()
                        .selected(current && self.session.selection == Selection::Root)
                        .child(
                            div().flex_1().min_w_0().text_left().truncate().child(
                                entry
                                    .path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned(),
                            ),
                        )
                        .accessibility_label(
                            entry
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                        )
                        .tooltip(entry.label())
                        .flex_1()
                        .min_w_0()
                        .justify_start()
                        .disabled(self.busy)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this
                                .session
                                .open
                                .as_ref()
                                .is_some_and(|open| open.entry.path == entry.path)
                            {
                                this.select(Selection::Root, window, cx);
                            } else {
                                this.load_document(entry.clone(), window, cx);
                            }
                        })),
                )
                .children(
                    live_status
                        .map(|(status, color)| div().text_xs().text_color(color).child(status)),
                ),
            );
            if current && expanded {
                let items = self.value["scenarios"]
                    .as_array()
                    .or_else(|| self.value.as_array());
                if let Some(items) = items {
                    for (scenario, value) in items.iter().enumerate() {
                        let label = value["label"]
                            .as_str()
                            .or_else(|| value["id"].as_str())
                            .unwrap_or("Untitled")
                            .to_string();
                        let is_viewport = self.value.is_array();
                        let selection = if is_viewport {
                            Selection::Viewport(scenario)
                        } else {
                            Selection::Scenario(scenario)
                        };
                        if query.is_empty()
                            || file_match
                            || label.to_lowercase().contains(&query)
                            || ["id", "url"].iter().any(|key| {
                                value[*key]
                                    .as_str()
                                    .is_some_and(|text| text.to_lowercase().contains(&query))
                            })
                        {
                            files = files.child(
                                Button::new(("scenario", scenario))
                                    .ghost()
                                    .small()
                                    .accessibility_label(label.clone())
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_left()
                                            .truncate()
                                            .child(label),
                                    )
                                    .selected(self.session.selection == selection)
                                    .w_full()
                                    .justify_start()
                                    .pl_5()
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select(selection, window, cx)
                                    })),
                            );
                            if self.session.selection.scenario() == Some(scenario)
                                && let Some(actions) = value["actions"].as_array()
                            {
                                for (index, action) in actions.iter().enumerate() {
                                    let name = ACTIONS
                                        .iter()
                                        .find(|name| action.get(**name).is_some())
                                        .copied()
                                        .unwrap_or("unknown");
                                    let selection = Selection::Action { scenario, index };
                                    files = files.child(
                                        Button::new(("action", index))
                                            .ghost()
                                            .small()
                                            .accessibility_label(format!("{}. {name}", index + 1))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_left()
                                                    .truncate()
                                                    .child(format!("{}. {name}", index + 1)),
                                            )
                                            .w_full()
                                            .justify_start()
                                            .pl_8()
                                            .selected(self.session.selection == selection)
                                            .disabled(self.busy)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.select(selection, window, cx)
                                            })),
                                    );
                                }
                            }
                        }
                    }
                }
            } else if expanded && let Some(suite) = suite {
                for (scenario, details) in suite.scenarios.iter().enumerate() {
                    if !file_match
                        && ![&details.label, &details.id, &details.url]
                            .iter()
                            .any(|text| text.to_lowercase().contains(&query))
                    {
                        continue;
                    }
                    let entry = suite.entry.clone();
                    files = files.child(
                        Button::new(SharedString::from(format!(
                            "scenario:{}:{scenario}",
                            entry.label()
                        )))
                        .ghost()
                        .small()
                        .accessibility_label(details.label.clone())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_left()
                                .truncate()
                                .child(details.label.clone()),
                        )
                        .tooltip(format!("{}\n{}", details.label, details.url))
                        .w_full()
                        .pl_5()
                        .disabled(self.busy)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.open_location(
                                    entry.clone(),
                                    Selection::Scenario(scenario),
                                    window,
                                    cx,
                                )
                            },
                        )),
                    );
                }
            }
        }
        if visible_files == 0 && self.session.workspace.is_some() {
            files = files.child(
                div()
                    .id("navigator-empty")
                    .role(Role::Status)
                    .aria_label("No matches")
                    .test_support()
                    .py_6()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child("No matches"),
            );
        }
        navigation.child(files).into_any_element()
    }

    fn refresh_effective(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .session
            .open
            .as_ref()
            .is_some_and(|open| open.entry.kind == DocumentKind::Workspace)
        {
            self.workspace_value = self.value.clone();
        }
        let suite = self
            .session
            .open
            .as_ref()
            .filter(|open| open.entry.kind == DocumentKind::Suite)
            .map(|_| &self.value);
        let resolved = schema::effective(
            self.workspace_value
                .is_object()
                .then_some(&self.workspace_value),
            suite,
            self.session.selection.scenario(),
        );
        self.effective = resolved.as_ref().cloned().unwrap_or(Value::Null);
        let text = resolved
            .and_then(|value| Ok(serde_json::to_string_pretty(&value)?))
            .unwrap_or_else(|error| format!("{error:#}"));
        self.resolved
            .update(cx, |state, cx| state.set_value(text, window, cx));
        self.load_viewport_choices(window, cx);
    }

    fn field_hint(&self, field: &Field, explicit: bool) -> String {
        if field.required {
            return "Required".into();
        }
        if explicit {
            return "Override".into();
        }
        if matches!(
            self.session.selection,
            Selection::Action { .. } | Selection::Viewport(_)
        ) {
            return "Optional".into();
        }
        let value = &self.effective[&schema::effective_key(&field.key)];
        if value.is_null() {
            return "Optional".into();
        }
        let suite = self
            .session
            .open
            .as_ref()
            .filter(|open| open.entry.kind == DocumentKind::Suite)
            .map(|_| &self.value);
        let scenario = self
            .session
            .selection
            .scenario()
            .and_then(|index| self.value["scenarios"].get(index));
        let origin = schema::origin(&field.key, Some(&self.workspace_value), suite, scenario);
        let text = field.display(value).replace('\n', ", ");
        format!("{origin}: {text}")
    }

    fn reset_field_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field_search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.overrides_only = false;
        self.collapsed_fields.clear();
    }

    fn adjust_field(
        &mut self,
        field: &Field,
        increase: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let result = (|| -> Result<Value> {
            let binding = self
                .bindings
                .iter()
                .find(|binding| binding.field.key == field.key)
                .ok_or_else(|| anyhow!("Field is unavailable"))?;
            let current = if let Control::Input(input) = &binding.control {
                let text = input.read(cx).value();
                if text.is_empty() {
                    self.effective[&schema::effective_key(&field.key)].clone()
                } else {
                    field.parse(&text)?
                }
            } else {
                return Err(anyhow!("Field is not numeric"));
            };
            field.stepped_value(&current, increase)
        })();
        match result {
            Ok(value) => {
                let text = field.display(&value);
                self.change_field(field, &text, window, cx);
                if !self.field_errors.contains_key(&field.key)
                    && let Some(binding) = self
                        .bindings
                        .iter()
                        .find(|binding| binding.field.key == field.key)
                    && let Control::Input(input) = &binding.control
                {
                    input.update(cx, |state, cx| state.set_value(text, window, cx));
                }
            }
            Err(error) => {
                self.field_errors
                    .insert(field.key.clone(), format!("{error:#}"));
                cx.notify();
            }
        }
    }

    fn reveal_problem(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = 0;
        self.overview = false;
        self.overrides_only = false;
        self.field_search
            .update(cx, |state, cx| state.set_value(key.to_string(), window, cx));
        self.collapsed_fields.clear();
        if let Some(binding) = self
            .bindings
            .iter()
            .find(|binding| binding.field.key == key)
        {
            match &binding.control {
                Control::Input(input) => window.focus(&input.read(cx).focus_handle(cx), cx),
                Control::Textarea(input) => window.focus(&input.read(cx).focus_handle(cx), cx),
                _ => {}
            }
        }
        cx.notify();
    }

    fn render_fields(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self
            .value
            .pointer(&self.session.selection.path())
            .unwrap_or(&Value::Null);
        let mut form = div()
            .id("field-form")
            .flex()
            .flex_col()
            .gap_5()
            .p_6()
            .w_full();
        let action = matches!(self.session.selection, Selection::Action { .. });
        let query = self.field_search.read(cx).value().trim().to_lowercase();
        let mut bindings: Vec<_> = self.bindings.iter().enumerate().collect();
        bindings.sort_by_key(|(index, binding)| {
            (
                if action {
                    0
                } else {
                    design::group(&binding.field.key).0
                },
                *index,
            )
        });
        let mut current_group = None;
        let mut section = div().grid().grid_cols(2).gap_4();
        let mut matching = 0;
        for (_, binding) in bindings {
            let field = &binding.field;
            let explicit = selected
                .get(&field.key)
                .is_some_and(|value| !value.is_null());
            let error = self.field_errors.contains_key(&field.key);
            let group = if action {
                (0, "Action properties")
            } else {
                design::group(&field.key)
            };
            if !query.is_empty()
                && ![
                    field.key.as_str(),
                    field.label.as_str(),
                    field.description.as_str(),
                    group.1,
                ]
                .iter()
                .any(|text| text.to_lowercase().contains(&query))
            {
                continue;
            }
            if self.overrides_only && !explicit && !error {
                continue;
            }
            if query.is_empty()
                && !self.overrides_only
                && !explicit
                && !error
                && !self.advanced
                && !field.required
                && self.session.selection == Selection::Root
                && ![
                    "id",
                    "browser",
                    "viewportNames",
                    "misMatchThreshold",
                    "postInteractionWait",
                    "asyncCaptureLimit",
                    "asyncCompareLimit",
                ]
                .contains(&field.key.as_str())
            {
                continue;
            }
            matching += 1;
            if current_group != Some(group.0) {
                if current_group.is_some() {
                    form = form.child(section);
                    if current_group == Some(0)
                        && query.is_empty()
                        && !self.overrides_only
                        && let Selection::Scenario(scenario) = self.session.selection
                    {
                        form = form.child(self.render_sequence(scenario, cx));
                    }
                }
                let collapsed = self.collapsed_fields.contains(&group.0) && query.is_empty();
                form = form.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .pt_1()
                        .child(
                            Button::new(("field-group", group.0))
                                .ghost()
                                .small()
                                .icon(if collapsed {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                })
                                .label(group.1)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.collapsed_fields.remove(&group.0) {
                                        this.collapsed_fields.insert(group.0);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(div().flex_1().h(px(1.)).bg(cx.theme().border)),
                );
                section = div().grid().grid_cols(2).gap_4();
                current_group = Some(group.0);
            }
            if self.collapsed_fields.contains(&group.0) && query.is_empty() && !error {
                continue;
            }
            let value = selected.get(&field.key).unwrap_or(&Value::Null);
            let explicit = !value.is_null();
            let field_id = SharedString::from(format!("field-{}", field.key));
            let wide = matches!(field.kind, FieldKind::List { .. } | FieldKind::Structured)
                || matches!(field.key.as_str(), "url" | "description")
                || action;
            let mut row = div()
                .flex()
                .flex_col()
                .gap_2()
                .w_full()
                .min_w_0()
                .when(wide, |row| row.col_span(2));
            let key = field.key.clone();
            let reset = field.clone();
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!(
                                "{}{}",
                                field.label,
                                if field.required { " *" } else { "" }
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .id(SharedString::from(format!("hint-{}", field.key)))
                                    .role(Role::Status)
                                    .aria_label(self.field_hint(field, explicit))
                                    .test_support()
                                    .max_w(px(if wide { 300. } else { 150. }))
                                    .truncate()
                                    .child(self.field_hint(field, explicit)),
                            )
                            .child(
                                Button::new(SharedString::from(format!("reset-{key}")))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Undo)
                                    .tooltip("Use inherited value")
                                    .disabled(
                                        self.busy
                                            || field.required
                                            || (!explicit && !self.field_errors.contains_key(&key)),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.reset_field(&reset, window, cx)
                                    })),
                            ),
                    ),
            );
            let control = match &binding.control {
                Control::Input(state) => Input::new(state)
                    .id(field_id)
                    .small()
                    .disabled(self.busy)
                    .when(matches!(field.kind, FieldKind::Number { .. }), |input| {
                        let decrease = field.clone();
                        let increase = field.clone();
                        input.suffix(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "decrease-{}",
                                        field.key
                                    )))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Minus)
                                    .tooltip(format!("Decrease {}", field.label))
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.adjust_field(&decrease, false, window, cx)
                                    })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "increase-{}",
                                        field.key
                                    )))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Plus)
                                    .tooltip(format!("Increase {}", field.label))
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.adjust_field(&increase, true, window, cx)
                                    })),
                                ),
                        )
                    })
                    .when(field.key.ends_with("Path"), |input| {
                        let key = field.key.clone();
                        input.suffix(
                            Button::new(SharedString::from(format!("browse-{key}")))
                                .ghost()
                                .xsmall()
                                .icon(IconName::FolderOpen)
                                .tooltip("Choose a workspace file")
                                .disabled(self.busy)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.choose_field_file(key.clone(), window, cx)
                                })),
                        )
                    })
                    .into_any_element(),
                Control::Textarea(state) => div()
                    .id(field_id)
                    .child(Textarea::new(state).disabled(self.busy).h(px(72.)))
                    .into_any_element(),
                Control::Boolean => {
                    let descriptor = field.clone();
                    let checked = value
                        .as_bool()
                        .or_else(|| self.effective[&schema::effective_key(&field.key)].as_bool())
                        .unwrap_or(false);
                    Checkbox::new(field_id)
                        .checked(checked)
                        .label(field.label.clone())
                        .disabled(self.busy)
                        .on_change(cx.listener(move |this, checked, window, cx| {
                            this.change_field(
                                &descriptor,
                                if *checked { "true" } else { "false" },
                                window,
                                cx,
                            )
                        }))
                        .into_any_element()
                }
                Control::Choice => {
                    let choices = if let FieldKind::Choice(choices) = &field.kind {
                        choices.clone()
                    } else {
                        Vec::new()
                    };
                    let key = field.key.clone();
                    let focus = self.focus.clone();
                    Button::new(field_id)
                        .label(
                            value
                                .as_str()
                                .or_else(|| self.effective[&field.key].as_str())
                                .unwrap_or("Inherited")
                                .to_string(),
                        )
                        .disabled(self.busy)
                        .dropdown_menu(move |menu, _, _| {
                            let mut menu = menu.action_context(focus.clone());
                            for value in &choices {
                                menu = menu.menu(
                                    value.clone(),
                                    Box::new(ChooseField {
                                        key: key.clone(),
                                        value: value.clone(),
                                    }),
                                );
                            }
                            menu
                        })
                        .into_any_element()
                }
            };
            row = row.child(control);
            if let Some(choices) = self.render_list_choices(field, value, cx) {
                row = row.child(choices);
            }
            if field.key == "file" {
                let key = field.key.clone();
                row = row.child(
                    Button::new(SharedString::from(format!("browse-{key}")))
                        .ghost()
                        .small()
                        .icon(IconName::FolderOpen)
                        .tooltip("Choose a workspace file")
                        .disabled(self.busy)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.choose_field_file(key.clone(), window, cx)
                        })),
                );
            }
            if let Some(error) = self.field_errors.get(&field.key) {
                row = row.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error.clone()),
                );
            }
            section = section.child(row);
        }
        if current_group.is_some() {
            form = form.child(section);
        }
        if query == "actions"
            && let Selection::Scenario(scenario) = self.session.selection
        {
            form = form.child(self.render_sequence(scenario, cx));
            matching += 1;
        }
        if matching == 0 {
            form = form.child(div().text_color(cx.theme().muted_foreground).child(
                if self.session.open.is_some() {
                    "No matching settings"
                } else {
                    "No document open"
                },
            ));
        }
        div()
            .id("field-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(form)
            .into_any_element()
    }

    fn render_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.overview || self.session.open.is_none() {
            return self.render_overview(cx);
        }
        let title = self
            .session
            .open
            .as_ref()
            .map(|open| {
                format!(
                    "{}{}",
                    open.entry.label(),
                    if self.unsaved() { " *" } else { "" }
                )
            })
            .unwrap_or_else(|| "Workspace".into());
        let content = match self.tab {
            1 => self.render_source(cx),
            2 => div()
                .id("effective-editor")
                .size_full()
                .child(
                    CodeEditor::new(&self.resolved)
                        .aria_label("Effective configuration")
                        .size_full()
                        .readonly(true),
                )
                .into_any_element(),
            _ => self.render_fields(cx),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .px_4()
                    .py_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                Button::new("breadcrumb-document")
                                    .ghost()
                                    .xsmall()
                                    .accessibility_label(title.clone())
                                    .tooltip(title.clone())
                                    .min_w_0()
                                    .max_w(px(520.))
                                    .child(div().truncate().child(title))
                                    .disabled(self.busy)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.select(Selection::Root, window, cx)
                                    })),
                            )
                            .when_some(
                                self.session.selection.scenario().filter(|_| {
                                    matches!(self.session.selection, Selection::Action { .. })
                                }),
                                |row, scenario| {
                                    row.child(Icon::new(IconName::ChevronRight).xsmall()).child(
                                        Button::new("breadcrumb-scenario")
                                            .ghost()
                                            .xsmall()
                                            .min_w_0()
                                            .max_w(px(360.))
                                            .accessibility_label("Parent scenario")
                                            .tooltip("Open parent scenario")
                                            .child(
                                                div().truncate().child(
                                                    self.value["scenarios"][scenario]["label"]
                                                        .as_str()
                                                        .unwrap_or("Scenario")
                                                        .to_string(),
                                                ),
                                            )
                                            .disabled(self.busy)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.select(
                                                    Selection::Scenario(scenario),
                                                    window,
                                                    cx,
                                                )
                                            })),
                                    )
                                },
                            ),
                    )
                    .child(
                        div()
                            .id("editor-heading")
                            .role(Role::Heading)
                            .aria_label(self.selection_title())
                            .test_support()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(self.selection_title()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .px_5()
                    .bg(cx.theme().muted.opacity(0.45))
                    .child(
                        TabBar::new("editor-tabs")
                            .underline()
                            .selected_index(self.tab)
                            .on_click(cx.listener(|this, index, window, cx| {
                                if !this.field_errors.is_empty() {
                                    return;
                                }
                                this.tab = *index;
                                this.session.reconcile_selection();
                                this.rebuild_fields(window, cx);
                                cx.notify();
                            }))
                            .child(Tab::new().label("Fields"))
                            .child(Tab::new().label("Source"))
                            .child(Tab::new().label("Effective")),
                    )
                    .child(
                        Checkbox::new("advanced-fields")
                            .label("Advanced")
                            .checked(self.advanced)
                            .on_change(cx.listener(|this, checked, _, cx| {
                                this.advanced = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.tab == 0, |editor| {
                editor.child(
                    div()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .gap_3()
                        .px_5()
                        .py_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            Input::new(&self.field_search)
                                .id("field-search")
                                .prefix(IconName::Search)
                                .cleanable(true)
                                .small()
                                .w(px(250.)),
                        )
                        .child(
                            Checkbox::new("overrides-only")
                                .label("Overrides only")
                                .checked(self.overrides_only)
                                .on_change(cx.listener(|this, checked, _, cx| {
                                    this.overrides_only = *checked;
                                    cx.notify();
                                })),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("expand-fields")
                                .ghost()
                                .xsmall()
                                .icon(gpui_kit::assets::IconName::ChevronsUpDown)
                                .tooltip("Expand all settings")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.collapsed_fields.clear();
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(!self.field_errors.is_empty(), |editor| {
                editor.child(
                    div()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .gap_2()
                        .px_5()
                        .py_2()
                        .bg(cx.theme().danger.opacity(0.07))
                        .child(
                            Icon::new(gpui_kit::assets::IconName::CircleAlert)
                                .small()
                                .text_color(cx.theme().danger),
                        )
                        .child(format!(
                            "{} field {}",
                            self.field_errors.len(),
                            if self.field_errors.len() == 1 {
                                "issue"
                            } else {
                                "issues"
                            }
                        ))
                        .children(self.field_errors.keys().map(|key| {
                            let key = key.clone();
                            Button::new(SharedString::from(format!("problem-{key}")))
                                .ghost()
                                .small()
                                .label(key.clone())
                                .text_color(cx.theme().danger)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.reveal_problem(&key, window, cx)
                                }))
                        })),
                )
            })
            .child(div().flex_1().min_h_0().child(content))
            .children(self.diagnostic.as_ref().map(|diagnostic| {
                div()
                    .id("diagnostic")
                    .role(Role::Alert)
                    .aria_label(diagnostic.clone())
                    .test_support()
                    .max_h(px(92.))
                    .overflow_y_scroll()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(diagnostic.clone())
            }))
            .into_any_element()
    }

    fn selection_title(&self) -> String {
        match self.session.selection {
            Selection::Root => self
                .session
                .open
                .as_ref()
                .and_then(|open| open.entry.suite_name())
                .unwrap_or_else(|| "Workspace settings".into()),
            Selection::Scenario(index) => self.value["scenarios"][index]["label"]
                .as_str()
                .or_else(|| self.value["scenarios"][index]["id"].as_str())
                .unwrap_or("Scenario")
                .into(),
            Selection::Action { scenario, index } => {
                let action = &self.value["scenarios"][scenario]["actions"][index];
                let kind = ACTIONS
                    .iter()
                    .find(|kind| action.get(**kind).is_some())
                    .copied()
                    .unwrap_or("Action");
                format!("{}. {}", index + 1, kind)
            }
            Selection::Viewport(index) => self.value[index]["label"]
                .as_str()
                .unwrap_or("Viewport")
                .into(),
        }
    }

    fn save_preferences(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let preferences = self.preferences.clone();
        let path = self.preferences_path.clone();
        self.task(
            "Saving preferences...",
            move || {
                preferences
                    .save(&path.ok_or_else(|| anyhow!("Cannot determine the preferences path"))?)
            },
            |this, (), _, _| this.status = "Preferences saved".into(),
            window,
            cx,
        );
    }

    fn toggle_navigator(
        &mut self,
        _: &ToggleNavigator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.preferences.navigator_hidden = !self.preferences.navigator_hidden;
        if self.preferences.navigator_hidden {
            window.focus(&self.focus, cx);
        }
        self.save_preferences(window, cx);
    }

    fn select_view(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index > 2 {
            return;
        }
        let changed = self.main_tab != index;
        self.main_tab = index;
        window.focus(&self.focus, cx);
        if changed && index == 2 {
            self.refresh_results(window, cx);
        }
        cx.notify();
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = if self.main_tab == 2 {
            self.results.clone().into_any_element()
        } else if self.main_tab == 1 {
            self.runs.clone().into_any_element()
        } else {
            self.render_editor(cx)
        };
        let layout = if self.preferences.navigator_hidden {
            editor
        } else {
            h_resizable("workspace-layout")
                .child(
                    resizable_panel()
                        .size(px(280.))
                        .size_range(px(220.)..px(440.))
                        .child(
                            div()
                                .id("workspace-navigation")
                                .test_support()
                                .size_full()
                                .child(self.render_navigation(cx)),
                        ),
                )
                .child(resizable_panel().child(editor))
                .into_any_element()
        };
        div()
            .id("workbench")
            .key_context("Regressify")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .on_action(cx.listener(Self::save_document))
            .on_action(cx.listener(Self::undo_document))
            .on_action(cx.listener(Self::redo_document))
            .on_action(cx.listener(Self::choose_workspace))
            .on_action(cx.listener(|this, _: &FocusNavigator, window, cx| {
                if this.preferences.navigator_hidden {
                    this.toggle_navigator(&ToggleNavigator, window, cx);
                }
                cx.notify();
                window.focus(&this.search.read(cx).focus_handle(cx), cx)
            }))
            .on_action(cx.listener(Self::toggle_navigator))
            .on_action(cx.listener(|this, action: &SelectView, window, cx| {
                this.select_view(action.index, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusSettings, window, cx| {
                if this.session.open.is_none() {
                    return;
                }
                this.select_view(0, window, cx);
                this.overview = false;
                if this.tab != 0 && this.field_errors.is_empty() {
                    this.rebuild_fields(window, cx);
                }
                this.tab = 0;
                window.focus(&this.field_search.read(cx).focus_handle(cx), cx);
                cx.notify();
            }))
            .on_action(cx.listener(Self::file_command))
            .on_action(cx.listener(Self::toggle_list_choice))
            .on_action(cx.listener(|this, _: &AddScenario, window, cx| {
                this.edit(Session::add_scenario, window, cx)
            }))
            .on_action(cx.listener(|this, _: &AddViewport, window, cx| {
                this.edit(Session::add_viewport, window, cx)
            }))
            .on_action(cx.listener(|this, _: &DuplicateItem, window, cx| {
                this.edit(Session::duplicate_selected, window, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveItemUp, window, cx| {
                this.edit(|session| session.move_selected(-1), window, cx)
            }))
            .on_action(cx.listener(|this, _: &MoveItemDown, window, cx| {
                this.edit(|session| session.move_selected(1), window, cx)
            }))
            .on_action(cx.listener(|this, action: &AddBrowserAction, window, cx| {
                this.edit(|session| session.add_action(&action.kind), window, cx)
            }))
            .on_action(cx.listener(|this, action: &ChooseField, window, cx| {
                this.edit(
                    |session| session.set_field(&action.key, Some(json!(action.value))),
                    window,
                    cx,
                )
            }))
            .font_family(cx.theme().font_family.clone())
            .child(self.render_header(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("toggle-navigation")
                            .ghost()
                            .small()
                            .icon(if self.preferences.navigator_hidden {
                                gpui_kit::assets::IconName::PanelLeftOpen
                            } else {
                                gpui_kit::assets::IconName::PanelLeftClose
                            })
                            .tooltip(if self.preferences.navigator_hidden {
                                "Show navigator"
                            } else {
                                "Hide navigator"
                            })
                            .accessibility_label(if self.preferences.navigator_hidden {
                                "Show navigator"
                            } else {
                                "Hide navigator"
                            })
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_navigator(&ToggleNavigator, window, cx)
                            })),
                    )
                    .child(
                        TabBar::new("workbench-tabs")
                            .underline()
                            .selected_index(self.main_tab)
                            .on_click(cx.listener(|this, index, window, cx| {
                                this.select_view(*index, window, cx);
                            }))
                            .child(Tab::new().label("Editor"))
                            .child(Tab::new().label("Runs"))
                            .child(Tab::new().label("Results")),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("run-suite")
                            .ghost()
                            .small()
                            .icon(gpui_kit::assets::IconName::Play)
                            .label("Run suite")
                            .disabled(
                                self.busy
                                    || self
                                        .session
                                        .open
                                        .as_ref()
                                        .and_then(|open| open.entry.suite_name())
                                        .is_none(),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select_run_suite(window, cx);
                            })),
                    ),
            )
            .child(div().flex_1().min_h_0().child(layout))
            .child(
                div()
                    .id("status")
                    .role(Role::Status)
                    .aria_label(self.status.clone())
                    .test_support()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_1().truncate().child(self.status.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(if self.busy {
                                "Working"
                            } else if self.unsaved() {
                                "Modified"
                            } else {
                                "Ready"
                            }),
                    ),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

pub fn launch(root: Option<PathBuf>) {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let dark = Preferences::path()
                .and_then(|path| Preferences::load(&path))
                .map(|preferences| preferences.dark_mode)
                .unwrap_or(false);
            design::apply_theme(dark, None, cx);
            bind_keys(cx);
            let bounds = Bounds::centered(None, size(px(1320.), px(840.)), cx);
            cx.spawn(async move |cx| {
                let result = cx.open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(1000.), px(640.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Regressify Desktop".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    |window, cx| {
                        let view = cx.new(|cx| Workbench::new(window, cx));
                        let weak = view.downgrade();
                        window.on_window_should_close(cx, move |window, cx| {
                            weak.update(cx, |this, cx| {
                                if this.busy {
                                    this.status =
                                        "Finish the current operation before closing".into();
                                    cx.notify();
                                    false
                                } else if this.runs.read(cx).running() {
                                    this.runs.update(cx, |panel, cx| panel.cancel(cx));
                                    this.status =
                                        "Cancelling the active run; close again after it finishes"
                                            .into();
                                    cx.notify();
                                    false
                                } else if this.unsaved() {
                                    this.guard(files::Pending::Close, window, cx);
                                    false
                                } else {
                                    cx.quit();
                                    true
                                }
                            })
                            .unwrap_or(true)
                        });
                        if let Some(root) = root {
                            view.update(cx, |this, cx| this.load_workspace(root, window, cx));
                        }
                        cx.new(|cx| Root::new(view, window, cx))
                    },
                );
                if let Err(error) = result {
                    eprintln!("Unable to open Regressify Desktop: {error:#}");
                }
            })
            .detach();
        });
}

fn bind_keys(cx: &mut App) {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.bind_keys([
        KeyBinding::new("ctrl-space", CompleteSource, Some("RegressifySource")),
        KeyBinding::new(&format!("{modifier}-p"), FocusNavigator, Some("Regressify")),
        KeyBinding::new(
            &format!("{modifier}-b"),
            ToggleNavigator,
            Some("Regressify"),
        ),
        KeyBinding::new(
            &format!("{modifier}-shift-f"),
            FocusSettings,
            Some("Regressify"),
        ),
        KeyBinding::new(
            &format!("{modifier}-1"),
            SelectView { index: 0 },
            Some("Regressify"),
        ),
        KeyBinding::new(
            &format!("{modifier}-2"),
            SelectView { index: 1 },
            Some("Regressify"),
        ),
        KeyBinding::new(
            &format!("{modifier}-3"),
            SelectView { index: 2 },
            Some("Regressify"),
        ),
        KeyBinding::new(&format!("{modifier}-s"), SaveDocument, Some("Regressify")),
        KeyBinding::new(&format!("{modifier}-o"), OpenWorkspace, Some("Regressify")),
        KeyBinding::new(&format!("{modifier}-z"), UndoDocument, Some("Regressify")),
        KeyBinding::new(
            &format!("{modifier}-shift-z"),
            RedoDocument,
            Some("Regressify"),
        ),
        KeyBinding::new(
            &format!("{modifier}-n"),
            files::FileCommand {
                operation: files::Operation::New,
            },
            Some("Regressify"),
        ),
        KeyBinding::new(
            &format!("{modifier}-shift-s"),
            files::FileCommand {
                operation: files::Operation::SaveAs,
            },
            Some("Regressify"),
        ),
        KeyBinding::new(
            &format!("{modifier}-r"),
            files::FileCommand {
                operation: files::Operation::Reload,
            },
            Some("Regressify"),
        ),
        KeyBinding::new(
            "f2",
            files::FileCommand {
                operation: files::Operation::Rename,
            },
            Some("Regressify"),
        ),
        KeyBinding::new(
            "f5",
            files::FileCommand {
                operation: files::Operation::Refresh,
            },
            Some("Regressify"),
        ),
    ]);
}
