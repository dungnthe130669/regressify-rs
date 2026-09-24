# Snapshot Frontend

Verified: 2026-09-24

- Suite `displayName` is optional in version-2 summaries/reports. `suiteLabel`
  supplies all visible suite labels while IDs remain route/asset keys. The
  `suite display names` browser test covers desktop/mobile search, titles, Info,
  markup-like text, and stable deep links. Wait for loaded overview rows before
  reading the title; Vite initially serves the HTML title placeholder.
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
- Scenario and generated capture display names come from the shared Rust resolver,
  not frontend string manipulation; see [results naming](desktop-results.md).
  The search/navigation fixture includes colliding URL paths and repeated viewports.
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
- [Review navigation](../../frontend/src/components/ReviewNavigation.tsx) loads only
  the selected suite's scenarios. [Shell](../../frontend/src/App.tsx) owns separate
  left/right scroll areas and pointer-captured divider resizing; preferred desktop
  width survives narrow/mobile viewports. Tailwind resets native `hr` height to zero,
  so the vertical divider explicitly needs `height: 100%`.
- The main/footer wrapper in [Shell](../../frontend/src/App.tsx) must be a growing
  flex column; otherwise `#main` growth and footer auto margins have no effect.
  `bun run test:e2e --grep footer` covers short/filtered/missing pages and long
  document/review scrolling on desktop and mobile.
- [Image viewer](../../frontend/src/components/ComparisonViewer.tsx) has no wheel/pan
  engine. Scaled images reserve their full height in the right page; native wheel
  scrolling leaves the zoom value unchanged. Only explicit controls change scale.
- [Info dialog](../../frontend/src/components/CaptureInfo.tsx) uses Floating UI and
  permits native selection/copy. Pinning must ignore both `hover` and `safe-polygon`
  close reasons; focus opening uses `visibleOnly: false`. Native dialog semantics
  and a non-banner heading avoid duplicate landmark diagnostics in the portal.
- Suite failure arrows navigate only filtered failed cards, retain the suite URL,
  focus/highlight the target, and disable at endpoints or when no failures remain.
- [Shell](../../frontend/src/App.tsx) uses the shared [desktop SVG](../../assets/icon.svg)
  for the logo/favicon; [Vite](../../frontend/vite.config.ts) supplies its content
  version at build time. Suite/image titles prepend the route's decoded suite name;
  the overview restores the report title.
- [Suite list rows](../../frontend/src/pages/SuitePage.tsx) contain complete images
  in equal-width Reference/Test/optional Diff lanes, including on phones. Missing
  images retain their lanes; grid thumbnails and filtered failure jumps are unchanged.
- [Theme startup](../../frontend/index.html) applies saved/system appearance and a
  low-specificity background before React loads. [ThemeControl](../../frontend/src/App.tsx)
  persists only explicit selections and listens for cross-tab changes and resets.
  `bun run test:e2e --grep theme` blocks the app bundle to verify first-paint styling
  and covers missing, invalid, and blocked storage on desktop/mobile.
- Browser image-failure tests intercept the exact URL returned in suite data.
  Development fixtures share preview images instead of using production's
  per-suite image directories; assuming those directories misses the request.
- `bun run test:e2e` tests Vite by default. Set `SNAPSHOT_URL` to the local
  `bun run preview` address to exercise the Rust-generated fixture and hosting headers.
  Source and production browser checks passed on desktop and mobile.
- [Asset workflow](../../.github/workflows/snapshot-assets.yml) builds release source
  read-only, transfers generated assets to a separate master checkout, and opens
  a generated-only PR using `gh`. Updates/deletions use observed-SHA Git leases;
  no-change runs close stale PRs without committing. Offline checks mock both
  commands and feed Bash UTF-8 bytes to avoid Windows stdin newline conversion:
  `python ci/test-release.py -k snapshot_frontend`.
