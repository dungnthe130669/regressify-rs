//! Run state is independent of document fields so log updates cannot disturb drafts.

use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use super::*;
use crate::desktop::runner::{RunCommand, RunManager, RunOptions, RunOutcome, RunRequest};
use crate::desktop::workspace::{SuiteSelection, SuiteSummary};

mod activity;
mod artifacts;
mod log;
mod settings;

#[derive(Clone, Debug)]
pub(super) enum RunEvent {
    Requested(Box<RunIntent>),
    Finished,
    SelectionChanged,
    ArtifactsChanged,
    ProgressChanged,
    ReviewResults {
        root: PathBuf,
        run_id: String,
    },
    ReviewSavedResults {
        root: PathBuf,
        targets: Vec<(String, String)>,
    },
}

#[derive(Clone, Debug)]
pub(super) struct RunIntent {
    pub request: RunRequest,
    pub suite_names: Vec<String>,
    pub expected_output_ids: Option<Vec<String>>,
    pub review_after_run: bool,
}

impl RunIntent {
    pub(super) fn resolve_for_execution(&self, suites: &[SuiteSummary]) -> Result<RunRequest> {
        let request = self.resolve(suites)?;
        if request.command == RunCommand::Test {
            let output = request.options.output_directory(&request.root);
            for name in &self.suite_names {
                let suite = suites
                    .iter()
                    .find(|suite| &suite.name == name)
                    .ok_or_else(|| anyhow!("Selected suite '{name}' is unavailable"))?;
                let artifacts =
                    crate::desktop::results::SuiteArtifacts::load(&output, &suite.output_id)?;
                anyhow::ensure!(
                    artifacts.reference_count > 0,
                    "No reference images for '{name}'. Run Reference before Run Test."
                );
            }
        }
        Ok(request)
    }

    pub(super) fn compare_run(
        mut request: RunRequest,
        executed_suites: &[(String, String)],
        review_after_run: bool,
    ) -> Result<Self> {
        if matches!(request.command, RunCommand::Compare | RunCommand::Approve) {
            for filter in &mut request.options.scenarios {
                let (name, _) = executed_suites
                    .iter()
                    .find(|(_, id)| id == &filter.suite)
                    .ok_or_else(|| {
                        anyhow!(
                            "The recorded scenario's suite is unavailable; review the run selection"
                        )
                    })?;
                filter.suite = name.clone();
            }
        }
        request.command = RunCommand::Compare;
        Ok(Self {
            request,
            suite_names: executed_suites
                .iter()
                .map(|(name, _)| name.clone())
                .collect(),
            expected_output_ids: Some(executed_suites.iter().map(|(_, id)| id.clone()).collect()),
            review_after_run,
        })
    }

    pub(super) fn resolve(&self, suites: &[SuiteSummary]) -> Result<RunRequest> {
        let mut request = self.request.clone();
        if request.command.supports_suites() {
            let mut selection = SuiteSelection::default();
            for name in &self.suite_names {
                let suite = suites
                    .iter()
                    .find(|suite| suite.name == *name)
                    .ok_or_else(|| anyhow!("Selected suite '{name}' no longer exists"))?;
                selection.toggle(suite, true)?;
            }
            request.options.suites = selection.filters(
                suites,
                matches!(request.command, RunCommand::Compare | RunCommand::Approve),
            )?;
            for filter in &mut request.options.scenarios {
                anyhow::ensure!(
                    self.suite_names.contains(&filter.suite),
                    "Selected scenario '{filter}' is outside the requested suites"
                );
                let suite = suites
                    .iter()
                    .find(|suite| suite.name == filter.suite)
                    .ok_or_else(|| anyhow!("Selected suite '{}' no longer exists", filter.suite))?;
                anyhow::ensure!(
                    suite
                        .scenarios
                        .iter()
                        .filter(|scenario| scenario.id == filter.scenario)
                        .count()
                        == 1,
                    "Selected scenario '{filter}' changed or no longer exists; review the selection before running"
                );
                if matches!(request.command, RunCommand::Compare | RunCommand::Approve) {
                    filter.suite = suite.output_id.clone();
                }
            }
            if let Some(expected) = &self.expected_output_ids {
                anyhow::ensure!(
                    request.options.suites
                        == expected
                            .iter()
                            .map(|id| glob::Pattern::escape(id))
                            .collect::<Vec<_>>(),
                    "The report's suite output changed; refresh results before approving"
                );
            }
        }
        request.arguments()?;
        Ok(request)
    }
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
    settings_search: Entity<InputState>,
    _settings_subscription: Subscription,
    pub(super) manager: RunManager,
    log: Entity<EditorState>,
    focus: FocusHandle,
    status: String,
    tab: usize,
    disabled: bool,
    confirmation: bool,
    pending_request: Option<RunIntent>,
    log_text: String,
    log_scope: Option<log::ScenarioLogScope>,
    suites: Vec<SuiteSummary>,
    selection: SuiteSelection,
    catalog_error: Option<String>,
    collapsed: BTreeSet<String>,
    expanded_selections: BTreeSet<String>,
    failed_only: bool,
    started: Option<Instant>,
    executed_suites: Vec<(String, String)>,
    artifacts: artifacts::ArtifactScan,
}

