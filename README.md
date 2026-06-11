# Regressify CLI

Rust CLI for visual regression testing. It launches Chromium through the Chrome DevTools Protocol, captures screenshots from YAML/JSON suites, compares them with reference images, and generates HTML report data.

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

## Commands

Implemented commands are `init`, `install`, `version`, `ref`, `test`, `compare`, `approve`, `snapshot`, and `optimize`. See [docs/commands.md](docs/commands.md) for command behavior, global flags, output folders, and exit-code semantics.

## Release Script Permissions

```bash
git update-index --chmod=+x ./ci/install-rust.sh
git update-index --chmod=+x ./ci/make-release.sh
```

See also:

- <https://github.com/mattsse/chromiumoxide>
