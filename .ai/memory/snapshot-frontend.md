# Snapshot Frontend

Verified: 2026-09-23

- The [SPA](../../frontend/README.md) uses root-based clean URLs and lazy suite JSON.
  `bun run build` replaces `src/snapshot/data` and generates the Rust include list;
  Cargo does not execute JavaScript tools. All runtime assets, fonts, JSON, and
  image URLs use stable filenames plus `?v=`; only HTML revalidates. Versions use
  [get_asset_version](../../src/snapshot/helpers.rs) with 20 characters. Frontend
  [versioning](../../frontend/scripts/version-assets.ts) mirrors SHA-384/base64url;
  Rust tests verify the built references. CDN keys must include the query parameter.
- [Schema](../../frontend/src/data/report.ts) and [Rust types](../../src/snapshot/site.rs)
  use schema version 2. Markdown is rendered through React with raw HTML/images
  omitted and HTTP(S)-only page links. Report JS is parsed as a JSON wrapper, never run.
- [Synthetic fixtures](../../frontend/scripts/create-fixture.ts) use local Chromium,
  not production sites. Biome must exclude `.fixtures`: formatting `config.js`
  removes JSON key quotes and intentionally fails Rust's strict report parser.
- Biome 2.5 uses `rules.preset: recommended`. Verify the migrated value; a migration
  can select `none`, making a nominally clean run stop checking source rules.
- Windows editor watchers can prevent renaming `src/snapshot/data`. The sync script
  stages the build, atomically replaces individual files, and removes stale files
  afterward; its tests assert the destination directory identity remains unchanged.
- Snapshot generation validates all suites in a temporary directory before
  replacing stable paths; an invalid later suite preserves the old report. Repeated
  generation changes image/suite/manifest versions without file-list churn and
  removes obsolete files only inside generated `_snapshot` directories.
- Editor symbol rename may skip desktop-gated references when rust-analyzer uses
  default features. Always run the desktop-feature check after a shared rename.
- `react-zoom-pan-pinch` smooth wheel mode multiplies `wheel.step` by raw delta.
  A 120-pixel notch with step 0.15 jumps to a bound. The viewer uses `smooth=false`
  with steps proportional to current scale; real-wheel browser tests reproduce
  the old 100%-to-800% jump and verify bounded steps in both directions.
- `bun run test:e2e` tests Vite by default. Set `SNAPSHOT_URL` to the local
  `bun run preview` address to exercise the Rust-generated fixture and CSP too.
  Source and production browser checks passed on desktop and mobile.
- [Asset workflow](../../.github/workflows/snapshot-assets.yml) builds release source
  read-only, transfers generated assets to a separate master checkout, and opens
  a generated-only PR using `gh`. Updates/deletions use observed-SHA Git leases;
  no-change runs close stale PRs without committing. Offline checks mock both
  commands and feed Bash UTF-8 bytes to avoid Windows stdin newline conversion:
  `python ci/test-release.py -k snapshot_frontend`.
