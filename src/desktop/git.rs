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
    pub ahead: usize,
    pub behind: usize,
    pub detached: bool,
    pub operation: Option<String>,
    pub changes: Vec<Change>,
}

impl Snapshot {
    pub fn conflicts(&self) -> usize {
        self.changes.iter().filter(|change| change.conflict).count()
    }
    pub fn default_remote(&self) -> Option<&str> {
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
        git(
            directory.path(),
            &["config", "core.hooksPath", "disabled-hooks"],
        );
        directory
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
