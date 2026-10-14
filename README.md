# Regressify CLI

Rust CLI for visual regression testing. It launches Chromium through the Chrome DevTools Protocol, captures screenshots from YAML/JSON suites, compares them with reference images, and generates HTML report data.

## Desktop App

An optional Rust-native GPUI Kit workbench lets QA edit workspace configuration,
suites, scenarios, all action types, and viewports. It includes guided and
source editors, inheritance previews, safe saves with undo/redo, a bundled engine,
and reference/current/diff image review. One workspace is active at a time.
Select multiple suites with checkboxes and track their scenarios live through
queued, running, captured, comparing, passed, failed, and cancelled states.

```bash
cargo build --features gui --bin regressify-desktop
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

Implemented commands are `init`, `install`, `mcp`, `version`, `ref`, `test`, `compare`, `approve`, `snapshot`, and `optimize`. See [docs/commands.md](docs/commands.md) for command behavior, global flags, output folders, and exit-code semantics.

## MCP Authoring Guidance

Generate a standalone Cloudflare Worker from the installed CLI:

```sh
regressify mcp --output ./regressify-mcp-worker
cd regressify-mcp-worker
npm ci
npm run check
npm test
npm run build
```

The destination must be new and its parent must exist. Generation needs no source
checkout, browser, Node installation, Cloudflare credentials, or workspace data.
The generated project includes versioned guides, schemas, synthetic examples,
the MCP handler, and a pinned npm lockfile. Local Worker checks require Node 22.12
or newer; `build` is a dry run. Review the generated Worker name and README before
separately running `npm run deploy` with your own Cloudflare authentication.

AI clients connect to the deployed `/mcp` endpoint to retrieve instructions for
suites, scenarios, actions, workspace configuration, predefined files, lifecycle
hooks, and troubleshooting. The hooks topic includes tested Node examples; the
troubleshooting topic explains local reports/logs and focused diagnostic commands. The
default Worker is public and read-only: it cannot read local files, run captures,
accept private logs, or approve references. See [the command guide](docs/commands.md#mcp-authoring-worker).

After creating or editing a suite, agents must run
`regressify ref --test-suite <suite-name>` with the CLI installed alongside desktop
on the user's `PATH`, then verify all intended screenshots in the fresh run.
This updates successful references, so target-page and baseline-write permissions
still apply; blocked verification must be reported as incomplete.

For components that expand between scenarios, the MCP guidance recommends the
same larger capture area for default and expanded states: viewport or full page.
A header-only default screenshot can miss a menu that is already open before a
click. Optional component details supplement, rather than replace, these images.

The MCP guidance and generated `AGENTS.md` direct agents to reuse and record
site-specific lessons in the consumer project's `.ai/memory/`: one behavior or
issue on one site per file, at most 60 lines, with a compact linked index. Notes
retain verified fixes, failed approaches, and evidence without secrets or raw logs.
The server never reads or stores this project memory.

## Snapshot Frontend

The unified snapshot SPA has overview, suite, and image URLs, light/dark themes,
responsive suite navigation, and zoomable side-by-side, wipe, overlay, and diff views.
Develop it in [frontend](frontend/README.md). `bun run build` there regenerates the
embedded assets in `src/snapshot/data`; Cargo builds need no JavaScript tooling.
Frontend changes pushed to `release` create a generated-assets-only PR to `master`
through the [snapshot assets workflow](.github/workflows/snapshot-assets.yml).

## Release Script Permissions

Release definitions are available for [GitHub Actions](.github/workflows/tuyen-release.yml)
and [Azure Pipelines](.ado/workflows/tuyen-release.yaml). Desktop selections also
build the CLI in parallel. The Windows installer installs both executables in
the same folder and adds that folder to the current user's PATH. See the
[release guide](docs/desktop.md#release-pipeline) for setup, Library variables,
supported targets, and publishing safeguards.

```bash
git update-index --chmod=+x ./ci/install-rust.sh
git update-index --chmod=+x ./ci/make-release.sh
```

See also:

- <https://github.com/mattsse/chromiumoxide>
