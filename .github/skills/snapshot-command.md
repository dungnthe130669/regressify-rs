# Skill: `snapshot` Command

## When to Use

Use this skill when changing or reviewing the `regressify snapshot` command or HTML summary report generation.

## Key Files

- `src/cli/args.rs` — `SnapshotArgs`
- `src/snapshot/mod.rs` — snapshot entry point
- `src/snapshot/report.rs` — report data assembly and output
- `src/snapshot/actions.rs`, `src/snapshot/helpers.rs`, `src/snapshot/transform.rs` — snapshot support code
- `src/snapshot/data/` — static report assets and templates
- `docs/commands.md` — user-facing command semantics

## Behavior Contract

- `snapshot` generates the HTML summary from existing comparison data.
- `title`, `heading`, and `subheading` are CLI-configurable.
- Report generation/artifact write failures are process failures.
- Website failures already represented in comparison reports should remain report data, not become command failures.

## Review Checklist

- Does the command read from the active Regressify output directory, including `--site-env` variants?
- Are static assets copied or embedded consistently with the existing template system?
- Does Markdown in `subheading` continue to render as intended?
- Are missing optional suite reports handled gracefully while truly broken report generation still errors?

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test
```

For UI/report changes, generate a snapshot from a fixture-like `.regressify` tree and inspect the generated HTML/config assets carefully.
