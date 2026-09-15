# Regressify Desktop

Regressify Desktop is an optional Rust-native GPUI Kit application. It shares
the CLI's Rust library and keeps YAML/JSON files as the workspace source of truth.
See [the implementation plan](desktop-plan.md) for delivery and verification status.

## Build

Use the stable Rust toolchain and the native build tools for your platform. On
Windows, install the Visual Studio C++ build tools and Windows SDK.

```sh
cargo build --features gui --bins
cargo run --features gui --bin regressify-desktop
```

The `gui` feature is opt-in. `cargo build` and `cargo run -- test` continue to
build/run the CLI without GPUI dependencies. Keep `regressify` alongside the
desktop executable when distributing the application; commands execute in a
separate CLI process so run-specific state is isolated.

## Contributor Checks

```sh
cargo test --all-targets
cargo test --features desktop --lib desktop::
cargo check --features gui --bin regressify-desktop
```

A successful compile does not replace a native smoke test. Verify keyboard
focus, file dialogs, text input/IME, resizing, and image rendering on the target
platform before shipping. Detailed editing and execution documentation will
accompany the corresponding implementation units.

## Document Guarantees

The headless desktop model can be built/tested with `--features desktop` without
compiling GPUI. Guided YAML changes use a concrete syntax tree to retain comments;
JSON edits use pretty-printed JSON. Every guided change is parsed again and
compared to the requested semantic edit. If a YAML construct cannot be handled
losslessly, the operation fails and the original source remains untouched.

Source drafts may be temporarily invalid. Validation is a mandatory save gate.
Undo/redo stores exact source snapshots, limited to 100 entries and a 16 MiB undo
budget. Individual documents are limited to 4 MiB for interactive editing.

Saves compare the file against its last loaded version and refuse to overwrite
external edits. Reload explicitly discards the draft; Save As preserves it in a
new file and never replaces an existing destination. Changing format with Save As
converts the data and does not preserve YAML comments in JSON.

Suite copies drop the suite-level `id` to prevent sharing output artifacts with
the original. Renaming keeps IDs unchanged and requires saved content. File
deletion is restricted to suites and requires a confirmation in the interface.
Recent workspaces and the selected CLI executable are stored in the operating
system's per-user application configuration directory, outside the test project.

## Guided Fields and Inheritance

The field catalog is generated from the existing Rust types with `schemars`.
Collections of scenarios and actions have dedicated editors; generated runtime
`testDate` metadata remains available in Source. A new serialized config or
action property therefore appears in the field catalog without a parallel
handwritten schema.

An absent property inherits from its parent. Explicit `false`, zero, an empty
string, or an empty list is an override, not a request to inherit. Resetting a
field removes the property. Effective configuration uses the CLI resolver with
the scenario, suite, workspace, and built-in defaults in that order. State and
restore names use the existing merge behavior rather than simple replacement.