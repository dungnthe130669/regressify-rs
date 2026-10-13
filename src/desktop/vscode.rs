use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, ensure};

pub(crate) fn open(workspace: &Path, file: Option<&Path>) -> Result<()> {
    ensure!(
        workspace.is_absolute(),
        "VS Code requires an absolute workspace path"
    );
    ensure!(
        workspace.is_dir(),
        "VS Code workspace directory does not exist"
    );
    ensure!(
        file.is_none_or(Path::is_absolute),
        "VS Code requires an absolute file path"
    );
    let search_directories = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    let executable = find_executable(&search_directories, &installation_paths())?;
    launch(command(&executable, workspace, file))
}

fn installation_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            paths.push(PathBuf::from(local).join("Programs/Microsoft VS Code/Code.exe"));
        }
        for name in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(directory) = std::env::var_os(name) {
                paths.push(PathBuf::from(directory).join("Microsoft VS Code/Code.exe"));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let application = "Visual Studio Code.app/Contents/Resources/app/bin/code";
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(home).join("Applications").join(application));
        }
        paths.push(PathBuf::from("/Applications").join(application));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        paths.extend(["/usr/bin/code", "/usr/local/bin/code", "/snap/bin/code"].map(PathBuf::from));
    }
    paths
}

fn find_executable(search_directories: &[PathBuf], installations: &[PathBuf]) -> Result<PathBuf> {
    let mut candidates = Vec::new();
    for directory in search_directories.iter().filter(|path| path.is_absolute()) {
        candidates.push(directory.join(if cfg!(windows) { "Code.exe" } else { "code" }));
        #[cfg(windows)]
        if directory.join("code.cmd").is_file()
            && let Some(parent) = directory.parent()
        {
            candidates.push(parent.join("Code.exe"));
        }
    }
    candidates.extend_from_slice(installations);
    candidates
        .into_iter()
        .find(|path| path.is_absolute() && path.is_file())
        .context("Cannot find VS Code. Install Visual Studio Code or add its bin directory to PATH")
}

fn launch(mut command: Command) -> Result<()> {
    let mut child = command.spawn().context("Cannot launch VS Code")?;
    std::thread::Builder::new()
        .name("vscode-launch".into())
        .spawn(move || {
            let _ = child.wait();
        })
        .context("Cannot monitor VS Code launch")?;
    Ok(())
}

pub(crate) fn command(executable: &Path, workspace: &Path, file: Option<&Path>) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("--new-window")
        .arg("--")
        .arg(editor_path(workspace))
        .env_remove("ELECTRON_RUN_AS_NODE")
        .env_remove("VSCODE_IPC_HOOK_CLI")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(file) = file {
        command.arg(editor_path(file));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    command
}

