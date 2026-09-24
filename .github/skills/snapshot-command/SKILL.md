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
1. Supply scenario names and generated capture labels through the [shared Rust resolver](../../../src/helpers/scenario_names.rs), not a second SPA algorithm. Prefer captured `scenarioDisplayName`; derive legacy names from original URLs across logical scenarios in the report. Keep explicit labels, capture IDs, filenames, source URLs, and routes independent of generated presentation names.
1. Every script, CSS, font, manifest, suite JSON, and image URL uses a stable filename plus `?v=` from `get_asset_version(bytes, 20)`. Frontend build uses matching SHA-384/base64url encoding; verify generated references against Rust, including font-to-CSS version propagation.
1. Stage and validate all selected suites before publishing stable paths. Replace files atomically, HTML last, then prune only stale generated `_snapshot` files. Validate decoded canonical path components, symlink boundaries, and query/content versions at publication/staging boundaries. Never join URLs including queries to filesystem paths.
1. Develop source in `frontend`; `bun run build` replaces `src/snapshot/data` and generates `assets.rs`. Do not hand-edit generated files. Keep capture-owned error artwork in `src/test/data`, outside this replaced directory.
1. Routes use origin-root clean URLs with Azure/Netlify fallback. HTML revalidates; versioned resource URLs cache immutably. CDN keys must include `v`. Exclude `/_snapshot/*` from SPA fallback; do not promise historical origin versions after deployment.
1. Keep Netlify's global headers aligned with Azure's `globalHeaders`, including referrer and MIME-sniffing protection. Edit hosting sources, regenerate embedded assets, and check required cache directives independent of ordering or extra directives.
1. Keep deployment inventory and post-compression query versions synchronized without renaming files. Never package the entire output directory or include raw captures, state, or stale assets.
1. The CLI optimizer skips `_snapshot`; optimize source captures before generating a new snapshot. Never mutate published bytes without updating their URL hash.
1. Existing visual failures remain report data. Malformed input or generated-file write failures are process errors; do not broadly swallow optional-file errors.
1. Review navigation shows all suites but only the selected suite's scenarios. Keep capture groups, active links, search, loading/error states, and mobile drawer content scoped to that suite. Sidebar and review scroll independently; clamp divider width without losing the preferred desktop size on mobile.
1. Images use native right-pane page scrolling, not wheel zoom or inner pan/scroll handlers. Explicit zoom buttons resize their document-flow footprint. Keep stats and metadata in a hover/focus/pinnable Info dialog with selectable text; failure arrows scroll within the filtered suite without changing routes.
1. Reuse the desktop [icon source](../../../assets/icon.svg) for the logo and favicon. Bundle it once and content-version both URLs. Suite/image browser titles include the suite name and configured report title; overview navigation restores the report title.
1. Suite list rows show equal-width Reference/Test lanes and a Diff lane only when present, including on phones. Contain complete previews without cropping, retain missing-image lanes, and preserve filters/view selection when opening captures. Grid view keeps one thumbnail.
1. Apply theme preferences synchronously in the document head before the app bundle. Keep the critical background palette aligned with the main stylesheet and lower in specificity. Storage is optional; persist only explicit selections, synchronize other tabs, and follow OS appearance only in System mode.

## Checks

```sh
cargo test --lib snapshot::
cargo test --lib helpers::scenario_names::tests
cargo test --features desktop --lib snapshot::capture_report::tests
cargo test --lib config::tests::snapshot_report_text_loads_from_workspace_yaml
cargo test --features desktop --lib snapshot::site::tests
```

In `frontend`, run `bun run lint`, `bun run test`, `bun run build`, and `bun run test:e2e`. Browser tests require an existing Chromium executable or an explicit Playwright install. Use synthetic fixtures; never production credentials. Generate with `cargo run --bin regressify -- --root frontend/.fixtures/workspace snapshot`, then `bun run preview` and `SNAPSHOT_URL` for production-bundle browser checks. Cover empty data, filters, hostile labels/URLs, repeated generation, write errors, deep-link refresh, mobile navigation, and all image modes. Keep `.fixtures` out of Biome. Verify lane alignment, mouse-controlled wipe, native page scrolling without scale changes, independent panes, divider mouse/keyboard bounds and persistence, suite-scoped scenarios, failure-arrow endpoints, and Info hover/pinning/selection/clipboard/Escape behavior. Check logo/favicon bytes against the shared SVG, title updates across navigation/refresh, and list lanes with absent diffs, missing images, and long labels.

For theme changes, run `bun run test:e2e --grep theme` against development and generated reports. Block the app bundle to verify the initial theme/background, then check saved/system precedence, invalid or inaccessible storage, cross-tab changes, and resets without automatic preference writes.

## Maintain

Update [command docs](../../../docs/commands.md), [frontend guide](../../../frontend/README.md), deployment consumers, and this skill. Frontend pushes to `release` use the [asset workflow](../../workflows/snapshot-assets.yml) to open a generated-only PR to `master`.
