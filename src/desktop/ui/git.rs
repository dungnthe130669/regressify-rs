use anyhow::Context as _;
use std::time::{Duration, Instant};

use super::*;
use crate::desktop::git::{self as model, Cancellation, Snapshot};

mod view;

#[cfg(all(test, feature = "gui-tests"))]
mod tests;

const LOCAL_INTERVAL: Duration = Duration::from_secs(5);
const REMOTE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, gpui_kit::Action)]
struct ChooseGitRemote {
    name: String,
}

#[derive(Clone)]
enum RemoteEdit {
    Add,
    Rename(String),
    Url { name: String, push: bool },
}

struct GitActivity {
    label: String,
    started: Instant,
    finished: Option<Duration>,
    success: Option<bool>,
    reporter: Cancellation,
}

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
    text_review: Option<std::result::Result<model::review::TextReview, String>>,
    diff_scroll: UniformListScrollHandle,
    diff_change: Option<usize>,
    message: Entity<TextareaState>,
    remote_name: Entity<InputState>,
    remote_url: Entity<InputState>,
    remote_edit: Option<(RemoteEdit, Snapshot)>,
    focus: FocusHandle,
    activity: Option<GitActivity>,
    activity_history: std::collections::VecDeque<String>,
    activity_open: bool,
    activity_follow: bool,
    activity_task: Option<Task<()>>,
    activity_log: Entity<EditorState>,
    activity_text: String,
    status: String,
    rows: Vec<view::TreeRow>,
    comparison_rows: Vec<view::TreeRow>,
    tree_focus: [FocusHandle; 2],
    tree_current: [Option<String>; 2],
    tree_scroll: [UniformListScrollHandle; 2],
    comparison_open: bool,
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
        let remote_name = cx.new(|cx| InputState::new(window, cx).placeholder("Remote name"));
        let remote_url = cx.new(|cx| {
            InputState::new(window, cx).placeholder("HTTPS, SSH URL or local repository path")
        });
        let message_subscription = cx.subscribe(&message, |_, _, _: &InputEvent, cx| cx.notify());
        let remote_subscription = cx.subscribe(&remote_url, |_, _, _: &InputEvent, cx| cx.notify());
        let name_subscription = cx.subscribe(&remote_name, |_, _, _: &InputEvent, cx| cx.notify());
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
            text_review: None,
            diff_scroll: Default::default(),
            diff_change: None,
            message,
            remote_name,
            remote_url,
            remote_edit: None,
            focus: cx.focus_handle(),
            activity: None,
            activity_history: Default::default(),
            activity_open: false,
            activity_follow: true,
            activity_task: None,
            activity_log: cx.new(|cx| EditorState::new(window, cx).line_number(false)),
            activity_text: String::new(),
            status: String::new(),
            rows: Vec::new(),
            comparison_rows: Vec::new(),
            tree_focus: std::array::from_fn(|_| cx.focus_handle().tab_index(0)),
            tree_current: Default::default(),
            tree_scroll: Default::default(),
            comparison_open: false,
            _subscriptions: vec![message_subscription, remote_subscription, name_subscription],
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
        self.activity_task = None;
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
        self.comparison_rows.clear();
        self.tree_current = Default::default();
        self.tree_scroll = Default::default();
        self.comparison_open = false;
        self.diff_text.clear();
        self.text_review = None;
        self.diff_scroll = Default::default();
        self.diff_change = None;
        self.status.clear();
        self.remote_edit = None;
        self.activity = None;
        self.activity_history.clear();
        self.activity_open = false;
        self.activity_follow = true;
        self.activity_text.clear();
        self.activity_log
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.remote_name
            .update(cx, |state, cx| state.set_value("", window, cx));
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
        if let Some(operation) = self.action.as_ref().or(self.fetch.as_ref()) {
            operation.cancel();
            operation.stage("Cancellation requested; waiting for Git processes to exit");
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
                self.finish_activity(
                    false,
                    "Remote configuration changed; fetch cancelled",
                    window,
                    cx,
                );
            }
            self.last_fetch = None;
            self.last_fetch_attempt = None;
            self.remote_error = None;
        }
        self.selection.retain(|path| {
            snapshot.changes.iter().any(|change| {
                &change.path == path
                    && !change.conflict
                    && !change.submodule
                    && (!change.is_comparison()
                        || self.snapshot.as_ref().is_some_and(|previous| {
                            previous
                                .changes
                                .iter()
                                .any(|previous| &previous.path == path && previous.is_comparison())
                        }))
            })
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
            self.text_review = None;
            self.diff_scroll = Default::default();
            self.diff_change = None;
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
        if snapshot.selected_remote.is_none() && snapshot.remote.as_deref() == Some(".") {
            return "Local upstream branch; no remote fetch required".into();
        }
        if snapshot.remotes.is_empty() {
            return "No remote configured".into();
        }
        if snapshot.default_remote().is_none() {
            return "Select a remote to continue".into();
        }
        let checked = self
            .last_fetch
            .map(|time| format!("Remote checked {}s ago", time.elapsed().as_secs()))
            .unwrap_or_else(|| "Remote not checked yet".into());
        if snapshot.comparison_upstream.is_none() {
            format!("Remote branch not fetched or published | {checked}")
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
                            this.check_remote(false, window, cx);
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

    fn check_remote(&mut self, reveal: bool, window: &mut Window, cx: &mut Context<Self>) {
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
        let cancellation = Cancellation::reporting();
        self.begin_activity(
            format!(
                "Fetch from {}",
                snapshot.default_remote().unwrap_or_default()
            ),
            cancellation.clone(),
            reveal,
            window,
            cx,
        );
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
                        this.finish_activity(true, "Fetch completed", window, cx);
                    }
                    Err(error) => {
                        let error = format!("{error:#}");
                        this.finish_activity(false, &error, window, cx);
                        this.remote_error = Some(error);
                    }
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

    fn begin_activity(
        &mut self,
        label: String,
        reporter: Cancellation,
        reveal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(previous) = self.activity.take() {
            self.activity_history.push_back(format!(
                "{} | {}",
                previous.label,
                if previous.success == Some(true) {
                    "Completed"
                } else if previous.finished.is_some() {
                    "Stopped or failed"
                } else {
                    "Interrupted"
                }
            ));
            let progress = previous.reporter.progress();
            self.activity_history.extend(progress.lines);
            while self.activity_history.len() > 400 {
                self.activity_history.pop_front();
            }
        }
        self.activity = Some(GitActivity {
            label,
            reporter,
            started: Instant::now(),
            finished: None,
            success: None,
        });
        self.activity_open |= reveal;
        self.sync_activity_log(window, cx);
        let generation = self.generation;
        self.activity_task = Some(cx.spawn_in(window, async move |panel, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if !panel
                    .update_in(cx, |this, window, cx| {
                        if this.generation != generation
                            || this
                                .activity
                                .as_ref()
                                .is_none_or(|activity| activity.finished.is_some())
                        {
                            return false;
                        }
                        this.sync_activity_log(window, cx);
                        cx.notify();
                        true
                    })
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }));
    }

    fn finish_activity(
        &mut self,
        success: bool,
        message: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(activity) = &mut self.activity {
            activity.finished = Some(activity.started.elapsed());
            activity.success = Some(success);
            activity.reporter.stage(message);
        }
        self.activity_task = None;
        self.sync_activity_log(window, cx);
    }

    fn sync_activity_log(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut lines = self.activity_history.iter().cloned().collect::<Vec<_>>();
        if let Some(activity) = &self.activity {
            lines.push(format!(
                "{} | {}",
                activity.label,
                if activity.finished.is_none() {
                    "Running"
                } else if activity.success == Some(true) {
                    "Completed"
                } else if activity.reporter.is_cancelled() {
                    "Cancelled"
                } else {
                    "Failed"
                }
            ));
            let progress = activity.reporter.progress();
            if progress.omitted > 0 {
                lines.push(format!("[{} earlier log lines omitted]", progress.omitted));
            }
            lines.extend(progress.lines);
        }
        let text = lines.join("\n");
        if self.activity_text != text {
            self.activity_log.update(cx, |state, cx| {
                let position = if self.activity_follow {
                    lsp_types::Position::new(
                        text.bytes().filter(|byte| *byte == b'\n').count() as u32,
                        text.rsplit('\n')
                            .next()
                            .unwrap_or_default()
                            .encode_utf16()
                            .count() as u32,
                    )
                } else {
                    state.cursor_position()
                };
                state.set_value(text.clone(), window, cx);
                state.set_cursor_position(position, window, cx);
            });
            self.activity_text = text;
        }
    }

    fn edit_remote(&mut self, mode: RemoteEdit, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.active() || self.error.is_some() {
            return;
        }
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        let name = match &mode {
            RemoteEdit::Add => {
                if snapshot.remotes.is_empty() {
                    "origin"
                } else {
                    ""
                }
            }
            RemoteEdit::Rename(name) | RemoteEdit::Url { name, .. } => name,
        };
        let url = match &mode {
            RemoteEdit::Url { name, push } => snapshot
                .remote_urls
                .get(name)
                .and_then(|urls| {
                    (if *push && !urls.push.is_empty() {
                        &urls.push
                    } else {
                        &urls.fetch
                    })
                    .first()
                    .and_then(Option::as_deref)
                })
                .unwrap_or_default(),
            _ => "",
        };
        self.remote_name
            .update(cx, |state, cx| state.set_value(name, window, cx));
        self.remote_url
            .update(cx, |state, cx| state.set_value(url, window, cx));
        let focus = if matches!(mode, RemoteEdit::Url { .. }) {
            self.remote_url.read(cx).focus_handle(cx)
        } else {
            self.remote_name.read(cx).focus_handle(cx)
        };
        self.remote_edit = Some((mode, snapshot));
        window.focus(&focus, cx);
        cx.notify();
    }

    fn save_remote(&mut self, cx: &mut Context<Self>) {
        if self.disabled || self.active() || self.error.is_some() {
            return;
        }
        let Some((mode, expected)) = &self.remote_edit else {
            return;
        };
        let Some(workspace) = self.root.clone() else {
            return;
        };
        let action = match mode {
            RemoteEdit::Add => model::Action::AddRemote {
                name: self.remote_name.read(cx).value().trim().into(),
                url: self.remote_url.read(cx).value().trim().into(),
            },
            RemoteEdit::Rename(name) => model::Action::RenameRemote {
                name: name.clone(),
                new_name: self.remote_name.read(cx).value().trim().into(),
            },
            RemoteEdit::Url { name, push } => model::Action::UpdateRemote {
                name: name.clone(),
                url: self.remote_url.read(cx).value().trim().into(),
                push: *push,
            },
        };
        cx.emit(GitEvent::Requested(Box::new(model::Request {
            workspace,
            expected: expected.clone(),
            action,
        })));
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
            self.text_review = None;
            self.diff_scroll = Default::default();
            self.diff_change = None;
        }
        self.active_file = Some(path.clone());
        self.diff_generation += 1;
        let generation = self.diff_generation;
        let cancellation = Cancellation::default();
        self.diff_cancel = Some(cancellation.clone());
        let task = cx.background_executor().spawn(async move {
            let review = model::review::load_text(&snapshot, &path, &cancellation)
                .map_err(|error| format!("{error:#}"));
            let fallback = if review.is_err() {
                model::diff(&snapshot, &path, &cancellation)
                    .unwrap_or_else(|error| format!("{error:#}"))
            } else {
                String::new()
            };
            (review, fallback)
        });
        cx.spawn_in(window, async move |panel, cx| {
            let (review, fallback) = task.await;
            let _ = panel.update_in(cx, |this, window, cx| {
                if this.diff_generation != generation {
                    return;
                }
                this.diff_cancel = None;
                this.text_review = Some(review);
                this.set_diff(fallback, window, cx);
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

    fn copy_diff(&self, after: bool, cx: &mut Context<Self>) {
        if let Some(Ok(review)) = &self.text_review {
            let lines = if after { &review.after } else { &review.before };
            cx.write_to_clipboard(ClipboardItem::new_string(lines.concat()));
        }
    }

    fn move_diff_change(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(Ok(review)) = &self.text_review else {
            return;
        };
        let count = review.changes.len();
        if count == 0 {
            return;
        }
        let index = match (self.diff_change, forward) {
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
            (None, true) => 0,
            (None, false) => count - 1,
        };
        self.diff_change = Some(index);
        self.diff_scroll
            .scroll_to_item_strict(review.changes[index], ScrollStrategy::Center);
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
            model::Action::Commit { paths, .. } => {
                let comparisons = request
                    .expected
                    .changes
                    .iter()
                    .filter(|change| change.is_comparison() && paths.contains(&change.path))
                    .count();
                format!(
                    "Commit the current contents of {} selected file(s) on '{}'? Other staged files will not be committed.{}",
                    paths.len(),
                    request.expected.branch,
                    if comparisons == 0 {
                        String::new()
                    } else {
                        format!(" Includes {comparisons} explicitly selected comparison file(s).")
                    },
                )
            }
            model::Action::Discard { paths } => format!(
                "Discard all staged and unstaged changes in {} selected file(s)? Tracked files will be restored to the last commit. New files, including staged additions, will be permanently deleted. This cannot be undone.",
                paths.len()
            ),
            model::Action::Pull => format!(
                "Pull '{}' from '{}' ({})? Only a fast-forward is allowed; local files must be clean.",
                request.expected.branch,
                request
                    .expected
                    .pull_remote()
                    .unwrap_or("no selected remote"),
                request
                    .expected
                    .destination(request.expected.pull_remote().unwrap_or_default()),
            ),
            model::Action::Push => format!(
                "Push local commits on '{}' to '{}'? Remote history will not be overwritten.",
                request.expected.branch,
                request
                    .expected
                    .default_remote()
                    .unwrap_or("no selected remote")
            ),
            model::Action::SelectRemote { name } => format!(
                "Select '{name}' for fetch, pull and push in this repository? The current branch and files will not change."
            ),
            model::Action::AddRemote { name, .. } => format!(
                "Add remote '{name}' to this repository? Authentication uses your existing Git configuration."
            ),
            model::Action::UpdateRemote { name, push, .. } => format!(
                "Replace the {} URL for remote '{name}'? Future {} operations will use the new destination.",
                if *push { "push" } else { "fetch" },
                if *push {
                    "push"
                } else {
                    "fetch and pull (and push when no separate push URL exists)"
                }
            ),
            model::Action::RenameRemote { name, new_name } => format!(
                "Rename remote '{name}' to '{new_name}'? Its remote-tracking references and branch configuration will be updated."
            ),
            model::Action::RemoveRemote { name } => format!(
                "Remove remote '{name}' from this repository? Its local remote-tracking references and upstream settings will be removed. Local branches, files and the hosted repository will not be deleted."
            ),
        };
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, window, _cx| {
            let run = request.clone();
            dialog
                .title(request.action.label())
                .child(detail.clone())
                .when(
                    matches!(
                        request.action,
                        model::Action::Commit { .. } | model::Action::Discard { .. }
                    ),
                    |dialog| {
                        let paths = match &request.action {
                            model::Action::Commit { paths, .. }
                            | model::Action::Discard { paths } => paths.clone(),
                            _ => Vec::new(),
                        };
                        dialog.child(div().id("git-confirm-files").test_support().child(
                            div().max_h(px(140.)).overflow_y_scrollbar().children(
                                paths.into_iter().map(|path| div().text_sm().child(path)),
                            ),
                        ))
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
                                .when(
                                    matches!(
                                        request.action,
                                        model::Action::Discard { .. }
                                            | model::Action::RemoveRemote { .. }
                                    ),
                                    |button| button.danger(),
                                )
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
        let cancellation = Cancellation::reporting();
        self.busy = true;
        self.status = format!("{}...", request.action.label());
        self.sync_modal(cx);
        self.git.update(cx, |panel, cx| {
            panel.cancel_all();
            panel.generation += 1;
            panel.diff_generation += 1;
            panel.action = Some(cancellation.clone());
            panel.status = self.status.clone();
            panel.begin_activity(
                request.action.label().into(),
                cancellation.clone(),
                true,
                window,
                cx,
            );
            panel.start_polling(window, cx);
            cx.emit(GitEvent::Changed);
            cx.notify();
        });
        let action = request.action.clone();
        let task = cx.background_executor().spawn(async move {
            let result = request.execute(&cancellation);
            cancellation.stage("Refreshing repository status");
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
                    panel.finish_activity(success, &this.status, window, cx);
                    panel.disabled = false;
                    match snapshot {
                        Ok(snapshot) => panel.install_snapshot(snapshot, window, cx),
                        Err(error) => panel.error = Some(format!("{error:#}")),
                    }
                    if success
                        && matches!(
                            action,
                            model::Action::Commit { .. } | model::Action::Discard { .. }
                        )
                    {
                        panel.selection.clear();
                    }
                    if success && matches!(action, model::Action::Commit { .. }) {
                        panel
                            .message
                            .update(cx, |state, cx| state.set_value("", window, cx));
                    }
                    if success
                        && matches!(
                            action,
                            model::Action::AddRemote { .. }
                                | model::Action::UpdateRemote { .. }
                                | model::Action::RenameRemote { .. }
                        )
                    {
                        panel.remote_edit = None;
                        panel
                            .remote_url
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
                if matches!(action, model::Action::Pull | model::Action::Discard { .. }) {
                    this.reload_after_git_operation(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn reload_after_git_operation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