fn editor_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::path::{Component, Prefix};

        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            let mut normalized = match prefix.kind() {
                Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:", char::from(drive))),
                Prefix::VerbatimUNC(server, share) => {
                    let mut prefix = OsString::from(r"\\");
                    prefix.push(server);
                    prefix.push(r"\");
                    prefix.push(share);
                    PathBuf::from(prefix)
                }
                _ => return path.to_path_buf(),
            };
            normalized.push(components.as_path());
            return normalized;
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn vscode_command_opens_a_new_window_with_a_literal_path() {
        let executable = Path::new("VS Code/Code.exe");
        for target in [
            "workspace with spaces",
            "--reuse-window",
            "workspace & other #100% ! %PATH%",
            r"C:\projects\workspace with spaces\suite.tests.yaml",
        ] {
            let command = command(executable, Path::new(target), None);
            assert_eq!(command.get_program(), executable.as_os_str());
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                [
                    OsStr::new("--new-window"),
                    OsStr::new("--"),
                    OsStr::new(target),
                ]
            );
        }
    }

    #[test]
    fn vscode_command_clears_inherited_editor_launch_overrides() {
        let command = command(Path::new("code"), Path::new("workspace"), None);
        let environment = command.get_envs().collect::<Vec<_>>();
        for name in ["ELECTRON_RUN_AS_NODE", "VSCODE_IPC_HOOK_CLI"] {
            assert!(environment.contains(&(OsStr::new(name), None)));
        }
    }

    #[test]
    fn vscode_executable_prefers_an_absolute_path_entry() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory
            .path()
            .join(if cfg!(windows) { "Code.exe" } else { "code" });
        let installed = directory.path().join("installed-editor");
        std::fs::write(&executable, "").unwrap();
        std::fs::write(&installed, "").unwrap();
        assert_eq!(
            find_executable(&[directory.path().to_path_buf()], &[installed]).unwrap(),
            executable
        );
    }

    #[test]
    fn vscode_executable_falls_back_to_an_installed_file() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing");
        let installed = directory.path().join("installed-editor");
        std::fs::write(&installed, "").unwrap();
        assert_eq!(
            find_executable(
                &[missing.clone()],
                &[missing, directory.path().to_path_buf(), installed.clone()]
            )
            .unwrap(),
            installed
        );
    }

    #[test]
    fn vscode_executable_rejects_relative_entries_and_missing_installations() {
        let directory = tempfile::tempdir().unwrap();
        let relative = PathBuf::from(".");
        let error = find_executable(
            &[relative.clone()],
            &[relative, directory.path().join("missing")],
        )
        .unwrap_err();
        assert!(error.to_string().contains("Install Visual Studio Code"));
    }

    #[cfg(windows)]
    #[test]
    fn vscode_executable_resolves_the_binary_beside_the_windows_cli() {
        let directory = tempfile::tempdir().unwrap();
        let bin = directory.path().join("bin");
        let executable = directory.path().join("Code.exe");
        std::fs::create_dir(&bin).unwrap();
        std::fs::write(&executable, "").unwrap();
        assert!(find_executable(std::slice::from_ref(&bin), &[]).is_err());
        std::fs::write(bin.join("code.cmd"), "").unwrap();
        assert_eq!(find_executable(&[bin], &[]).unwrap(), executable);
    }

    #[test]
    fn vscode_launch_reports_a_missing_executable() {
        let directory = tempfile::tempdir().unwrap();
        let error = launch(command(
            &directory.path().join("missing"),
            directory.path(),
            None,
        ))
        .unwrap_err();
        assert!(format!("{error:#}").contains("Cannot launch VS Code"));
    }

    #[test]
    fn vscode_open_rejects_relative_targets_before_launching() {
        let error = open(Path::new("relative-workspace"), None).unwrap_err();
        assert!(error.to_string().contains("absolute workspace path"));
        let directory = tempfile::tempdir().unwrap();
        let error = open(directory.path(), Some(Path::new("relative-file"))).unwrap_err();
        assert!(error.to_string().contains("absolute file path"));
    }

    #[test]
    fn vscode_open_rejects_a_missing_workspace_before_launching() {
        let directory = tempfile::tempdir().unwrap();
        let error = open(&directory.path().join("missing"), None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("workspace directory does not exist")
        );
    }

    #[test]
    fn vscode_command_keeps_workspace_identity_when_opening_a_file() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory
            .path()
            .join("suite with spaces & #100%.tests.yaml");
        let executable = Path::new("code");
        let folder_command = command(executable, directory.path(), None);
        let file_command = command(executable, directory.path(), Some(&file));
        let mut expected = folder_command
            .get_args()
            .map(OsStr::to_os_string)
            .collect::<Vec<_>>();
        expected.push(editor_path(&file).into_os_string());
        assert_eq!(file_command.get_args().collect::<Vec<_>>(), expected);
        assert_eq!(
            file_command.get_args().nth(2),
            Some(editor_path(directory.path()).as_os_str())
        );
    }

    #[cfg(windows)]
    #[test]
    fn vscode_paths_share_the_same_identity_for_verbatim_and_regular_windows_paths() {
        for (verbatim, regular) in [
            (r"\\?\C:\", r"C:\"),
            (r"\\?\C:\projects\workspace", r"C:\projects\workspace"),
            (r"\\?\UNC\server\share\", r"\\server\share\"),
            (
                r"\\?\UNC\server\share\workspace",
                r"\\server\share\workspace",
            ),
        ] {
            assert_eq!(editor_path(Path::new(verbatim)), Path::new(regular));
            let regular = command(Path::new("Code.exe"), Path::new(regular), None);
            let verbatim = command(Path::new("Code.exe"), Path::new(verbatim), None);
            assert_eq!(
                regular.get_args().collect::<Vec<_>>(),
                verbatim.get_args().collect::<Vec<_>>()
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn vscode_paths_preserve_non_unicode_windows_filenames() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let suffix = [b'C' as u16, b':' as u16, b'\\' as u16, 0xd800];
        let verbatim =
            OsString::from_wide(&r"\\?\".encode_utf16().chain(suffix).collect::<Vec<_>>());
        assert_eq!(
            editor_path(Path::new(&verbatim)),
            PathBuf::from(OsString::from_wide(&suffix))
        );
    }
}