impl EventEmitter<RunEvent> for RunPanel {}

impl RunPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Find a run setting"));
        let settings_subscription =
            cx.subscribe(&settings_search, |_, _, _: &InputEvent, cx| cx.notify());
        let mut panel = Self {
            root: None,
            command: RunCommand::Test,
            options: serde_json::to_value(RunOptions::default()).unwrap_or(Value::Null),
            bindings: Vec::new(),
            errors: BTreeMap::new(),
            subscriptions: Vec::new(),
            settings_search,
            _settings_subscription: settings_subscription,
            manager: RunManager::default(),
            log: cx.new(|cx| EditorState::new(window, cx).line_number(false)),
            focus: cx.focus_handle(),
            status: "No command running".into(),
            tab: 0,
            disabled: false,
            confirmation: false,
            pending_request: None,
            log_text: String::new(),
            log_scope: None,
            suites: Vec::new(),
            selection: SuiteSelection::default(),
            catalog_error: None,
            collapsed: BTreeSet::new(),
            expanded_selections: BTreeSet::new(),
            failed_only: false,
            started: None,
            executed_suites: Vec::new(),
            artifacts: artifacts::ArtifactScan::default(),
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
    fn can_open_report(&self) -> bool {
        self.can_open_saved_selection() || self.can_open_executed_report()
    }
    fn can_open_executed_report(&self) -> bool {
        (self.selection.is_empty()
            || self.selected_names()
                == self
                    .executed_suites
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>())
            && self.executed_report_available()
    }
    fn executed_report_available(&self) -> bool {
        !self.running()
            && self
                .manager
                .request
                .as_ref()
                .is_some_and(|request| request.command.supports_suites())
            && (matches!(self.manager.outcome, Some(RunOutcome::Completed))
                || matches!(self.manager.outcome, Some(RunOutcome::Failed(_)))
                    && self.manager.progress.comparison_counts(None).total > 0)
    }
    fn open_report(&mut self, cx: &mut Context<Self>) {
        if self.disabled || !self.can_open_report() {
            return;
        }
        if self.can_open_saved_selection() {
            self.open_saved_reports(self.selected_names(), cx);
            return;
        }
        self.confirmation = false;
        self.pending_request = None;
        if self.manager.request.as_ref().is_some_and(|request| {
            request.command == RunCommand::Test && request.options.skip_compare
        }) {
            self.compare_completed_run(true, cx);
        } else {
            self.review_results(cx);
        }
        cx.notify();
    }
    fn review_results(&self, cx: &mut Context<Self>) {
        if let Some(request) = &self.manager.request {
            let root = request.options.output_directory(&request.root);
            if !request.options.run_id.is_empty() && !self.executed_suites.is_empty() {
                cx.emit(RunEvent::ReviewSavedResults {
                    root,
                    targets: self
                        .executed_suites
                        .iter()
                        .map(|(_, id)| (id.clone(), request.options.run_id.clone()))
                        .collect(),
                });
            } else {
                cx.emit(RunEvent::ReviewResults {
                    root,
                    run_id: request.options.run_id.clone(),
                });
            }
        }
    }
    fn compare_completed_run(&mut self, review_after_run: bool, cx: &mut Context<Self>) {
        if self.running() || self.disabled {
            return;
        }
        let Some(request) = self.manager.request.clone() else {
            return;
        };
        let result = RunIntent::compare_run(request, &self.executed_suites, review_after_run)
            .and_then(|intent| {
                intent.resolve(&self.suites)?;
                Ok(intent)
            });
        match result {
            Ok(intent) => cx.emit(RunEvent::Requested(Box::new(intent))),
            Err(error) => {
                self.status = format!("{error:#}");
                cx.notify();
            }
        }
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
        self.refresh_artifacts(cx);
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
        self.pending_request = None;
        self.options = serde_json::to_value(RunOptions::default()).unwrap_or(Value::Null);
        self.manager = RunManager::default();
        self.command = RunCommand::Test;
        self.settings_search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.selection.clear();
        self.suites.clear();
        self.catalog_error = None;
        self.log_text.clear();
        self.log_scope = None;
        self.collapsed.clear();
        self.expanded_selections.clear();
        self.failed_only = false;
        self.started = None;
        self.executed_suites.clear();
        self.artifacts.clear();
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
        self.refresh_artifacts(cx);
        cx.emit(RunEvent::SelectionChanged);
        cx.notify();
    }

    pub(super) fn selected(&self, name: &str) -> bool {
        self.selection.contains(name)
    }
    pub(super) fn selected_count(&self) -> usize {
        self.selection.len()
    }
    pub(super) fn all_selected(&self, name: &str) -> bool {
        self.suites
            .iter()
            .find(|suite| suite.name == name)
            .is_some_and(|suite| self.selection.all_selected(suite))
    }
    pub(super) fn selected_scenario_count(&self) -> usize {
        self.suites
            .iter()
            .map(|suite| self.selection.scenario_count(suite))
            .sum()
    }
    pub(super) fn scenario_selected(&self, name: &str, id: &str) -> bool {
        self.selection.scenario_selected(name, id)
    }
    pub(super) fn set_scenario(
        &mut self,
        name: &str,
        id: &str,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        anyhow::ensure!(
            !self.running() && !self.disabled,
            "Another operation is already running"
        );
        let suite = self
            .suites
            .iter()
            .find(|suite| suite.name == name)
            .ok_or_else(|| anyhow!("Selected suite is no longer available"))?;
        let mut selection = SuiteSelection::default();
        selection.toggle_scenario(suite, id, true)?;
        self.selection = selection;
        self.confirmation = false;
        cx.emit(RunEvent::SelectionChanged);
        cx.notify();
        Ok(())
    }
    pub(super) fn update_draft_suite(&mut self, suite: SuiteSummary, cx: &mut Context<Self>) {
        if self.running() || self.disabled {
            return;
        }
        if let Some(current) = self
            .suites
            .iter_mut()
            .find(|current| current.entry.path == suite.entry.path)
        {
            *current = suite;
            self.selection.reconcile(&self.suites);
            self.confirmation = false;
            cx.emit(RunEvent::SelectionChanged);
            cx.notify();
        }
    }
    pub(super) fn toggle_scenario(
        &mut self,
        name: &str,
        id: &str,
        checked: bool,
        cx: &mut Context<Self>,
    ) {
        if self.running() || self.disabled {
            return;
        }
        if let Some(suite) = self.suites.iter().find(|suite| suite.name == name) {
            if let Err(error) = self.selection.toggle_scenario(suite, id, checked) {
                self.status = format!("{error:#}");
            }
            self.confirmation = false;
            cx.emit(RunEvent::SelectionChanged);
            cx.notify();
        }
    }
    fn scope_label(&self) -> String {
        if !self.command.supports_suites() {
            "Workspace command".into()
        } else if self.selection.is_empty() {
            "No suites selected".into()
        } else if self.selection.has_scenario_selection() {
            let labels = self
                .suites
                .iter()
                .flat_map(|suite| {
                    suite
                        .scenarios
                        .iter()
                        .filter(|scenario| self.scenario_selected(&suite.name, &scenario.id))
                        .map(|scenario| format!("{} / {}", suite.name, scenario.label))
                })
                .collect::<Vec<_>>();
            format!("Scenarios: {}", labels.join(", "))
        } else {
            format!("Suites: {}", self.selected_names().join(", "))
        }
    }
    pub(super) fn approve_report(
        &mut self,
        report: &super::reports::ReportApproval,
        cx: &mut Context<Self>,
    ) {
        if self.running() || self.disabled {
            return;
        }
        let result = (|| -> Result<RunIntent> {
            let name = report
                .entry
                .suite_name()
                .ok_or_else(|| anyhow!("The report's suite is unavailable"))?;
            let intent = RunIntent {
                request: RunRequest {
                    root: self
                        .root
                        .clone()
                        .ok_or_else(|| anyhow!("Open a workspace first"))?,
                    command: RunCommand::Approve,
                    options: RunOptions {
                        regressify_dir: display_path(&report.output),
                        run_id: report.run_id.clone(),
                        scenarios: report
                            .scenario_ids
                            .iter()
                            .map(|id| crate::cli::args::ScenarioFilter {
                                suite: name.clone(),
                                scenario: id.clone(),
                            })
                            .collect(),
                        ..Default::default()
                    },
                },
                suite_names: vec![name],
                expected_output_ids: Some(vec![report.suite_id.clone()]),
                review_after_run: false,
            };
            intent.resolve(&self.suites)?;
            Ok(intent)
        })();
        match result {
            Ok(intent) => {
                let mut selection = SuiteSelection::default();
                for suite in self
                    .suites
                    .iter()
                    .filter(|suite| intent.suite_names.contains(&suite.name))
                {
                    let selected = if report.scenario_ids.is_empty() {
                        selection.toggle(suite, true)
                    } else {
                        report
                            .scenario_ids
                            .iter()
                            .try_for_each(|id| selection.toggle_scenario(suite, id, true))
                    };
                    if let Err(error) = selected {
                        self.status = format!("{error:#}");
                        cx.notify();
                        return;
                    }
                }
                self.selection = selection;
                cx.emit(RunEvent::SelectionChanged);
                self.command = RunCommand::Approve;
                self.pending_request = Some(intent);
                self.confirmation = true;
                self.tab = 0;
            }
            Err(error) => self.status = format!("{error:#}"),
        }
        cx.notify();
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
            && !self
                .errors
                .keys()
                .any(|key| RunOptions::applies(key, command))
            && self.root.is_some()
            && (command != RunCommand::Test || self.require_references().is_ok())
            && (!command.supports_suites()
                || (!self.selection.is_empty() && self.catalog_error.is_none()))
    }

    fn flush_options(&mut self, command: RunCommand, cx: &mut Context<Self>) -> Result<()> {
        let previous_output = self.output_root().ok();
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
            match binding.field.parse(&text).and_then(|value| {
                let mut candidate = options.clone();
                candidate[&binding.field.key] = value.clone();
                serde_json::from_value::<RunOptions>(candidate)?;
                Ok(value)
            }) {
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
        self.options = options;
        if self.output_root().ok() != previous_output {
            self.refresh_artifacts(cx);
        }
        anyhow::ensure!(
            !self
                .errors
                .keys()
                .any(|key| RunOptions::applies(key, command)),
            "Correct run options before continuing"
        );
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

    pub(super) fn choose(&mut self, command: RunCommand, _: &mut Window, cx: &mut Context<Self>) {
        if self.running() || self.disabled {
            return;
        }
        if let Err(error) = self.flush_options(command, cx) {
            self.status = format!("{error:#}");
            self.tab = 2;
            cx.notify();
            return;
        }
        self.command = command;
        self.tab = if command.supports_suites() { 0 } else { 2 };
        self.confirmation = false;
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
            .filter(|field| !matches!(field.key.as_str(), "suites" | "scenarios"))
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
        match field.parse(text).and_then(|value| {
            let mut candidate = self.options.clone();
            candidate[&field.key] = value.clone();
            serde_json::from_value::<RunOptions>(candidate)?;
            Ok(value)
        }) {
            Ok(value) => {
                self.options[&field.key] = value;
                self.errors.remove(&field.key);
            }
            Err(error) => {
                self.errors.insert(field.key.clone(), format!("{error:#}"));
            }
        }
        if matches!(field.key.as_str(), "siteEnv" | "regressifyDir") {
            self.refresh_artifacts(cx);
        }
        self.confirmation = false;
        cx.notify();
    }

    fn request_start(&mut self, cx: &mut Context<Self>) {
        match self.prepare_intent(cx) {
            Ok(intent) if self.command.needs_confirmation() => {
                self.pending_request = Some(intent);
                self.confirmation = true;
            }
            Ok(intent) => cx.emit(RunEvent::Requested(Box::new(intent))),
            Err(error) => {
                self.status = format!("{error:#}");
                self.tab = 2;
            }
        }
        cx.notify();
    }

    pub(super) fn prepare_intent(&mut self, cx: &mut Context<Self>) -> Result<RunIntent> {
        self.flush_options(self.command, cx)?;
        if self.command == RunCommand::Test {
            self.require_references()?;
        }
        anyhow::ensure!(
            self.can_start(),
            "Select valid suites and correct run settings before continuing"
        );
        let mut values = self.options.clone();
        let defaults = serde_json::to_value(RunOptions::default())?;
        if let Some(values) = values.as_object_mut() {
            for (key, value) in values {
                if !RunOptions::applies(key, self.command) {
                    *value = defaults[key].clone();
                }
            }
        }
        let mut options: RunOptions = serde_json::from_value(values)?;
        if self.command.supports_suites() {
            options.scenarios = self.selection.scenario_filters(&self.suites, false)?;
        }
        let suite_names = self.selected_names();
        let mut expected_output_ids = None;
        if matches!(self.command, RunCommand::Approve | RunCommand::Compare)
            && options.run_id.is_empty()
            && let Some(previous) = &self.manager.request
            && previous.command.supports_suites()
            && !previous.options.run_id.is_empty()
            && (previous.options.scenarios.is_empty()
                || (!options.scenarios.is_empty()
                    && options.scenarios.iter().all(|selected| {
                        previous.options.scenarios.iter().any(|old| {
                            old.scenario == selected.scenario
                                && if matches!(
                                    previous.command,
                                    RunCommand::Compare | RunCommand::Approve
                                ) {
                                    self.executed_suites.iter().any(|(name, id)| {
                                        name == &selected.suite && id == &old.suite
                                    })
                                } else {
                                    old.suite == selected.suite
                                }
                        })
                    })))
            && self.root.as_ref().is_some_and(|root| {
                options.output_directory(root) == previous.options.output_directory(&previous.root)
            })
            && self
                .suites
                .iter()
                .filter(|suite| suite_names.contains(&suite.name))
                .all(|suite| {
                    previous.options.suites.contains(&glob::Pattern::escape(
                        if matches!(previous.command, RunCommand::Approve | RunCommand::Compare) {
                            &suite.output_id
                        } else {
                            &suite.name
                        },
                    ))
                })
        {
            options.run_id = previous.options.run_id.clone();
            let ids = self
                .executed_suites
                .iter()
                .filter(|(name, _)| suite_names.contains(name))
                .map(|(_, id)| id.clone())
                .collect::<Vec<_>>();
            if ids.len() == suite_names.len() {
                expected_output_ids = Some(ids);
            }
        }
        let intent = RunIntent {
            request: RunRequest {
                root: self
                    .root
                    .clone()
                    .ok_or_else(|| anyhow!("Open a workspace first"))?,
                command: self.command,
                options,
            },
            suite_names,
            expected_output_ids,
            review_after_run: false,
        };
        intent.resolve(&self.suites)?;
        Ok(intent)
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
        if self.command != command
            || self
                .errors
                .keys()
                .any(|key| RunOptions::applies(key, command))
        {
            return;
        }
        self.tab = 0;
        self.confirmation = false;
        self.request_start(cx);
    }

    pub(super) fn start(
        &mut self,
        request: RunRequest,
        review_after_run: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.running() || self.disabled || self.root.as_ref() != Some(&request.root) {
            return;
        }
        self.command = request.command;
        self.executed_suites = self
            .suites
            .iter()
            .filter(|suite| {
                request.options.suites.contains(&glob::Pattern::escape(
                    if matches!(request.command, RunCommand::Compare | RunCommand::Approve) {
                        &suite.output_id
                    } else {
                        &suite.name
                    },
                ))
            })
            .map(|suite| (suite.name.clone(), suite.output_id.clone()))
            .collect();
        match self.manager.start(request) {
            Ok(()) => {
                self.confirmation = false;
                self.pending_request = None;
                self.tab = 0;
                self.started = Some(Instant::now());
                self.collapsed.clear();
                self.status = format!("Running {}", self.command.label());
                self.log_text.clear();
                self.log_scope = None;
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
                                this.refresh_log(window, cx);
                                if finished {
                                    this.status = match &this.manager.outcome {
                                        Some(RunOutcome::Completed) => "Completed".into(),
                                        Some(RunOutcome::Cancelled) => "Cancelled".into(),
                                        Some(RunOutcome::Failed(error)) => error.clone(),
                                        None => "Command ended without a result".into(),
                                    };
                                    cx.emit(RunEvent::Finished);
                                    if review_after_run && this.executed_report_available() {
                                        this.review_results(cx);
                                    }
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
}

impl RunPanel {
    fn render_selection(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.disabled || self.running();
        let all_selected = !self.selection.is_empty()
            && self
                .suites
                .iter()
                .filter(|suite| suite.error.is_none())
                .all(|suite| self.selection.all_selected(suite));
        let panel = cx.entity();
        let mut list = div().flex().flex_col().gap_2().child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(format!(
                    "{} {} in {} {} selected",
                    self.selected_scenario_count(),
                    if self.selected_scenario_count() == 1 {
                        "scenario"
                    } else {
                        "scenarios"
                    },
                    self.selection.len(),
                    if self.selection.len() == 1 {
                        "suite"
                    } else {
                        "suites"
                    }
                ))
                .child(
                    design::selection_checkbox(
                        "select-all-suites",
                        all_selected,
                        !self.selection.is_empty() && !all_selected,
                        disabled || self.suites.is_empty(),
                        cx,
                    )
                    .accessibility_label("Select all")
                    .child("Select all")
                    .on_change(move |state, _, _, cx| {
                        panel.update(cx, |panel, cx| {
                            panel.select_all(state == gpui_kit::base::CheckboxState::Checked, cx)
                        });
                    }),
                ),
        );
        for suite in &self.suites {
            let name = suite.name.clone();
            let report_name = name.clone();
            let panel = cx.entity();
            let expanded = self.expanded_selections.contains(&name);
            let expand_name = name.clone();
            let mut section = div()
                .flex()
                .flex_col()
                .gap_1()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            design::selection_checkbox(
                                SharedString::from(format!("suite-check-{name}")),
                                self.selection.all_selected(suite),
                                self.selection.contains(&name)
                                    && !self.selection.all_selected(suite),
                                disabled || suite.error.is_some(),
                                cx,
                            )
                            .accessibility_label(format!("Select suite {name}"))
                            .on_change(move |state, _, _, cx| {
                                panel.update(cx, |panel, cx| {
                                    panel.toggle_suite(
                                        &name,
                                        state == gpui_kit::base::CheckboxState::Checked,
                                        cx,
                                    )
                                });
                            }),
                        )
                        .child(
                            Button::new(SharedString::from(format!("suite-expand-{}", suite.name)))
                                .ghost()
                                .small()
                                .flex_1()
                                .min_w_0()
                                .justify_start()
                                .icon(if expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .accessibility_label(format!(
                                    "{} ({}/{} scenarios)",
                                    suite.name,
                                    self.selection.scenario_count(suite),
                                    suite.scenarios.len()
                                ))
                                .child(div().flex_1().min_w_0().truncate().text_left().child(
                                    format!(
                                        "{} ({}/{} scenarios)",
                                        suite.name,
                                        self.selection.scenario_count(suite),
                                        suite.scenarios.len()
                                    ),
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.expanded_selections.remove(&expand_name) {
                                        this.expanded_selections.insert(expand_name.clone());
                                    }
                                    cx.notify();
                                })),
                        )
                        .when(self.can_open_saved_suite(&suite.name), |row| {
                            row.child(
                                Button::new(SharedString::from(format!(
                                    "saved-report-{}",
                                    suite.name
                                )))
                                .ghost()
                                .small()
                                .icon(gpui_kit::assets::IconName::Images)
                                .label("Open Report")
                                .disabled(self.disabled)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.open_saved_reports(vec![report_name.clone()], cx)
                                    },
                                )),
                            )
                        }),
                )
                .child(
                    div()
                        .pl_6()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(self.artifact_summary(&suite.name)),
                )
                .children(suite.error.as_ref().map(|error| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error.clone())
                }));
            if expanded {
                for scenario in &suite.scenarios {
                    let name = suite.name.clone();
                    let id = scenario.id.clone();
                    let log_suite = name.clone();
                    let log_id = id.clone();
                    section = section.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .pl_6()
                            .py_1()
                            .child(
                                Checkbox::new(SharedString::from(format!(
                                    "scenario-check-{name}:{id}"
                                )))
                                .label(scenario.label.clone())
                                .flex_1()
                                .min_w_0()
                                .tooltip(format!("{}\n{id}", scenario.url))
                                .checked(self.scenario_selected(&name, &id))
                                .disabled(disabled || suite.error.is_some())
                                .on_change(cx.listener(
                                    move |this, checked, _, cx| {
                                        this.toggle_scenario(&name, &id, *checked, cx)
                                    },
                                )),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "scenario-log-{log_suite}:{log_id}"
                                )))
                                .ghost()
                                .xsmall()
                                .icon(gpui_kit::assets::IconName::Terminal)
                                .tooltip(format!("View run log: {}", scenario.label))
                                .accessibility_label(format!("View run log: {}", scenario.label))
                                .disabled(
                                    self.disabled || !self.has_scenario_log(&log_suite, &log_id),
                                )
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.show_scenario_log(&log_suite, &log_id, window, cx)
                                    },
                                )),
                            )
                            .children([RunCommand::Test, RunCommand::Reference].into_iter().map(
                                |command| {
                                    let name = suite.name.clone();
                                    let id = scenario.id.clone();
                                    Button::new(SharedString::from(format!(
                                        "scenario-{}-{name}:{id}",
                                        command.argument()
                                    )))
                                    .ghost()
                                    .xsmall()
                                    .icon(if command == RunCommand::Test {
                                        gpui_kit::assets::IconName::Play
                                    } else {
                                        gpui_kit::assets::IconName::Camera
                                    })
                                    .tooltip(format!(
                                        "{}: {}",
                                        command.action_label(),
                                        scenario.label
                                    ))
                                    .accessibility_label(format!(
                                        "{}: {}",
                                        command.action_label(),
                                        scenario.label
                                    ))
                                    .disabled(
                                        disabled
                                            || suite.error.is_some()
                                            || (command == RunCommand::Test
                                                && !self.has_reference(&name)),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        match this.set_scenario(&name, &id, cx) {
                                            Ok(()) => this.request_command(command, window, cx),
                                            Err(error) => {
                                                this.status = format!("{error:#}");
                                                cx.notify();
                                            }
                                        }
                                    }))
                                },
                            )),
                    );
                }
            }
            list = list.child(section);
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
        let scope = self.scope_label();
        let content = match self.tab {
            0 => self.render_activity(cx),
            1 => div()
                .id("run-suite-list")
                .size_full()
                .overflow_y_scrollbar()
                .p_5()
                .child(self.render_selection(cx))
                .into_any_element(),
            2 => self.render_options(cx),
            _ => self.render_log(cx),
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
                                .when(command == RunCommand::Test, |button| {
                                    button.tooltip(
                                        "Requires reference images for every selected suite",
                                    )
                                })
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
                    .when(self.can_open_report(), |bar| {
                        bar.child(
                            Button::new("activity-view-results")
                                .primary()
                                .small()
                                .icon(gpui_kit::assets::IconName::Images)
                                .label("Open Report")
                                .tooltip("Open this run's test and reference comparison")
                                .disabled(self.disabled)
                                .on_click(cx.listener(|this, _, _, cx| this.open_report(cx))),
                        )
                    })
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
            .child(self.render_artifact_status(cx))
            .when_some(
                self.pending_request.as_ref().filter(|_| self.confirmation),
                |panel, intent| {
                    panel.child(
                        div()
                            .id("run-confirmation")
                            .role(Role::Group)
                            .aria_label("Run confirmation")
                            .test_support()
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
                                            .child(intent.request.command.confirmation_message()),
                                    )
                                    .child(div().text_sm().child(
                                        if intent.request.command.supports_suites() {
                                            format!("Suites: {}", intent.suite_names.join(", "))
                                        } else {
                                            "Workspace images".into()
                                        },
                                    ))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .truncate()
                                            .child(display_path(
                                                &intent
                                                    .request
                                                    .options
                                                    .output_directory(&intent.request.root),
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .id("run-confirm-target")
                                    .role(Role::Status)
                                    .aria_label(format!(
                                        "Run: {}",
                                        if intent.request.options.run_id.is_empty() {
                                            "Latest results"
                                        } else {
                                            &intent.request.options.run_id
                                        }
                                    ))
                                    .test_support()
                                    .text_xs()
                                    .child(if intent.request.options.run_id.is_empty() {
                                        if intent.request.command.captures() {
                                            "New capture".to_string()
                                        } else {
                                            "Latest results".to_string()
                                        }
                                    } else {
                                        format!("Run: {}", intent.request.options.run_id)
                                    }),
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
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.confirmation
                                            && !this.disabled
                                            && !this.running()
                                            && let Some(intent) = this.pending_request.take()
                                        {
                                            this.confirmation = false;
                                            cx.emit(RunEvent::Requested(Box::new(intent)));
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                },
            )
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
                    .child(Tab::new().label("Run settings"))
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

#[cfg(all(test, feature = "gui-tests"))]
mod tests {
    use super::{RunCommand, RunEvent, RunIntent, RunOptions, RunOutcome, RunRequest};
    use gpui_kit::test::{TestAppContextExt, TestWindowExt};
    use gpui_kit::{AppContext, TestAppContext};
    use serde_json::json;
    use std::{cell::RefCell, rc::Rc};

    #[gpui_kit::test]
    fn different_suite_selection_does_not_offer_an_unrelated_previous_report(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let (_, view) = super::super::tests::editor(cx, directory.path());
        cx.update(|cx| {
            view.update(cx, |this, cx| {
                this.runs.update(cx, |panel, cx| {
                    panel.manager.request = Some(RunRequest {
                        root: directory.path().canonicalize().unwrap(),
                        command: RunCommand::Test,
                        options: RunOptions {
                            suites: vec!["previous-suite".into()],
                            run_id: "old-run".into(),
                            ..Default::default()
                        },
                    });
                    panel.executed_suites =
                        vec![("previous-suite".into(), "previous-output".into())];
                    panel.manager.outcome = Some(RunOutcome::Completed);
                    panel.select_all(true, cx);
                    assert!(!panel.can_open_report());
                    assert!(panel.executed_report_available());
                    panel.select_all(false, cx);
                    assert!(panel.can_open_report());
                })
            })
        });
    }

    #[gpui_kit::test]
    async fn saved_capture_report_request_compares_without_recapturing(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join(".regressify/ui-test");
        for path in [
            output.join("bitmaps_reference/home.png"),
            output.join("bitmaps_test/stored-capture/home.png"),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            lodepng::encode32_file(path, &[255_u8; 4], 1, 1).unwrap();
        }
        let (handle, view) = super::super::tests::editor(cx, directory.path());
        let requests = Rc::new(RefCell::new(Vec::<RunIntent>::new()));
        let received = requests.clone();
        let _subscription = cx.update(|cx| {
            view.update(cx, |this, cx| {
                cx.subscribe(&this.runs, move |_, _, event, _| {
                    if let RunEvent::Requested(intent) = event {
                        received.borrow_mut().push((**intent).clone());
                    }
                })
            })
        });
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            view.read(cx).runs.read(cx).can_open_saved_suite("ui-test")
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("duplicate-item", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            view.read(cx).runs.read(cx).can_open_saved_suite("ui-test")
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("suite-open-report", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let requests = requests.borrow();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].request.command, RunCommand::Compare);
        assert_eq!(requests[0].request.options.run_id, "stored-capture");
        assert_eq!(requests[0].suite_names, vec!["ui-test"]);
        assert!(requests[0].review_after_run);
        cx.update(|cx| assert!(!view.read(cx).runs.read(cx).running()));
    }

    #[gpui_kit::test]
    async fn artifact_detection_tracks_output_changes_and_multiple_suite_baselines(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let reference = directory
            .path()
            .join(".regressify/ui-test/bitmaps_reference/home.png");
        std::fs::create_dir_all(reference.parent().unwrap()).unwrap();
        lodepng::encode32_file(reference, &[255_u8; 4], 1, 1).unwrap();
        let (handle, view) = super::super::tests::editor(cx, directory.path());
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            view.read(cx).runs.read(cx).has_reference("ui-test")
        })
        .await;
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.runs.update(cx, |panel, cx| {
                    panel.select_all(true, cx);
                    let mut suites = panel.suites.clone();
                    let mut second = suites[0].clone();
                    second.name = "another-suite".into();
                    second.output_id = "another-output".into();
                    suites.push(second);
                    panel.update_catalog(suites, None, cx);
                    panel.select_all(true, cx);
                    panel.choose(RunCommand::Reference, window, cx);
                });
            })
        })
        .unwrap();
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            !view.read(cx).runs.read(cx).artifacts.loading
        })
        .await;
        cx.update_window(handle.into(), |_, _, cx| {
            view.update(cx, |this, cx| {
                this.runs.update(cx, |panel, cx| {
                    assert!(!panel.can_test_selection());
                    panel.toggle_suite("another-suite", false, cx);
                    assert!(panel.can_test_selection());
                    let field = panel
                        .bindings
                        .iter()
                        .find(|binding| binding.field.key == "siteEnv")
                        .unwrap()
                        .field
                        .clone();
                    panel.change(&field, "staging", cx);
                    assert!(!panel.has_reference("ui-test"));
                    panel.change(&field, "", cx);
                    panel.change(&field, "staging", cx);
                })
            })
        })
        .unwrap();
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            !view.read(cx).runs.read(cx).artifacts.loading
        })
        .await;
        cx.update(|cx| {
            let panel = view.read(cx).runs.read(cx);
            assert_eq!(
                panel.output_root().unwrap(),
                directory
                    .path()
                    .canonicalize()
                    .unwrap()
                    .join(".regressify__staging")
            );
            assert!(!panel.has_reference("ui-test"));
        });
    }

    #[gpui_kit::test]
    async fn baseline_detection_controls_test_and_revalidates_before_execution(
        cx: &mut TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, directory.path());
        cx.update_window(handle.into(), |_, window, cx| {
            view.update(cx, |this, cx| {
                this.main_tab = 1;
                this.runs.update(cx, |panel, cx| panel.select_all(true, cx));
            });
            window.render_frame(cx);
            window.click("run-test", cx);
            assert!(!view.read(cx).runs.read(cx).running());
        })
        .unwrap();
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            !view.read(cx).runs.read(cx).artifacts.loading
        })
        .await;
        cx.update(|cx| {
            let panel = view.read(cx).runs.read(cx);
            assert!(!panel.can_test_selection());
            assert!(panel.can_request(RunCommand::Reference));
            assert!(!view.read(cx).can_test_open_suite(cx));
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.within("workbench-tabs").click(0_usize, cx);
            window.click("suite-run-test", cx);
            window.click("scenario-run-test", cx);
            window.click(("sidebar-scenario-test", 0_usize), cx);
            assert_eq!(view.read(cx).main_tab, 0);
            assert!(!view.read(cx).runs.read(cx).running());
            window.within("workbench-tabs").click(1_usize, cx);
        })
        .unwrap();
        let reference = directory
            .path()
            .join(".regressify/ui-test/bitmaps_reference/home.png");
        std::fs::create_dir_all(reference.parent().unwrap()).unwrap();
        lodepng::encode32_file(&reference, &[255_u8; 4], 1, 1).unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("refresh-run-artifacts", cx);
        })
        .unwrap();
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            view.read(cx).runs.read(cx).can_test_selection()
        })
        .await;
        cx.update(|cx| assert!(view.read(cx).can_test_open_suite(cx)));
        cx.update(|cx| {
            view.update(cx, |this, cx| {
                let suites = this.session.suites.clone();
                this.runs.update(cx, |panel, cx| {
                    let intent = panel.prepare_intent(cx).unwrap();
                    assert!(intent.resolve_for_execution(&suites).is_ok());
                    std::fs::remove_file(&reference).unwrap();
                    assert!(
                        intent
                            .resolve_for_execution(&suites)
                            .unwrap_err()
                            .to_string()
                            .contains("Run Reference")
                    );
                    panel.refresh_artifacts(cx);
                });
            })
        });
        cx.wait_for(handle.into(), std::time::Duration::from_secs(3), |_, cx| {
            !view.read(cx).runs.read(cx).artifacts.loading
        })
        .await;
        cx.update(|cx| assert!(!view.read(cx).runs.read(cx).can_test_selection()));
    }

    #[gpui_kit::test]
    fn open_report_button_uses_recorded_capture_scope(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let (handle, view) = super::super::tests::editor(cx, directory.path());
        let requests = Rc::new(RefCell::new(Vec::<RunIntent>::new()));
        let received = requests.clone();
        let _subscription = cx.update(|cx| {
            view.update(cx, |this, cx| {
                cx.subscribe(&this.runs, move |_, _, event, _| {
                    if let RunEvent::Requested(intent) = event {
                        received.borrow_mut().push((**intent).clone());
                    }
                })
            })
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("duplicate-item", cx);
            view.update(cx, |this, cx| {
                let suite = this.session.suites[0].clone();
                this.main_tab = 1;
                this.runs.update(cx, |panel, cx| {
                    panel.manager.request = Some(RunRequest {
                        root: directory.path().canonicalize().unwrap(),
                        command: RunCommand::Test,
                        options: RunOptions {
                            regressify_dir: ".captured-output".into(),
                            run_id: "captured-run".into(),
                            suites: vec![suite.name.clone()],
                            scenarios: vec![crate::cli::args::ScenarioFilter {
                                suite: suite.name.clone(),
                                scenario: suite.scenarios[0].id.clone(),
                            }],
                            skip_compare: true,
                            ..Default::default()
                        },
                    });
                    panel.executed_suites = vec![(suite.name, suite.output_id)];
                    panel.manager.outcome = Some(RunOutcome::Completed);
                    panel.options["regressifyDir"] = json!(".changed-output");
                    panel.options["runId"] = json!("changed-run");
                    panel.selection.clear();
                    cx.notify();
                });
            });
            window.render_frame(cx);
            window.within("run-tabs").click(3_usize, cx);
            window.click("activity-view-results", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let requests = requests.borrow();
        assert_eq!(requests.len(), 1);
        let intent = &requests[0];
        assert!(intent.review_after_run);
        assert_eq!(intent.request.command, RunCommand::Compare);
        assert_eq!(intent.request.options.regressify_dir, ".captured-output");
        assert_eq!(intent.request.options.run_id, "captured-run");
        assert_eq!(intent.suite_names, vec!["ui-test"]);
        assert_eq!(intent.request.options.scenarios.len(), 1);
        cx.update(|cx| {
            assert!(view.read(cx).session.dirty());
            assert!(!view.read(cx).runs.read(cx).running());
        });
    }
}
