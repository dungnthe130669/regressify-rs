# Skill: `ref` Command

## When to Use

Use this skill when changing or reviewing the `regressify ref` command.

## Key Files

- `src/cli/args.rs` — `RefArgs`
- `src/test/test_suites.rs` — `exec_ref`, suite discovery, capture job orchestration
- `src/test/test_suite.rs` — parsing and per-scenario browser launch wrapper
- `src/test/test_scenario.rs` — scenario capture and error-placeholder screenshots
- `src/test/capture.rs` — screenshot names, manifests, state paths, error markers
- `src/compare/report_gen.rs` — comparison report generation after baseline capture
- `docs/commands.md` — user-facing command semantics

## Behavior Contract

- `ref` captures reference baselines from raw scenario URLs.
- `--replacement-profile` is accepted by the CLI but ignored for `ref`; scenario URLs and action `wait.url` values must not use test-environment URL replacements.
- YAML/config validation must happen before browser fetch or reference mutation.
- Successful captures are promoted into `bitmaps_reference`.
- Failed capture placeholders must not be promoted.
- Existing good references should not be deleted before a successful replacement exists.
- Page/action failures are report results and must not produce a non-zero exit.
- Browser fetch/launch failures, invalid config, and artifact write failures are process failures.

## Review Checklist

- Does `exec_ref` parse and validate all selected suites before launching browsers?
- Does it build viewport jobs with `scenario_capture_viewports(...)?` so unknown viewports fail early?
- Does it preserve existing references when a scenario fails?
- Does it skip `.error.json` sidecar screenshots during promotion?
- Does it still generate snapshot/report data after capture?

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test
```

For behavior changes, use a small suite with one passing scenario and one intentionally failing action, then confirm `ref` exits zero and does not promote the failed placeholder successfully.
