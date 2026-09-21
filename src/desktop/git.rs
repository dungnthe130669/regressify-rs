use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};

mod process;

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    pub root: PathBuf,
    pub git_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub original_path: Option<String>,
    pub index: char,
    pub worktree: char,
    pub untracked: bool,
    pub conflict: bool,
    pub submodule: bool,
}

impl Change {
    pub fn label(&self) -> &'static str {
        if self.conflict {
            "Conflict"
        } else if self.untracked {
            "Untracked"
        } else if self.index == 'R' || self.worktree == 'R' {
            "Renamed"
        } else if self.index == 'D' || self.worktree == 'D' {
            "Deleted"
        } else if self.index == 'A' {
            "Added"
        } else {
            "Modified"
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub repository: Option<Repository>,
    pub branch: String,
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub remote: Option<String>,
    pub merge_ref: Option<String>,
    pub remotes: Vec<String>,
    pub remote_config: String,
    pub ahead: usize,
    pub behind: usize,
    pub detached: bool,
    pub operation: Option<String>,
    pub changes: Vec<Change>,
}

impl Snapshot {
    pub fn same_tracking_as(&self, other: &Self) -> bool {
        self.repository == other.repository
            && self.branch == other.branch
            && self.detached == other.detached
            && self.remote == other.remote
            && self.merge_ref == other.merge_ref
            && self.upstream == other.upstream
            && self.remote_config == other.remote_config
    }

    pub fn conflicts(&self) -> usize {
        self.changes.iter().filter(|change| change.conflict).count()
    }
    pub fn default_remote(&self) -> Option<&str> {
        if self.remote.as_deref() == Some(".") {
            return None;
        }
        self.remote
            .as_deref()
            .filter(|remote| *remote != ".")
            .or_else(|| {
                self.remotes
                    .iter()
                    .find(|remote| remote.as_str() == "origin")
                    .map(String::as_str)
            })
            .or_else(|| (self.remotes.len() == 1).then(|| self.remotes[0].as_str()))
    }
}

#[derive(Default)]
pub struct ChangeTree {
    pub folders: BTreeMap<String, ChangeTree>,
    pub files: Vec<usize>,
}

impl ChangeTree {
    pub fn new(changes: &[Change]) -> Self {
        let mut root = Self::default();
        for (index, change) in changes.iter().enumerate() {
            let mut tree = &mut root;
            let components = change.path.split('/').collect::<Vec<_>>();
            for component in &components[..components.len().saturating_sub(1)] {
                tree = tree.folders.entry((*component).into()).or_default();
            }
            tree.files.push(index);
        }
        root
    }
}

pub fn status(workspace: &Path, cancellation: &Cancellation) -> Result<Snapshot> {
    let workspace = workspace
        .canonicalize()
        .context("Workspace is unavailable")?;
    let root = process::run(
        &workspace,
        ["rev-parse", "--show-toplevel"],
        None,
        cancellation,
        Duration::from_secs(15),
    )?;
    if !root.success {
        if root.stderr.contains("not a git repository") {
            return Ok(Snapshot::default());
        }
        bail!("Cannot inspect Git repository: {}", root.stderr.trim());
    }
    let root = PathBuf::from(root.text()?.trim_end_matches(['\r', '\n'])).canonicalize()?;
    let git_dir = command(&root, ["rev-parse", "--absolute-git-dir"], cancellation)?.text()?;
    let repository = Repository {
        root: root.clone(),
        git_dir: PathBuf::from(git_dir.trim_end_matches(['\r', '\n'])),
    };
    let output = command(
        &root,
        [
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
        cancellation,
    )?;
    let mut snapshot = parse_status(&output.stdout)?;
    snapshot.remotes = command(&root, ["remote"], cancellation)?
        .text()?
        .lines()
        .map(str::to_string)
        .collect();
    let remote_config = process::run(
        &root,
        ["config", "--null", "--get-regexp", "^(remote\\.|url\\.)"],
        None,
        cancellation,
        Duration::from_secs(15),
    )?;
    if !remote_config.success && remote_config.code != Some(1) {
        remote_config.ensure_success()?;
    }
    snapshot.remote_config = format!("{:x}", Sha256::digest(&remote_config.stdout));
    if !snapshot.detached && !snapshot.branch.is_empty() {
        snapshot.remote = config_value(
            &root,
            &format!("branch.{}.remote", snapshot.branch),
            cancellation,
        )?;
        snapshot.merge_ref = config_value(
            &root,
            &format!("branch.{}.merge", snapshot.branch),
            cancellation,
        )?;
    }
    for (path, name) in [
        ("MERGE_HEAD", "Merge in progress"),
        ("rebase-merge", "Rebase in progress"),
        ("rebase-apply", "Rebase in progress"),
        ("CHERRY_PICK_HEAD", "Cherry-pick in progress"),
        ("REVERT_HEAD", "Revert in progress"),
    ] {
        if repository.git_dir.join(path).exists() {
            snapshot.operation = Some(name.into());
            break;
        }
    }
    snapshot.repository = Some(repository);
    Ok(snapshot)
}

fn config_value(root: &Path, key: &str, cancellation: &Cancellation) -> Result<Option<String>> {
    let result = process::run(
        root,
        ["config", "--get", key],
        None,
        cancellation,
        Duration::from_secs(15),
    )?;
    if result.success {
        Ok(Some(
            result.text()?.trim_end_matches(['\r', '\n']).to_string(),
        ))
    } else if result.code == Some(1) {
        Ok(None)
    } else {
        bail!(
            "Cannot read Git branch configuration: {}",
            result.stderr.trim()
        )
    }
}

pub fn fetch(snapshot: &Snapshot, cancellation: &Cancellation) -> Result<()> {
    let repository = snapshot
        .repository
        .as_ref()
        .context("Workspace is not a Git repository")?;
    let remote = snapshot
        .default_remote()
        .context("No tracking remote; configure a remote in VS Code")?;
    ensure!(
        snapshot.remotes.iter().any(|name| name == remote),
        "Tracking remote is no longer configured"
    );
    let output = process::run(
        &repository.root,
        [
            "-c",
            "core.sshCommand=ssh -oBatchMode=yes -oStrictHostKeyChecking=yes",
            "fetch",
            "--quiet",
            "--prune",
            "--no-tags",
            "--no-recurse-submodules",
            "--",
            remote,
        ],
        None,
        cancellation,
        Duration::from_secs(90),
    )?;
    output.ensure_success()
}

pub fn diff(snapshot: &Snapshot, path: &str, cancellation: &Cancellation) -> Result<String> {
    let repository = snapshot
        .repository
        .as_ref()
        .context("Workspace is not a Git repository")?;
    let change = snapshot
        .changes
        .iter()
        .find(|change| change.path == path)
        .context("File is no longer changed; refresh Git status")?;
    if change.untracked {
        let output = process::run(
            &repository.root,
            [
                "diff",
                "--no-index",
                "--no-ext-diff",
                "--no-textconv",
                "--",
                "/dev/null",
                path,
            ],
            None,
            cancellation,
            Duration::from_secs(15),
        )?;
        ensure!(
            output.success || output.code == Some(1),
            "Cannot review file: {}",
            output.stderr
        );
        return Ok(bounded_diff(&output.text()?));
    }
    let mut result = String::new();
    for (label, cached) in [("Staged changes", true), ("Unstaged changes", false)] {
        let mut args = vec!["diff", "--no-ext-diff", "--no-textconv"];
        if cached {
            args.push("--cached");
        }
        args.extend(["--", path]);
        if let Some(original) = &change.original_path {
            args.push(original);
        }
        let output = command(&repository.root, args, cancellation)?.text()?;
        if !output.is_empty() {
            result.push_str(&format!("{label}\n\n{output}\n"));
        }
    }
    if result.is_empty() {
        result = "No textual diff. The file may have changed since the last refresh, or contain submodule/conflict state. Open in VS Code for details.".into();
    }
    Ok(bounded_diff(&result))
}

fn bounded_diff(text: &str) -> String {
    const MAX_DIFF_BYTES: usize = 512 * 1024;
    if text.len() <= MAX_DIFF_BYTES {
        return text.into();
    }
    let mut end = MAX_DIFF_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n\n[Diff preview truncated. Open in VS Code to review the entire file.]",
        &text[..end]
    )
}

#[derive(Clone, Debug)]
pub enum Action {
    Initialize,
    Commit { paths: Vec<String>, message: String },
    Discard { paths: Vec<String> },
    Pull,
    Push,
    AddOrigin { url: String },
}

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Initialize => "Initialize repository",
            Self::Commit { .. } => "Commit selected files",
            Self::Discard { .. } => "Discard selected files",
            Self::Pull => "Pull",
            Self::Push => "Push",
            Self::AddOrigin { .. } => "Add origin",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub workspace: PathBuf,
    pub expected: Snapshot,
    pub action: Action,
}

