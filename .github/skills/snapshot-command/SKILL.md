---
name: snapshot-command
description: 'Use when changing regressify snapshot, the frontend React SPA, report schemas, suite filters, cache versions, hosting, or Markdown headings.'
---

# Snapshot Reports

## Start Here

[Snapshot selection](../../../src/snapshot/actions.rs), [comparison conversion](../../../src/snapshot/capture_report.rs), [publication](../../../src/snapshot/site.rs), and [frontend guide](../../../frontend/README.md).

## Procedure and Invariants

1. Read existing comparison data from the active output directory, including environment/custom-output overrides. Preserve suite filters and explicit misses.
1. Resolve report text per field: explicit `--title`/`--heading`/`--subheading`, then workspace `snapshotTitle`/`snapshotHeading`/`snapshotSubheading`, then built-in defaults. Preserve explicit empty strings and CLI values equal to the defaults. Automatic `test`/`ref` reports must leave CLI text options unset.
1. Desktop Snapshot/Deploy must also leave unchosen report text unset in `RunOptions` and omit its CLI flags. Verify both the argument builder and bundled-worker staging; CLI-only tests do not cover desktop-generated overrides.
1. Preserve safe Markdown subheadings and plain-text title/heading escaping. Raw HTML and embedded Markdown images are excluded; external page links allow only uncredentialed HTTP(S).
1. Keep version-2 Rust report types and [frontend schemas](../../../frontend/src/data/report.ts) synchronized. Read the run selected by `config.js`, verify `report.json` identity/inventory, and never execute report JavaScript or delete prior captures.
1. Every script, CSS, font, manifest, suite JSON, and image URL uses a stable filename plus `?v=` from `get_asset_version(bytes, 20)`. Frontend build uses matching SHA-384/base64url encoding; verify generated references against Rust, including font-to-CSS version propagation.
1. Stage and validate all selected suites before publishing stable paths. Replace files atomically, HTML last, then prune only stale generated `_snapshot` files. Validate decoded canonical path components, symlink boundaries, and query/content versions at publication/staging boundaries. Never join URLs including queries to filesystem paths.
1. Develop source in `frontend`; `bun run build` replaces `src/snapshot/data` and generates `assets.rs`. Do not hand-edit generated files. Keep capture-owned error artwork in `src/test/data`, outside this replaced directory.
1. Routes use origin-root clean URLs with Azure/Netlify fallback. HTML revalidates; versioned resource URLs cache immutably. CDN keys must include `v`. Exclude `/_snapshot/*` from SPA fallback; do not promise historical origin versions after deployment.
1. Keep deployment inventory and post-compression query versions synchronized without renaming files. Never package the entire output directory or include raw captures, state, or stale assets.
1. The CLI optimizer skips `_snapshot`; optimize source captures before generating a new snapshot. Never mutate published bytes without updating their URL hash.
1. Existing visual failures remain report data. Malformed input or generated-file write failures are process errors; do not broadly swallow optional-file errors.

## Checks

```sh
cargo test --lib snapshot::
cargo test --lib config::tests::snapshot_report_text_loads_from_workspace_yaml
cargo test --features desktop --lib snapshot::site::tests
```

In `frontend`, run `bun run lint`, `bun run test`, `bun run build`, and `bun run test:e2e`. Browser tests require an existing Chromium executable or an explicit Playwright install. Use synthetic fixtures; never production credentials. Generate with `cargo run --bin regressify -- --root frontend/.fixtures/workspace snapshot`, then `bun run preview` and `SNAPSHOT_URL` for production-bundle browser checks. Cover empty data, filters, hostile labels/URLs, repeated generation, write errors, deep-link refresh, mobile navigation, and all image modes. Keep `.fixtures` out of Biome. Verify Reference/Test/Diff lane alignment, pointer-controlled wipe after zoom/pan, readable review text, and compact secondary metadata on desktop and mobile. Test real mouse wheel input, not only zoom buttons. The zoom library's smooth mode multiplies the wheel step by raw delta pixels; disable that mode for bounded relative steps and cover normal, large, and trackpad-sized deltas plus fractional scales.

## Maintain

Update [command docs](../../../docs/commands.md), [frontend guide](../../../frontend/README.md), deployment consumers, and this skill. Frontend pushes to `release` use the [asset workflow](../../workflows/snapshot-assets.yml) to open a generated-only PR to `master`.
