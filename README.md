# Regressify CLI

Rust CLI for visual regression testing. It launches Chromium through the Chrome DevTools Protocol, captures screenshots from YAML/JSON suites, compares them with reference images, and generates HTML report data.

## Desktop App

An optional Rust-native GPUI Kit workbench lets QA edit workspace configuration,
suites, scenarios, all 15 action types, and viewports. It includes guided and
source editors, inheritance previews, safe saves with undo/redo, CLI execution
with cancellation and logs, and reference/current/diff image review.

```bash
cargo build --features gui --bins
cargo run --features gui --bin regressify-desktop -- /path/to/test-workspace
```

Omit the workspace argument to use the native folder picker. The default build
remains CLI-only. The desktop is self-contained: build it with
`--release --features gui --bin regressify-desktop`. No separate CLI executable
is required; the app runs the shared Rust engine in its own isolated workers.

See [the desktop guide](docs/desktop.md) for workflows, shortcuts, safety limits,
and test commands, and [the delivery plan](docs/desktop-plan.md) for feature scope
and verification evidence.

## Build

```bash
cargo build --release
```

Run the CLI from the repository with:

```bash
cargo run -- --help
```

Install the local build into your Cargo bin directory with:

```bash
cargo install --path .
```

## Validate

```bash
cargo fmt --check
cargo check
cargo test
```

Desktop gates:

```bash
cargo test --features gui-tests --all-targets -- --test-threads=1
cargo clippy --features gui --lib -- -D warnings
```

On an interactive Windows desktop, run [the native smoke test](ci/desktop-smoke.ps1)
to verify real windows, screenshot rendering, and GUI-to-CLI execution.

## Commands

Implemented commands are `init`, `install`, `version`, `ref`, `test`, `compare`, `approve`, `snapshot`, and `optimize`. See [docs/commands.md](docs/commands.md) for command behavior, global flags, output folders, and exit-code semantics.

## Release Script Permissions

```bash
git update-index --chmod=+x ./ci/install-rust.sh
git update-index --chmod=+x ./ci/make-release.sh
```

See also:

- <https://github.com/mattsse/chromiumoxide>