impl Request {
    pub fn execute(&self, cancellation: &Cancellation) -> Result<String> {
        let current = status(&self.workspace, cancellation)?;
        if matches!(self.action, Action::Initialize) {
            ensure!(
                current.repository.is_none(),
                "Workspace is already inside a Git repository; refresh before continuing"
            );
            command(&self.workspace, ["init"], cancellation)?;
            return Ok("Git repository initialized; files have not been staged".into());
        }
        let repository = current
            .repository
            .as_ref()
            .context("Workspace is no longer tracked by Git")?;
        ensure!(
            current.repository == self.expected.repository
                && current.head == self.expected.head
                && current.branch == self.expected.branch,
            "Git branch or HEAD changed; refresh and review before continuing"
        );
        if matches!(
            self.action,
            Action::Pull | Action::Push | Action::AddOrigin { .. }
        ) {
            ensure!(
                current.remote_config == self.expected.remote_config
                    && current.remote == self.expected.remote
                    && current.merge_ref == self.expected.merge_ref
                    && current.upstream == self.expected.upstream,
                "Git remote or upstream changed; refresh and confirm the destination again"
            );
        }
        match &self.action {
            Action::Initialize => unreachable!(),
            Action::Commit { paths, message } => {
                ensure!(
                    current.operation.is_none() && current.conflicts() == 0,
                    "Resolve and complete the merge/rebase in VS Code before committing here"
                );
                ensure!(
                    !current.detached,
                    "HEAD is detached; choose a branch in VS Code first"
                );
                ensure!(!paths.is_empty(), "Select at least one changed file");
                ensure!(
                    !message.trim().is_empty()
                        && message.len() <= 64 * 1024
                        && !message.contains('\0'),
                    "Enter a commit message (maximum 64 KiB)"
                );
                let mut selected = std::collections::BTreeSet::new();
                let mut staging = std::collections::BTreeSet::new();
                for path in paths {
                    let change = current
                        .changes
                        .iter()
                        .find(|change| &change.path == path)
                        .with_context(|| {
                            format!(
                                "Selected file '{path}' is no longer changed; refresh and review"
                            )
                        })?;
                    ensure!(!change.submodule, "Use VS Code to commit submodule changes");
                    selected.insert(change.path.clone());
                    if change.untracked || change.worktree != '.' {
                        staging.insert(change.path.clone());
                    }
                    if let Some(original) = &change.original_path {
                        selected.insert(original.clone());
                    }
                }
                let mut pathspec = tempfile::NamedTempFile::new()?;
                use std::io::Write;
                for path in selected {
                    pathspec.write_all(path.as_bytes())?;
                    pathspec.write_all(&[0])?;
                }
                pathspec.flush()?;
                let pathspec_arg = format!(
                    "--pathspec-from-file={}",
                    pathspec
                        .path()
                        .to_str()
                        .context("Git pathspec file must be UTF-8")?
                );
                if !staging.is_empty() {
                    let mut input = Vec::new();
                    for path in staging {
                        input.extend_from_slice(path.as_bytes());
                        input.push(0);
                    }
                    process::run(
                        &repository.root,
                        ["add", "-A", "--pathspec-from-file=-", "--pathspec-file-nul"],
                        Some(input),
                        cancellation,
                        Duration::from_secs(30),
                    )?
                    .ensure_success()?;
                }
                let output = process::run(
                    &repository.root,
                    [
                        "commit",
                        "--only",
                        &pathspec_arg,
                        "--pathspec-file-nul",
                        "--file=-",
                    ],
                    Some(message.as_bytes().to_vec()),
                    cancellation,
                    Duration::from_secs(120),
                )?;
                output.ensure_success()?;
                Ok(format!("Committed {} selected file(s)", paths.len()))
            }
            Action::Discard { paths } => {
                ensure!(
                    current.operation.is_none() && current.conflicts() == 0,
                    "Resolve the current Git operation in VS Code before discarding here"
                );
                ensure!(!paths.is_empty(), "Select at least one changed file");
                let selected = paths
                    .iter()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>();
                let mut tracked = std::collections::BTreeSet::new();
                let mut untracked = std::collections::BTreeSet::new();
                for path in &selected {
                    let changes = current
                        .changes
                        .iter()
                        .filter(|change| &change.path == path)
                        .collect::<Vec<_>>();
                    let expected = self
                        .expected
                        .changes
                        .iter()
                        .filter(|change| &change.path == path)
                        .collect::<Vec<_>>();
                    ensure!(
                        !changes.is_empty() && changes == expected,
                        "Selected file '{path}' changed Git status; refresh and review before discarding"
                    );
                    for change in changes {
                        ensure!(
                            !change.submodule,
                            "Use VS Code to discard submodule changes"
                        );
                        if change.untracked {
                            untracked.insert(path.clone());
                        } else {
                            tracked.insert(path.clone());
                        }
                        if (change.index == 'R' || change.worktree == 'R')
                            && let Some(original) = &change.original_path
                        {
                            let original_file = discard_file_path(&repository.root, original)?;
                            ensure!(
                                selected.contains(original)
                                    || (!original_file.try_exists()?
                                        && !original_file.is_symlink()),
                                "Discarding '{path}' would overwrite unselected file '{original}'; select both files or use VS Code"
                            );
                            tracked.insert(original.clone());
                        }
                    }
                }
                for path in tracked.iter().chain(untracked.iter()) {
                    discard_file_path(&repository.root, path)?;
                }
                for path in &tracked {
                    let prefix = format!("{path}/");
                    ensure!(
                        current.changes.iter().all(|change| {
                            !change.path.starts_with(&prefix) || selected.contains(&change.path)
                        }),
                        "Discarding '{path}' would affect unselected files below it; select those files or use VS Code"
                    );
                }
                for path in &tracked {
                    untracked.remove(path);
                }
                if !tracked.is_empty() {
                    let mut input = Vec::new();
                    for path in &tracked {
                        input.extend_from_slice(path.as_bytes());
                        input.push(0);
                    }
                    let args = if current.head.is_some() {
                        vec![
                            "restore",
                            "--source=HEAD",
                            "--staged",
                            "--worktree",
                            "--pathspec-from-file=-",
                            "--pathspec-file-nul",
                        ]
                    } else {
                        untracked.extend(tracked);
                        vec![
                            "rm",
                            "--cached",
                            "--force",
                            "--pathspec-from-file=-",
                            "--pathspec-file-nul",
                        ]
                    };
                    process::run(
                        &repository.root,
                        args,
                        Some(input),
                        cancellation,
                        Duration::from_secs(30),
                    )?
                    .ensure_success()?;
                }
                for path in untracked {
                    ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
                    let file = discard_file_path(&repository.root, &path)?;
                    match std::fs::remove_file(file) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => {
                            return Err(error).with_context(|| format!("Cannot discard '{path}'"));
                        }
                    }
                }
                Ok(format!(
                    "Discarded changes in {} selected file(s)",
                    selected.len()
                ))
            }
            Action::Pull => {
                ensure!(
                    current.operation.is_none() && current.conflicts() == 0,
                    "Resolve the current Git operation in VS Code before pulling"
                );
                ensure!(
                    !current.detached && current.head.is_some(),
                    "Pull requires a branch with a commit"
                );
                ensure!(
                    current.changes.is_empty(),
                    "Commit local changes before pulling, or use VS Code to manage them"
                );
                ensure!(
                    current.remote.is_some() && current.merge_ref.is_some(),
                    "No upstream branch; configure tracking in VS Code first"
                );
                let output = process::run(
                    &repository.root,
                    [
                        "-c",
                        "core.sshCommand=ssh -oBatchMode=yes -oStrictHostKeyChecking=yes",
                        "-c",
                        "merge.autoStash=false",
                        "-c",
                        "rebase.autoStash=false",
                        "pull",
                        "--ff-only",
                        "--no-rebase",
                        "--no-edit",
                        "--no-recurse-submodules",
                    ],
                    None,
                    cancellation,
                    Duration::from_secs(120),
                )?;
                output.ensure_success().context(
                    "Pull stopped without resolving divergence; use VS Code to merge or rebase",
                )?;
                Ok("Pull completed (fast-forward only)".into())
            }
            Action::Push => {
                ensure!(
                    current.operation.is_none() && current.conflicts() == 0,
                    "Resolve the current Git operation in VS Code before pushing"
                );
                ensure!(
                    !current.detached && current.head.is_some(),
                    "Create a commit on a branch before pushing"
                );
                let remote = current
                    .default_remote()
                    .context("No remote configured; add origin first")?;
                ensure!(
                    current.remotes.iter().any(|name| name == remote),
                    "Tracking remote is unavailable"
                );
                let destination = if current.remote.as_deref() == Some(remote) {
                    current
                        .merge_ref
                        .clone()
                        .unwrap_or_else(|| format!("refs/heads/{}", current.branch))
                } else {
                    format!("refs/heads/{}", current.branch)
                };
                ensure!(
                    destination.starts_with("refs/heads/"),
                    "Upstream is not a branch; configure it in VS Code"
                );
                let source = format!("refs/heads/{}:{destination}", current.branch);
                let output = process::run(
                    &repository.root,
                    [
                        "-c",
                        "core.sshCommand=ssh -oBatchMode=yes -oStrictHostKeyChecking=yes",
                        "push",
                        "--porcelain",
                        "--set-upstream",
                        "--",
                        remote,
                        &source,
                    ],
                    None,
                    cancellation,
                    Duration::from_secs(120),
                )?;
                output
                    .ensure_success()
                    .context("Push failed; fetch and review incoming commits before retrying")?;
                Ok("Push completed".into())
            }
            Action::AddOrigin { url } => {
                ensure!(
                    !current.remotes.iter().any(|remote| remote == "origin"),
                    "Origin already exists; edit remotes in VS Code"
                );
                ensure!(
                    !url.trim().is_empty()
                        && url == url.trim()
                        && url.len() <= 4096
                        && !url.starts_with('-')
                        && !url.contains("::")
                        && !url.chars().any(char::is_control),
                    "Enter a valid remote URL or repository path"
                );
                if url.contains("://") {
                    let parsed = reqwest::Url::parse(url).context("Invalid remote URL")?;
                    ensure!(
                        matches!(parsed.scheme(), "https" | "ssh" | "file")
                            && parsed.password().is_none()
                            && parsed.query().is_none()
                            && parsed.fragment().is_none()
                            && (parsed.scheme() != "https" || parsed.username().is_empty()),
                        "Use a credential-free HTTPS or SSH remote; configure authentication outside the app"
                    );
                }
                command(
                    &repository.root,
                    ["remote", "add", "origin", url],
                    cancellation,
                )?;
                Ok("Origin configured".into())
            }
        }
    }
}

