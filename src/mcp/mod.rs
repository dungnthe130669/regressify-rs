use std::{
    fs,
    io::Write,
    path::{Component, Path},
};

use anyhow::{Context, Result, ensure};

use crate::{cli::args::McpArgs, helpers::is_windows_reserved_path_component};

mod catalog;

pub fn generate(args: McpArgs) -> Result<()> {
    generate_to(&args.output)?;
    println!("Generated MCP Worker in '{}'", args.output.display());
    Ok(())
}

fn generate_to(output: &Path) -> Result<()> {
    let files = package_files()?;
    let name = output
        .file_name()
        .context("MCP output must name a new directory")?;
    let name_text = name
        .to_str()
        .context("MCP output directory name must be UTF-8")?;
    ensure!(
        !name_text.is_empty()
            && !name_text.ends_with(['.', ' '])
            && !name_text
                .chars()
                .any(|character| character.is_control() || "<>:\"|?*\\".contains(character))
            && !is_windows_reserved_path_component(name_text),
        "MCP output must use a portable directory name"
    );
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = parent.canonicalize().with_context(|| {
        format!(
            "MCP output parent '{}' must already exist",
            parent.display()
        )
    })?;
    let destination = parent.join(name);

    // create_dir reserves this destination without replacing even an empty directory or symlink.
    fs::create_dir(&destination).with_context(|| {
        format!(
            "Cannot create MCP output '{}'; the destination must not exist",
            output.display()
        )
    })?;
    for (relative, contents) in files {
        write_package_file(&destination, Path::new(&relative), &contents)?;
    }
    Ok(())
}

fn write_package_file(root: &Path, relative: &Path, contents: &[u8]) -> Result<()> {
    ensure!(
        relative.file_name().is_some()
            && relative
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "Invalid embedded MCP package path"
    );
    let real_directory = |path: &Path| -> Result<()> {
        let metadata = fs::symlink_metadata(path).with_context(|| {
            format!("Cannot inspect MCP package directory '{}'", path.display())
        })?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "MCP package directory '{}' must be a real directory",
            path.display()
        );
        Ok(())
    };
    real_directory(root)?;
    let mut parent = root.to_path_buf();
    for component in relative
        .parent()
        .context("MCP package file needs a parent")?
        .components()
    {
        parent.push(component.as_os_str());
        match fs::create_dir(&parent) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Cannot create MCP package directory '{}'", parent.display())
                });
            }
        }
        real_directory(&parent)?;
    }
    let path = root.join(relative);
    (|| -> Result<()> {
        let mut temporary = tempfile::NamedTempFile::new_in(&parent)?;
        temporary.write_all(contents)?;
        temporary.as_file().sync_all()?;
        temporary.persist_noclobber(&path)?;
        Ok(())
    })().with_context(|| {
        format!("Cannot write MCP package file '{}'; choose a new output directory after a failed export", path.display())
    })
}

