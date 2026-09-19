use anyhow::Context as _;
use std::time::{Duration, Instant};

use super::*;
use crate::desktop::git::{self as model, Cancellation, Snapshot};

mod view;

#[cfg(all(test, feature = "gui-tests"))]
mod tests;

const LOCAL_INTERVAL: Duration = Duration::from_secs(5);
const REMOTE_INTERVAL: Duration = Duration::from_secs(60);

pub(super) enum GitEvent {
    Changed,
    Requested(Box<model::Request>),
}

pub(super) struct GitPanel {
    root: Option<PathBuf>,
    snapshot: Option<Snapshot>,
    error: Option<String>,
    remote_error: Option<String>,
    last_fetch: Option<Instant>,
    last_fetch_attempt: Option<Instant>,
    generation: u64,
    remote_generation: u64,
    scan: Option<Cancellation>,
    fetch: Option<Cancellation>,
    action: Option<Cancellation>,
    diff_cancel: Option<Cancellation>,
    poller: Option<Task<()>>,
    disabled: bool,
    selection: BTreeSet<String>,
    collapsed: BTreeSet<String>,
    active_file: Option<String>,
    diff_generation: u64,
    diff: Entity<EditorState>,
    diff_text: String,
    message: Entity<TextareaState>,
    remote_url: Entity<InputState>,
    status: String,
    rows: Vec<view::TreeRow>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<GitEvent> for GitPanel {}

impl Drop for GitPanel {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

impl GitPanel {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder("Commit message")
        });
        let remote_url = cx.new(|cx| InputState::new(window, cx).placeholder("Remote URL"));
        let message_subscription = cx.subscribe(&message, |_, _, _: &InputEvent, cx| cx.notify());
        let remote_subscription = cx.subscribe(&remote_url, |_, _, _: &InputEvent, cx| cx.notify());
        Self {
            root: None,
            snapshot: None,
            error: None,
            remote_error: None,
            last_fetch: None,
            last_fetch_attempt: None,
            generation: 0,
            remote_generation: 0,
            scan: None,
            fetch: None,
            action: None,
            diff_cancel: None,
            poller: None,
            disabled: false,
            selection: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            active_file: None,
            diff_generation: 0,
            diff: cx.new(|cx| {
                EditorState::new(window, cx)
                    .language("diff")
                    .line_number(false)
            }),
            diff_text: String::new(),
            message,
            remote_url,
            status: String::new(),
            rows: Vec::new(),
            _subscriptions: vec![message_subscription, remote_subscription],
        }
    }

    pub(super) fn cancel_all(&mut self) {
        for operation in [
            &mut self.scan,
            &mut self.fetch,
            &mut self.action,
            &mut self.diff_cancel,
        ] {
            if let Some(cancellation) = operation.take() {
                cancellation.cancel();
            }
        }
        self.poller = None;
    }

    pub(super) fn set_workspace(
        &mut self,
        root: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_all();
        self.generation += 1;
        self.remote_generation += 1;
        self.diff_generation += 1;
        self.root = root;
        self.snapshot = None;
        self.error = None;
        self.remote_error = None;
        self.last_fetch = None;
        self.last_fetch_attempt = None;
        self.selection.clear();
        self.collapsed.clear();
        self.active_file = None;
        self.rows.clear();
        self.diff_text.clear();
        self.status.clear();
        self.diff
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.message
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.remote_url
            .update(cx, |state, cx| state.set_value("", window, cx));
        if self.root.is_some() {
            self.refresh(window, cx);
            self.start_polling(window, cx);
        }
        cx.emit(GitEvent::Changed);
        cx.notify();
    }

    fn start_polling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation = self.generation;
        self.poller = Some(cx.spawn_in(window, async move |panel, cx| {
            loop {
                cx.background_executor().timer(LOCAL_INTERVAL).await;
                let keep_running = panel
                    .update_in(cx, |this, window, cx| {
                        if this.generation != generation {
                            return false;
                        }
                        this.refresh(window, cx);
                        true
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        }));
    }

    pub(super) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            cx.notify();
        }
    }

    pub(super) fn active(&self) -> bool {
        self.action.is_some()
    }

    pub(super) fn cancel_operation(&mut self, cx: &mut Context<Self>) {
        if let Some(operation) = &self.action {
            operation.cancel();
            self.status = "Cancelling Git operation...".into();
            cx.notify();
        }
    }

    fn install_snapshot(
        &mut self,
        snapshot: Snapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .snapshot
            .as_ref()
            .is_some_and(|previous| !previous.same_tracking_as(&snapshot))
        {
            self.remote_generation += 1;
            if let Some(operation) = self.fetch.take() {
                operation.cancel();
            }
            self.last_fetch = None;
            self.last_fetch_attempt = None;
            self.remote_error = None;
        }
        self.selection.retain(|path| {
            snapshot
                .changes
                .iter()
                .any(|change| &change.path == path && !change.conflict && !change.submodule)
        });
        if self
            .active_file
            .as_ref()
            .is_some_and(|path| !snapshot.changes.iter().any(|change| &change.path == path))
        {
            self.active_file = None;
            self.diff_generation += 1;
            if let Some(cancellation) = self.diff_cancel.take() {
                cancellation.cancel();
            }
            self.set_diff(String::new(), window, cx);
        }
        self.snapshot = Some(snapshot);
        self.error = None;
        self.rebuild_tree();
    }

    pub(super) fn badge(&self) -> String {
        if self.action.is_some() {
            return "Git working".into();
        }
        if self.error.is_some() {
            return "Git unavailable".into();
        }
        let Some(snapshot) = &self.snapshot else {
            return "Git checking".into();
        };
        if snapshot.repository.is_none() {
            return "Not tracked".into();
        }
        if snapshot.conflicts() > 0 {
            return format!(
                "Git {} {}",
                snapshot.conflicts(),
                if snapshot.conflicts() == 1 {
                    "conflict"
                } else {
                    "conflicts"
                }
            );
        }
        if snapshot.behind > 0 {
            return format!(
                "{} files | {} incoming",
                snapshot.changes.len(),
                snapshot.behind
            );
        }
        if self.remote_error.is_some() {
            return format!("Git {} | offline", snapshot.changes.len());
        }
        if snapshot.ahead > 0 {
            return format!(
                "{} files | {} outgoing",
                snapshot.changes.len(),
                snapshot.ahead
            );
        }
        format!("Git {} files", snapshot.changes.len())
    }

    pub(super) fn attention(&self) -> bool {
        self.remote_error.is_some()
            || self.error.is_some()
            || self.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot.behind > 0 || snapshot.conflicts() > 0 || snapshot.operation.is_some()
            })
    }

    pub(super) fn summary(&self) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        let Some(snapshot) = &self.snapshot else {
            return "Checking Git repository".into();
        };
        if snapshot.repository.is_none() {
            return "Workspace is not tracked by Git".into();
        }
        format!(
            "{} | {} changed files | {} incoming | {} outgoing\n{}",
            snapshot.branch,
            snapshot.changes.len(),
            snapshot.behind,
            snapshot.ahead,
            self.remote_status()
        )
    }

    fn remote_status(&self) -> String {
        if self.fetch.is_some() {
            return "Checking remote...".into();
        }
        if let Some(error) = &self.remote_error {
            return format!("Remote check failed: {error}");
        }
        let Some(snapshot) = &self.snapshot else {
            return String::new();
        };
        if snapshot.remote.as_deref() == Some(".") {
            return "Local upstream branch; no remote fetch required".into();
        }
        if snapshot.remotes.is_empty() {
            return "No remote configured".into();
        }
        let checked = self
            .last_fetch
            .map(|time| format!("Remote checked {}s ago", time.elapsed().as_secs()))
            .unwrap_or_else(|| "Remote not checked yet".into());
        if snapshot.upstream.is_none() {
            format!("No upstream branch | {checked}")
        } else {
            checked
        }
    }

    pub(super) fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scan.is_some() || self.action.is_some() || self.disabled {
            return;
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        let cancellation = Cancellation::default();
        self.scan = Some(cancellation.clone());
        let generation = self.generation;
        let task = cx
            .background_executor()
            .spawn(async move { model::status(&root, &cancellation) });
        cx.spawn_in(window, async move |panel, cx| {
            let result = task.await;
            let _ = panel.update_in(cx, |this, window, cx| {
                if this.generation != generation {
                    return;
                }
                this.scan = None;
                match result {
                    Ok(snapshot) => {
                        this.install_snapshot(snapshot, window, cx);
                        if this.diff_cancel.is_none()
                            && let Some(path) = this.active_file.clone()
                        {
                            this.review(path, window, cx);
                        }
                        if this
                            .last_fetch_attempt
                            .is_none_or(|time| time.elapsed() >= REMOTE_INTERVAL)
                        {
                            this.check_remote(window, cx);
                        }
                    }
                    Err(error) => this.error = Some(format!("{error:#}")),
                }
                cx.emit(GitEvent::Changed);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn check_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fetch.is_some() || self.action.is_some() || self.disabled {
            return;
        }
        let Some(snapshot) = self
            .snapshot
            .clone()
            .filter(|snapshot| snapshot.default_remote().is_some())
        else {
            return;
        };
        let cancellation = Cancellation::default();
        self.fetch = Some(cancellation.clone());
        self.last_fetch_attempt = Some(Instant::now());
        let generation = self.generation;
        let remote_generation = self.remote_generation;
        let task = cx
            .background_executor()
            .spawn(async move { model::fetch(&snapshot, &cancellation) });
        cx.spawn_in(window, async move |panel, cx| {
            let result = task.await;
            let _ = panel.update_in(cx, |this, window, cx| {
                if this.generation != generation || this.remote_generation != remote_generation {
                    return;
                }
                this.fetch = None;
                match result {
                    Ok(()) => {
                        this.last_fetch = Some(Instant::now());
                        this.remote_error = None;
                    }
                    Err(error) => this.remote_error = Some(format!("{error:#}")),
                }
                this.refresh(window, cx);
                cx.emit(GitEvent::Changed);
                cx.notify();
            });
        })
        .detach();
        cx.emit(GitEvent::Changed);
        cx.notify();
    }

    fn review(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        if let Some(cancellation) = self.diff_cancel.take() {
            cancellation.cancel();
        }
        if self.active_file.as_ref() != Some(&path) {
            self.set_diff("Loading diff...".into(), window, cx);
        }
        self.active_file = Some(path.clone());
        self.diff_generation += 1;
        let generation = self.diff_generation;
        let cancellation = Cancellation::default();
        self.diff_cancel = Some(cancellation.clone());
        let task = cx
            .background_executor()
            .spawn(async move { model::diff(&snapshot, &path, &cancellation) });
        cx.spawn_in(window, async move |panel, cx| {
            let result = task.await;
            let _ = panel.update_in(cx, |this, window, cx| {
                if this.diff_generation != generation {
                    return;
                }
                this.diff_cancel = None;
                this.set_diff(
                    result.unwrap_or_else(|error| format!("{error:#}")),
                    window,
                    cx,
                );
            });
        })
        .detach();
        cx.notify();
    }

    fn set_diff(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.diff_text != text {
            self.diff
                .update(cx, |editor, cx| editor.set_value(text.clone(), window, cx));
            self.diff_text = text;
        }
        cx.notify();
    }

    fn request(&mut self, action: model::Action, cx: &mut Context<Self>) {
        if self.disabled || self.active() || self.error.is_some() {
            return;
        }
        if let Some((workspace, expected)) = self.root.clone().zip(self.snapshot.clone()) {
            cx.emit(GitEvent::Requested(Box::new(model::Request {
                workspace,
                expected,
                action,
            })));
        }
    }

    fn open_vscode(&mut self, file: bool, cx: &mut Context<Self>) {
        let root = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.repository.as_ref())
            .map(|repository| &repository.root)
            .or(self.root.as_ref());
        let result = (|| -> Result<()> {
            let root = root.context("Open a workspace first")?;
            let path = if file {
                self.active_file
                    .as_ref()
                    .map(|file| root.join(file))
                    .unwrap_or_else(|| root.clone())
            } else {
                root.clone()
            };
            let mut url = reqwest::Url::parse("vscode://file/")?;
            url.set_path(&display_path(&path));
            open::that_detached(url.as_str())
                .context("Cannot open VS Code; install it or open the repository manually")?;
            Ok(())
        })();
        if let Err(error) = result {
            self.status = format!("{error:#}");
            cx.notify();
        }
    }
}

impl Workbench {
    pub(super) fn guard_git(
        &mut self,
        request: Box<model::Request>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.runs.read(cx).running() || self.git.read(cx).active() {
            return;
        }
        if self.session.open.is_some()
            && let Err(error) = self.flush_drafts(window, cx)
        {
            self.status = format!("{error:#}");
            cx.notify();
            return;
        }
        self.main_tab = 3;
        self.guard(files::Pending::Git(request), window, cx);
    }

    pub(super) fn confirm_git(
        &mut self,
        request: model::Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.runs.read(cx).running() {
            return;
        }
        let detail = match &request.action {
            model::Action::Initialize => format!(
                "Initialize Git in {}? No files will be staged automatically.",
                display_path(&request.workspace)
            ),
            model::Action::Commit { paths, .. } => format!(
                "Commit the current contents of {} selected file(s) on '{}'? Other staged files will not be committed.",
                paths.len(),
                request.expected.branch
            ),
            model::Action::Pull => format!(
                "Pull '{}' from {}? Only a fast-forward is allowed; local files must be clean.",
                request.expected.branch,
                request
                    .expected
                    .upstream
                    .as_deref()
                    .unwrap_or("its upstream")
            ),
            model::Action::Push => format!(
                "Push local commits on '{}' to '{}'? Remote history will not be overwritten.",
                request.expected.branch,
                request.expected.default_remote().unwrap_or("origin")
            ),
            model::Action::AddOrigin { .. } => {
                "Add this URL as the repository's origin remote?".into()
            }
        };
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, window, _cx| {
            let run = request.clone();
            dialog
                .title(request.action.label())
                .child(detail.clone())
                .when(
                    matches!(request.action, model::Action::Commit { .. }),
                    |dialog| {
                        let paths = match &request.action {
                            model::Action::Commit { paths, .. } => paths.clone(),
                            _ => Vec::new(),
                        };
                        dialog.child(
                            div()
                                .id("git-confirm-files")
                                .max_h(px(140.))
                                .overflow_y_scrollbar()
                                .children(
                                    paths.into_iter().map(|path| div().text_sm().child(path)),
                                ),
                        )
                    },
                )
                .footer(
                    div()
                        .id("git-confirmation")
                        .role(Role::Group)
                        .aria_label("Git confirmation")
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("git-operation-cancel")
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("git-operation-confirm")
                                .primary()
                                .label(request.action.label())
                                .on_click(window.listener_for(
                                    &view,
                                    move |this, _, window, cx| {
                                        window.close_dialog(cx);
                                        this.start_git(run.clone(), window, cx);
                                    },
                                )),
                        ),
                )
        });
    }

    fn start_git(&mut self, request: model::Request, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy
            || self.runs.read(cx).running()
            || self
                .session
                .workspace
                .as_ref()
                .map(|workspace| &workspace.root)
                != Some(&request.workspace)
        {
            return;
        }
        let cancellation = Cancellation::default();
        self.busy = true;
        self.status = format!("{}...", request.action.label());
        self.sync_modal(cx);
        self.git.update(cx, |panel, cx| {
            panel.cancel_all();
            panel.generation += 1;
            panel.diff_generation += 1;
            panel.action = Some(cancellation.clone());
            panel.status = self.status.clone();
            panel.start_polling(window, cx);
            cx.emit(GitEvent::Changed);
            cx.notify();
        });
        let action = request.action.clone();
        let task = cx.background_executor().spawn(async move {
            let result = request.execute(&cancellation);
            let snapshot = model::status(&request.workspace, &Cancellation::default());
            (result, snapshot)
        });
        cx.spawn_in(window, async move |view, cx| {
            let (result, snapshot) = task.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.busy = false;
                let success = result.is_ok();
                this.status = result.unwrap_or_else(|error| format!("{error:#}"));
                this.git.update(cx, |panel, cx| {
                    panel.action = None;
                    panel.status = this.status.clone();
                    panel.disabled = false;
                    match snapshot {
                        Ok(snapshot) => panel.install_snapshot(snapshot, window, cx),
                        Err(error) => panel.error = Some(format!("{error:#}")),
                    }
                    if success && matches!(action, model::Action::Commit { .. }) {
                        panel.selection.clear();
                        panel
                            .message
                            .update(cx, |state, cx| state.set_value("", window, cx));
                    }
                    if matches!(action, model::Action::Pull | model::Action::Push) && success {
                        panel.remote_error = None;
                        panel.last_fetch = Some(Instant::now());
                    }
                    panel.refresh(window, cx);
                    cx.emit(GitEvent::Changed);
                    cx.notify();
                });
                this.sync_modal(cx);
                if matches!(action, model::Action::Pull) {
                    this.reload_after_git_pull(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn reload_after_git_pull(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.session.workspace.clone() else {
            return;
        };
        let entry = self.session.open.as_ref().map(|open| open.entry.clone());
        let selection = self.session.selection;
        let status = self.status.clone();
        self.task(
            "Refreshing workspace after Git...",
            move || {
                let mut session = Session::default();
                session.connect(&workspace.root)?;
                if let Some(entry) = entry
                    && workspace.root.join(&entry.path).try_exists()?
                {
                    session.install_document(workspace.open_document(entry)?)?;
                    session.selection = selection;
                    session.reconcile_selection();
                }
                Ok(session)
            },
            move |this, session, window, cx| {
                this.session = session;
                this.sync_suites(cx);
                this.sync_editor(window, cx);
                this.main_tab = 3;
                this.status = status;
                this.refresh_results(window, cx);
            },
            window,
            cx,
        );
    }
}