fn discard_file_path(root: &Path, path: &str) -> Result<PathBuf> {
    let relative = Path::new(path);
    ensure!(
        !path.is_empty()
            && relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "Invalid selected Git path '{path}'"
    );
    let mut parent = root.to_path_buf();
    if let Some(ancestors) = relative.parent() {
        for component in ancestors.components() {
            parent.push(component);
            match parent.symlink_metadata() {
                Ok(metadata) => ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "Parent of '{path}' is not a regular directory; review in VS Code"
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) => {
                    return Err(error).with_context(|| format!("Cannot inspect '{path}'"));
                }
            }
        }
    }
    let file = root.join(relative);
    match file.symlink_metadata() {
        Ok(metadata) => ensure!(
            !metadata.is_dir(),
            "Selected path '{path}' is a directory; select its individual changed files or use VS Code"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("Cannot inspect '{path}'")),
    }
    Ok(file)
}

fn command(
    root: &Path,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
    cancellation: &Cancellation,
) -> Result<process::Output> {
    let output = process::run(root, args, None, cancellation, Duration::from_secs(30))?;
    output.ensure_success()?;
    Ok(output)
}

fn parse_status(bytes: &[u8]) -> Result<Snapshot> {
    let text = std::str::from_utf8(bytes)
        .context("Git filenames must be valid UTF-8 for desktop review")?;
    let mut snapshot = Snapshot::default();
    let mut records = text.split('\0').filter(|record| !record.is_empty());
    while let Some(record) = records.next() {
        if let Some(value) = record.strip_prefix("# branch.head ") {
            snapshot.detached = value == "(detached)";
            snapshot.branch = value.into();
        } else if let Some(value) = record.strip_prefix("# branch.oid ") {
            snapshot.head = (value != "(initial)").then(|| value.into());
        } else if let Some(value) = record.strip_prefix("# branch.upstream ") {
            snapshot.upstream = Some(value.into());
        } else if let Some(value) = record.strip_prefix("# branch.ab ") {
            let (ahead, behind) = value.split_once(' ').context("Invalid Git branch counts")?;
            snapshot.ahead = ahead
                .strip_prefix('+')
                .context("Invalid ahead count")?
                .parse()?;
            snapshot.behind = behind
                .strip_prefix('-')
                .context("Invalid behind count")?
                .parse()?;
        } else if let Some(path) = record.strip_prefix("? ") {
            snapshot.changes.push(Change {
                path: path.into(),
                original_path: None,
                index: '?',
                worktree: '?',
                untracked: true,
                conflict: false,
                submodule: false,
            });
        } else if matches!(record.as_bytes().first(), Some(b'1' | b'2' | b'u')) {
            let kind = record.as_bytes()[0];
            let count = match kind {
                b'1' => 9,
                b'2' => 10,
                _ => 11,
            };
            let parts = record.splitn(count, ' ').collect::<Vec<_>>();
            ensure!(
                parts.len() == count && parts[1].len() == 2,
                "Invalid Git change record"
            );
            snapshot.changes.push(Change {
                path: parts[count - 1].into(),
                original_path: if kind == b'2' {
                    Some(
                        records
                            .next()
                            .context("Git rename has no original path")?
                            .into(),
                    )
                } else {
                    None
                },
                index: parts[1].as_bytes()[0] as char,
                worktree: parts[1].as_bytes()[1] as char,
                untracked: false,
                conflict: kind == b'u',
                submodule: parts[2].starts_with('S'),
            });
        }
    }
    snapshot
        .changes
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn git(root: &Path, args: &[&str]) -> String {
        command(root, args, &Cancellation::default())
            .unwrap()
            .text()
            .unwrap()
    }

    fn repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        git(directory.path(), &["init", "-b", "main"]);
        git(directory.path(), &["config", "user.name", "Git UI fixture"]);
        git(
            directory.path(),
            &["config", "user.email", "fixture@example.test"],
        );
        git(directory.path(), &["config", "commit.gpgsign", "false"]);
        git(directory.path(), &["config", "core.autocrlf", "false"]);
        git(
            directory.path(),
            &["config", "core.hooksPath", "disabled-hooks"],
        );
        directory
    }

    fn execute(root: &Path, action: Action) -> Result<String> {
        Request {
            workspace: root.into(),
            expected: status(root, &Cancellation::default())?,
            action,
        }
        .execute(&Cancellation::default())
    }

    #[test]
    fn selected_commit_preserves_unrelated_staged_files_and_handles_renames_deletions() {
        let root = repository();
        for file in ["chosen.txt", "staged.txt", "old.txt", "deleted.txt"] {
            fs::write(root.path().join(file), "initial\n").unwrap();
        }
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        fs::write(root.path().join("chosen.txt"), "selected\n").unwrap();
        fs::write(root.path().join("staged.txt"), "leave staged\n").unwrap();
        git(root.path(), &["add", "staged.txt"]);
        git(root.path(), &["mv", "old.txt", "renamed [file].txt"]);
        fs::remove_file(root.path().join("deleted.txt")).unwrap();
        fs::write(root.path().join("-new.txt"), "new\n").unwrap();
        execute(
            root.path(),
            Action::Commit {
                paths: vec![
                    "chosen.txt".into(),
                    "renamed [file].txt".into(),
                    "deleted.txt".into(),
                    "-new.txt".into(),
                ],
                message: "selected files only".into(),
            },
        )
        .unwrap();
        assert_eq!(git(root.path(), &["show", "HEAD:staged.txt"]), "initial\n");
        assert_eq!(git(root.path(), &["show", ":staged.txt"]), "leave staged\n");
        let snapshot = status(root.path(), &Cancellation::default()).unwrap();
        assert_eq!(snapshot.changes.len(), 1);
        assert_eq!(snapshot.changes[0].path, "staged.txt");
        assert_eq!(snapshot.changes[0].index, 'M');
    }

    #[test]
    fn selected_discard_restores_tracked_files_and_preserves_unselected_changes() {
        let root = repository();
        let originals = [
            "chosen.txt",
            "staged.txt",
            "old.txt",
            "deleted.txt",
            "cached-deleted.txt",
            "replaced.txt",
        ];
        for file in originals {
            fs::write(root.path().join(file), format!("initial {file}\n")).unwrap();
        }
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        let head = git(root.path(), &["rev-parse", "HEAD"]);
        for file in ["chosen.txt", "staged.txt"] {
            fs::write(root.path().join(file), "staged contents\n").unwrap();
            git(root.path(), &["add", file]);
            fs::write(root.path().join(file), "working contents\n").unwrap();
        }
        git(root.path(), &["mv", "old.txt", "renamed [file].txt"]);
        fs::remove_file(root.path().join("deleted.txt")).unwrap();
        git(root.path(), &["rm", "cached-deleted.txt"]);
        git(root.path(), &["rm", "--cached", "replaced.txt"]);
        fs::write(root.path().join("replaced.txt"), "replacement\n").unwrap();
        fs::write(root.path().join("added [file].txt"), "staged addition\n").unwrap();
        git(root.path(), &["add", "added [file].txt"]);
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/-new.txt"), "untracked\n").unwrap();
        fs::write(root.path().join("nested/keep.txt"), "keep\n").unwrap();
        execute(
            root.path(),
            Action::Discard {
                paths: vec![
                    "chosen.txt".into(),
                    "renamed [file].txt".into(),
                    "deleted.txt".into(),
                    "cached-deleted.txt".into(),
                    "replaced.txt".into(),
                    "added [file].txt".into(),
                    "nested/-new.txt".into(),
                ],
            },
        )
        .unwrap();
        for file in originals.into_iter().filter(|file| *file != "staged.txt") {
            assert_eq!(
                fs::read_to_string(root.path().join(file)).unwrap(),
                format!("initial {file}\n")
            );
        }
        for file in ["renamed [file].txt", "added [file].txt", "nested/-new.txt"] {
            assert!(!root.path().join(file).exists());
        }
        assert_eq!(git(root.path(), &["rev-parse", "HEAD"]), head);
        assert_eq!(
            git(root.path(), &["show", ":staged.txt"]),
            "staged contents\n"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("staged.txt")).unwrap(),
            "working contents\n"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("nested/keep.txt")).unwrap(),
            "keep\n"
        );
        let snapshot = status(root.path(), &Cancellation::default()).unwrap();
        assert_eq!(snapshot.changes.len(), 2);
    }

    #[test]
    fn selected_discard_works_before_the_first_commit() {
        let root = repository();
        for file in ["added.txt", "new.txt", "keep-staged.txt", "keep-new.txt"] {
            fs::write(root.path().join(file), file).unwrap();
        }
        git(root.path(), &["add", "added.txt", "keep-staged.txt"]);
        execute(
            root.path(),
            Action::Discard {
                paths: vec!["added.txt".into(), "new.txt".into()],
            },
        )
        .unwrap();
        assert!(!root.path().join("added.txt").exists());
        assert!(!root.path().join("new.txt").exists());
        assert_eq!(
            git(root.path(), &["show", ":keep-staged.txt"]),
            "keep-staged.txt"
        );
        assert!(root.path().join("keep-new.txt").exists());
        assert_eq!(
            status(root.path(), &Cancellation::default())
                .unwrap()
                .changes
                .len(),
            2
        );
    }

    #[test]
    fn selected_discard_rejects_stale_requests_and_directory_replacements_before_mutation() {
        let root = repository();
        fs::write(root.path().join("file.txt"), "initial\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        fs::write(root.path().join("file.txt"), "changed\n").unwrap();
        fs::write(root.path().join("new.txt"), "keep\n").unwrap();
        let request = Request {
            workspace: root.path().into(),
            expected: status(root.path(), &Cancellation::default()).unwrap(),
            action: Action::Discard {
                paths: vec!["new.txt".into(), "file.txt".into()],
            },
        };
        git(root.path(), &["add", "file.txt"]);
        assert!(
            request
                .execute(&Cancellation::default())
                .unwrap_err()
                .to_string()
                .contains("changed Git status")
        );
        assert_eq!(
            fs::read_to_string(root.path().join("file.txt")).unwrap(),
            "changed\n"
        );
        assert!(root.path().join("new.txt").exists());
        fs::remove_file(root.path().join("file.txt")).unwrap();
        fs::create_dir(root.path().join("file.txt")).unwrap();
        fs::write(root.path().join("file.txt/keep.txt"), "keep\n").unwrap();
        assert!(
            execute(root.path(), request.action.clone())
                .unwrap_err()
                .to_string()
                .contains("is a directory")
        );
        assert!(root.path().join("file.txt/keep.txt").exists());
        assert!(root.path().join("new.txt").exists());
        assert_eq!(git(root.path(), &["show", ":file.txt"]), "changed\n");
        for paths in [vec![], vec!["new.txt".into(), "../outside.txt".into()]] {
            assert!(execute(root.path(), Action::Discard { paths }).is_err());
            assert!(root.path().join("new.txt").exists());
        }
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(
            request
                .execute(&cancellation)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        assert!(root.path().join("new.txt").exists());
    }

    #[test]
    fn selected_discard_rejects_pathspecs_covering_unselected_staged_descendants() {
        let root = repository();
        fs::write(root.path().join("replaced.txt"), "original file\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        fs::remove_file(root.path().join("replaced.txt")).unwrap();
        fs::create_dir(root.path().join("replaced.txt")).unwrap();
        let child = root.path().join("replaced.txt/keep.txt");
        fs::write(&child, "unselected staged contents\n").unwrap();
        git(root.path(), &["add", "-A"]);
        fs::remove_file(child).unwrap();
        fs::remove_dir(root.path().join("replaced.txt")).unwrap();
        let error = execute(
            root.path(),
            Action::Discard {
                paths: vec!["replaced.txt".into()],
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("unselected files below it"));
        assert_eq!(
            git(root.path(), &["show", ":replaced.txt/keep.txt"]),
            "unselected staged contents\n"
        );
        assert!(!root.path().join("replaced.txt").exists());
    }

    #[test]
    fn selected_discard_does_not_overwrite_an_unselected_rename_source() {
        let root = repository();
        fs::write(root.path().join("old.txt"), "initial\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        git(root.path(), &["mv", "old.txt", "renamed.txt"]);
        fs::write(root.path().join("old.txt"), "unselected replacement\n").unwrap();
        assert!(
            execute(
                root.path(),
                Action::Discard {
                    paths: vec!["renamed.txt".into()]
                },
            )
            .unwrap_err()
            .to_string()
            .contains("unselected file")
        );
        assert_eq!(
            fs::read_to_string(root.path().join("old.txt")).unwrap(),
            "unselected replacement\n"
        );
        assert!(root.path().join("renamed.txt").exists());
        execute(
            root.path(),
            Action::Discard {
                paths: vec!["renamed.txt".into(), "old.txt".into()],
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("old.txt")).unwrap(),
            "initial\n"
        );
        assert!(!root.path().join("renamed.txt").exists());
        assert!(
            status(root.path(), &Cancellation::default())
                .unwrap()
                .changes
                .is_empty()
        );
    }

    #[test]
    fn init_first_commit_and_push_establish_tracking_without_force() {
        let root = tempfile::tempdir().unwrap();
        execute(root.path(), Action::Initialize).unwrap();
        git(root.path(), &["config", "user.name", "Git UI fixture"]);
        git(
            root.path(),
            &["config", "user.email", "fixture@example.test"],
        );
        git(root.path(), &["config", "commit.gpgsign", "false"]);
        git(root.path(), &["config", "core.hooksPath", "disabled-hooks"]);
        fs::write(root.path().join("first.txt"), "first\n").unwrap();
        fs::write(root.path().join("other.txt"), "other\n").unwrap();
        execute(
            root.path(),
            Action::Commit {
                paths: vec!["first.txt".into()],
                message: "first commit".into(),
            },
        )
        .unwrap();
        assert_eq!(
            git(root.path(), &["ls-tree", "--name-only", "HEAD"]).trim(),
            "first.txt"
        );
        let remote = tempfile::tempdir().unwrap();
        git(remote.path(), &["init", "--bare"]);
        execute(
            root.path(),
            Action::AddOrigin {
                url: remote.path().to_string_lossy().into(),
            },
        )
        .unwrap();
        execute(root.path(), Action::Push).unwrap();
        let snapshot = status(root.path(), &Cancellation::default()).unwrap();
        assert!(snapshot.upstream.is_some());
        assert_eq!(snapshot.ahead, 0);
        assert!(
            execute(root.path(), Action::Pull)
                .unwrap_err()
                .to_string()
                .contains("local changes")
        );
    }

    #[test]
    fn stale_branch_requests_and_existing_conflicts_are_rejected() {
        let root = repository();
        fs::write(root.path().join("file.txt"), "initial\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        let request = Request {
            workspace: root.path().into(),
            expected: status(root.path(), &Cancellation::default()).unwrap(),
            action: Action::Commit {
                paths: vec!["file.txt".into()],
                message: "stale".into(),
            },
        };
        git(root.path(), &["checkout", "-b", "other"]);
        fs::write(root.path().join("file.txt"), "other\n").unwrap();
        assert!(
            request
                .execute(&Cancellation::default())
                .unwrap_err()
                .to_string()
                .contains("HEAD changed")
        );
        git(root.path(), &["commit", "-am", "other"]);
        git(root.path(), &["checkout", "main"]);
        fs::write(root.path().join("file.txt"), "main\n").unwrap();
        git(root.path(), &["commit", "-am", "main"]);
        let merged = process::run(
            root.path(),
            ["merge", "other"],
            None,
            &Cancellation::default(),
            Duration::from_secs(15),
        )
        .unwrap();
        assert!(!merged.success);
        let snapshot = status(root.path(), &Cancellation::default()).unwrap();
        assert_eq!(snapshot.conflicts(), 1);
        assert_eq!(snapshot.operation.as_deref(), Some("Merge in progress"));
        assert!(
            execute(
                root.path(),
                Action::Commit {
                    paths: vec!["file.txt".into()],
                    message: "blocked".into()
                }
            )
            .unwrap_err()
            .to_string()
            .contains("VS Code")
        );
    }

    #[test]
    fn network_requests_reject_remote_configuration_changes_without_exposing_urls() {
        let root = repository();
        fs::write(root.path().join("file.txt"), "initial\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        git(
            root.path(),
            &["remote", "add", "origin", "https://example.test/first.git"],
        );
        let request = Request {
            workspace: root.path().into(),
            expected: status(root.path(), &Cancellation::default()).unwrap(),
            action: Action::Push,
        };
        git(
            root.path(),
            &[
                "remote",
                "set-url",
                "origin",
                "https://user:private@example.test/changed.git",
            ],
        );
        let error = request
            .execute(&Cancellation::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("remote or upstream changed"));
        assert!(
            !error.contains("private")
                && !format!(
                    "{:?}",
                    status(root.path(), &Cancellation::default()).unwrap()
                )
                .contains("private")
        );
    }

    #[test]
    fn status_distinguishes_untracked_workspaces_and_counts_real_file_changes() {
        let untracked = tempfile::tempdir().unwrap();
        assert!(
            status(untracked.path(), &Cancellation::default())
                .unwrap()
                .repository
                .is_none()
        );
        let root = repository();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/file name.txt"), "original").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        git(
            root.path(),
            &["mv", "nested/file name.txt", "nested/renamed.txt"],
        );
        fs::write(root.path().join("new.txt"), "untracked").unwrap();
        let snapshot = status(&root.path().join("nested"), &Cancellation::default()).unwrap();
        assert_eq!(
            snapshot.repository.unwrap().root,
            root.path().canonicalize().unwrap()
        );
        assert_eq!(snapshot.branch, "main");
        assert_eq!(snapshot.changes.len(), 2);
        assert_eq!(
            snapshot.changes[0].original_path.as_deref(),
            Some("nested/file name.txt")
        );
        assert_eq!(snapshot.changes[0].label(), "Renamed");
        assert!(snapshot.changes[1].untracked);
        let tree = ChangeTree::new(&snapshot.changes);
        assert_eq!(tree.folders["nested"].files, [0]);
        assert_eq!(tree.files, [1]);
    }

    #[test]
    fn remote_refresh_detects_new_commits_without_changing_the_worktree() {
        let server = tempfile::tempdir().unwrap();
        git(server.path(), &["init", "--bare", "-b", "main"]);
        let local = repository();
        fs::write(local.path().join("tracked.txt"), "first\n").unwrap();
        git(local.path(), &["add", "."]);
        git(local.path(), &["commit", "-m", "initial"]);
        git(
            local.path(),
            &["remote", "add", "origin", server.path().to_str().unwrap()],
        );
        git(local.path(), &["push", "-u", "origin", "main"]);
        let other = repository();
        git(
            other.path(),
            &["remote", "add", "origin", server.path().to_str().unwrap()],
        );
        git(other.path(), &["pull", "origin", "main"]);
        fs::write(other.path().join("tracked.txt"), "remote change\n").unwrap();
        git(other.path(), &["commit", "-am", "remote update"]);
        git(other.path(), &["push", "origin", "main"]);
        let before = status(local.path(), &Cancellation::default()).unwrap();
        assert_eq!(before.behind, 0);
        fetch(&before, &Cancellation::default()).unwrap();
        let after = status(local.path(), &Cancellation::default()).unwrap();
        assert_eq!((after.ahead, after.behind), (0, 1));
        assert_eq!(before.head, after.head);
        assert_eq!(
            fs::read_to_string(local.path().join("tracked.txt")).unwrap(),
            "first\n"
        );
        execute(local.path(), Action::Pull).unwrap();
        assert_eq!(
            fs::read_to_string(local.path().join("tracked.txt")).unwrap(),
            "remote change\n"
        );
        fs::write(local.path().join("local.txt"), "local\n").unwrap();
        execute(
            local.path(),
            Action::Commit {
                paths: vec!["local.txt".into()],
                message: "local branch".into(),
            },
        )
        .unwrap();
        fs::write(other.path().join("tracked.txt"), "newer remote\n").unwrap();
        git(other.path(), &["commit", "-am", "newer remote"]);
        git(other.path(), &["push", "origin", "main"]);
        fetch(
            &status(local.path(), &Cancellation::default()).unwrap(),
            &Cancellation::default(),
        )
        .unwrap();
        let diverged = status(local.path(), &Cancellation::default()).unwrap();
        assert_eq!((diverged.ahead, diverged.behind), (1, 1));
        assert!(execute(local.path(), Action::Pull).is_err());
        assert_eq!(
            status(local.path(), &Cancellation::default()).unwrap().head,
            diverged.head
        );
        let remote_head = git(server.path(), &["rev-parse", "refs/heads/main"]);
        assert!(execute(local.path(), Action::Push).is_err());
        assert_eq!(
            git(server.path(), &["rev-parse", "refs/heads/main"]),
            remote_head
        );
    }

    #[test]
    fn diff_previews_include_staged_unstaged_and_literal_untracked_names() {
        let root = repository();
        fs::write(root.path().join("tracked.txt"), "first\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        fs::write(root.path().join("tracked.txt"), "staged\n").unwrap();
        git(root.path(), &["add", "tracked.txt"]);
        fs::write(root.path().join("tracked.txt"), "working\n").unwrap();
        fs::write(root.path().join("-file[1].txt"), "new content\n").unwrap();
        let snapshot = status(root.path(), &Cancellation::default()).unwrap();
        let preview = diff(&snapshot, "tracked.txt", &Cancellation::default()).unwrap();
        assert!(preview.contains("Staged changes") && preview.contains("Unstaged changes"));
        assert!(
            diff(&snapshot, "-file[1].txt", &Cancellation::default())
                .unwrap()
                .contains("+new content")
        );
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(
            status(root.path(), &cancelled)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
    }

    #[test]
    fn porcelain_handles_branch_counts_conflicts_and_embedded_whitespace() {
        let snapshot = parse_status(b"# branch.head main\0# branch.oid abc\0# branch.upstream origin/main\0# branch.ab +2 -3\0u UU N... 100644 100644 100644 100644 a b c conflict file.txt\0? nested/new\nline.txt\0").unwrap();
        assert_eq!(
            (snapshot.ahead, snapshot.behind, snapshot.conflicts()),
            (2, 3, 1)
        );
        assert_eq!(snapshot.changes[1].path, "nested/new\nline.txt");
    }
}