fn package_files() -> Result<Vec<(String, Vec<u8>)>> {
    let catalog = catalog::build()?;
    let mut files: Vec<_> = [
        ("package.json", include_str!("worker/package.json")),
        (
            "package-lock.json",
            include_str!("worker/package-lock.json"),
        ),
        (
            "wrangler.jsonc",
            include_str!("worker/wrangler.jsonc.template"),
        ),
        (
            "tsconfig.json",
            include_str!("worker/tsconfig.json.template"),
        ),
        (".gitignore", include_str!("worker/.gitignore")),
        ("src/index.ts", include_str!("worker/src/index.ts.template")),
        (
            "tests/worker.test.mjs",
            include_str!("worker/tests/worker.test.mjs.template"),
        ),
        ("README.md", include_str!("worker/README.md")),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()))
    .collect();
    for (kind, schema) in catalog["schemas"]
        .as_object()
        .context("Missing MCP schemas")?
    {
        files.push((
            format!("content/schemas/{kind}.json"),
            serde_json::to_vec_pretty(schema)?,
        ));
    }
    for (topic, guide) in catalog["guides"]
        .as_object()
        .context("Missing MCP guides")?
    {
        files.push((
            format!("content/guides/{topic}.md"),
            guide["content"]
                .as_str()
                .context("Missing MCP guide content")?
                .as_bytes()
                .to_vec(),
        ));
    }
    for (name, example) in catalog["examples"]
        .as_object()
        .context("Missing MCP examples")?
    {
        let extension = match example["format"].as_str() {
            Some("javascript") => "js",
            Some(format) => format,
            None => anyhow::bail!("Missing MCP example format"),
        };
        files.push((
            format!("content/examples/{name}.{extension}"),
            example["content"]
                .as_str()
                .context("Missing MCP example content")?
                .as_bytes()
                .to_vec(),
        ));
    }
    files.push((
        "content/catalog.json".into(),
        serde_json::to_vec_pretty(&catalog)?,
    ));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_includes_a_complete_worker_and_matching_dependency_lock() {
        let files: std::collections::BTreeMap<_, _> =
            package_files().unwrap().into_iter().collect();
        for path in [
            "src/index.ts",
            "wrangler.jsonc",
            "tsconfig.json",
            "README.md",
            "tests/worker.test.mjs",
            "content/catalog.json",
        ] {
            assert!(files.contains_key(path), "{path}");
        }
        let package: serde_json::Value = serde_json::from_slice(&files["package.json"]).unwrap();
        let lock: serde_json::Value = serde_json::from_slice(&files["package-lock.json"]).unwrap();
        let wrangler: serde_json::Value = serde_json::from_slice(&files["wrangler.jsonc"]).unwrap();
        // Disabling uploads also stops Wrangler from generating source maps.
        assert_eq!(wrangler["minify"], true);
        assert_eq!(wrangler["upload_source_maps"], false);
        assert_eq!(
            package["dependencies"], lock["packages"][""]["dependencies"],
            "Worker dependencies must match the lockfile root"
        );
        assert_eq!(
            package["devDependencies"], lock["packages"][""]["devDependencies"],
            "Worker devDependencies must match the lockfile root"
        );
        for section in ["dependencies", "devDependencies"] {
            for (name, version) in package[section].as_object().unwrap() {
                assert!(
                    version
                        .as_str()
                        .unwrap()
                        .chars()
                        .all(|character| character.is_ascii_digit() || character == '.'),
                    "{section}.{name} must use an exact version without ranges; found {version}"
                );
                assert_eq!(
                    lock["packages"][format!("node_modules/{name}")]["version"],
                    *version,
                    "Resolved lockfile version must match the exact pin for {section}.{name}"
                );
            }
        }
    }

    #[test]
    fn deployment_scripts_gate_publication_and_export_checked_source() {
        let root: serde_json::Value =
            serde_json::from_str(include_str!("../../package.json")).unwrap();
        assert_eq!(
            root["scripts"]["deploy:check"],
            "cargo fmt --all -- --check && cargo test --locked --lib mcp && cargo build --locked --bin regressify"
        );
        assert_eq!(
            root["scripts"]["deploy"],
            "bun run deploy:check && tuyen rm -rf ../regressify-mcp-worker && cargo run --locked --bin regressify -- mcp --output ../regressify-mcp-worker && bun ci --cwd ../regressify-mcp-worker && bun run --cwd ../regressify-mcp-worker deploy"
        );
        let files: std::collections::BTreeMap<_, _> =
            package_files().unwrap().into_iter().collect();
        let package: serde_json::Value = serde_json::from_slice(&files["package.json"]).unwrap();
        assert_eq!(
            package["scripts"]["verify"],
            "npm audit --audit-level=high && npm run check && npm test && npm run build"
        );
        assert_eq!(
            package["scripts"]["deploy"],
            "npm run verify && wrangler deploy"
        );
    }

    #[test]
    fn export_contains_versioned_self_contained_schemas() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("worker");
        generate_to(&output).unwrap();
        let catalog: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("content/catalog.json")).unwrap())
                .unwrap();
        assert_eq!(catalog["regressifyVersion"], env!("CARGO_PKG_VERSION"));
        for kind in catalog["schemaKinds"].as_array().unwrap() {
            let path = output.join(format!("content/schemas/{}.json", kind.as_str().unwrap()));
            let schema: serde_json::Value =
                serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            assert!(schema["$schema"].is_string());
        }
    }

    #[test]
    fn export_is_deterministic_and_contains_no_workspace_data() {
        assert_eq!(package_files().unwrap(), package_files().unwrap());
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("regressify.yaml"), "not: [valid").unwrap();
        fs::write(
            root.path().join("private-state.json"),
            "private-fixture-marker",
        )
        .unwrap();
        generate_to(&root.path().join("worker")).unwrap();
        for (_, bytes) in package_files().unwrap() {
            assert!(
                !String::from_utf8(bytes)
                    .unwrap()
                    .contains("private-fixture-marker")
            );
        }
    }

    #[test]
    fn export_never_replaces_existing_destinations() {
        let root = tempfile::tempdir().unwrap();
        for name in ["empty", "edited"] {
            let output = root.path().join(name);
            fs::create_dir(&output).unwrap();
            if name == "edited" {
                fs::write(output.join("keep.txt"), "user edit").unwrap();
            }
            assert!(generate_to(&output).is_err());
            assert_eq!(
                fs::read_dir(&output).unwrap().count(),
                usize::from(name == "edited")
            );
        }
        let output = root.path().join("file");
        fs::write(&output, "user edit").unwrap();
        assert!(generate_to(&output).is_err());
        assert_eq!(fs::read_to_string(output).unwrap(), "user edit");
    }

    #[test]
    fn package_writes_preserve_unexpected_files_and_reject_unsafe_paths() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("content"), "keep directory blocker").unwrap();
        fs::write(root.path().join("existing.json"), "keep existing file").unwrap();
        for path in [
            "content/catalog.json",
            "existing.json",
            "../outside.json",
            "/absolute.json",
            "",
        ] {
            assert!(
                write_package_file(root.path(), Path::new(path), b"new").is_err(),
                "{path}"
            );
        }
        assert_eq!(
            fs::read_to_string(root.path().join("content")).unwrap(),
            "keep directory blocker"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("existing.json")).unwrap(),
            "keep existing file"
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn export_rejects_invalid_destinations_without_creating_parents() {
        let root = tempfile::tempdir().unwrap();
        for name in [
            "CON",
            "NUL.txt",
            "trailing.",
            "trailing ",
            "invalid:name",
            "missing/worker",
        ] {
            assert!(generate_to(&root.path().join(name)).is_err(), "{name}");
        }
        assert!(generate_to(Path::new("")).is_err());
        assert!(generate_to(Path::new(".")).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn concurrent_exports_have_one_winner() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("worker");
        let barrier = std::sync::Barrier::new(2);
        let successes = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        generate_to(&output).is_ok()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| usize::from(handle.join().unwrap()))
                .sum::<usize>()
        });
        assert_eq!(successes, 1);
    }

    #[cfg(unix)]
    #[test]
    fn export_rejects_a_symlink_destination() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let output = root.path().join("worker");
        std::os::unix::fs::symlink(outside.path(), &output).unwrap();
        assert!(generate_to(&output).is_err());
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);

        let destination = root.path().join("new-worker");
        fs::create_dir(&destination).unwrap();
        std::os::unix::fs::symlink(outside.path(), destination.join("content")).unwrap();
        assert!(
            write_package_file(&destination, Path::new("content/catalog.json"), b"new").is_err()
        );
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    }
}
