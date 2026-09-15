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
cargo check --features gui --bin regressify-desktop
```

A successful compile does not replace a native smoke test. Verify keyboard
focus, file dialogs, text input/IME, resizing, and image rendering on the target
platform before shipping. Detailed editing and execution documentation will
accompany the corresponding implementation units.