# Snapshot Frontend

The snapshot site is a static React SPA built with Vite 8, TypeScript, Tailwind CSS,
and Biome. It uses the supplied Roboto and JetBrains Mono fonts locally. No CDN,
backend, BackstopJS bundle, or runtime package manager is needed by published reports.

## Develop

Use the Bun version in [.bun-version](.bun-version). From this directory:

```sh
bun install --frozen-lockfile
bun run dev
```

Development generates synthetic screenshots using an existing Chrome/Chromium/Edge
installation, then starts Vite at `http://127.0.0.1:5173`. Set
`PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` to another Chromium executable, or explicitly
install Playwright's Chromium with `bun --bun playwright install chromium`.
No production pages or deployment credentials are used. Generated fixture data,
screenshots, traces, and dependencies are ignored by Git and Biome.

## Build

```sh
bun run lint
bun run test
bun run build
```

`bun run build` type-checks source, scripts, and tests; builds Vite; and replaces
[../src/snapshot/data](../src/snapshot/data) with the complete generated bundle.
The sync script validates the output in a staging directory before replacing the
previous assets. It removes obsolete assets and generates the Rust include list.
Files are replaced atomically inside the existing directory so Windows editor
watchers do not block a directory rename. Finish the frontend build before Cargo.
Commit the generated directory for ordinary Cargo builds, which do not run Bun.
Do not edit generated files directly. `bun build` is Bun's separate built-in bundler;
the project command is **`bun run build`**.

To preview the actual Rust-generated site, run from the repository root:

```sh
cargo run --bin regressify -- --root frontend/.fixtures/workspace snapshot
```

Then from this directory:

```sh
bun run preview
```

The preview serves the generated fixture at `http://127.0.0.1:4174` with SPA
fallback and production cache/security headers. Another report and port can be
supplied: `bun run preview /path/to/output 4175`. It binds to loopback and exposes
only the entry page and snapshot assets, not capture folders or workspace files.
Run `bun run fixtures` first if development has not created the synthetic workspace.

## Test

```sh
bun run typecheck
bun run lint
bun run test
bun run test:e2e
```

Unit tests cover schemas, safe links, filters, route encoding, and atomic asset
synchronization. Playwright uses desktop and phone viewports for navigation,
deep-link refresh, filtering, theme persistence, all image modes, zoom, downloads,
suite-scoped scenarios, independent pane scrolling, divider drag/keyboard resizing,
failure jumps, selectable/copyable Info content, errors, hostile content,
narrow/wide layouts, and axe accessibility checks.
Screenshots are saved under `.fixtures/screenshots`.

Set `SNAPSHOT_URL=http://127.0.0.1:4174` before `bun run test:e2e` to test the
Rust-generated synthetic report instead of starting the Vite fixture server.
The tests expect the supplied synthetic workspace, not arbitrary user reports.

## Structure

- [src/data](src/data): versioned schemas, report loading, cache, and route helpers.
- [src/pages](src/pages): overview, suite results, and individual image review.
- [src/components](src/components): controls, states, and synchronized image viewer.
- [src/assets/fonts](src/assets/fonts): the two locally bundled source fonts.
- [scripts](scripts): fixture generation, safe preview, and build synchronization.
- [hosting](hosting): Azure/Netlify fallback, security, and cache policies.
- [tests](tests): browser workflows and a synthetic screenshot page.

## Data And Hosting

All resource filenames are stable; cache versions appear only in `?v=...`.
The [Rust publisher](../src/snapshot/site.rs) uses
[get_asset_version](../src/snapshot/helpers.rs) with 20 characters of the SHA-384
base64url digest. The [frontend build](scripts/version-assets.ts) mirrors that
algorithm, with a Rust regression test checking the actual generated references.

The version-2 manifest and every suite/image URL follow the same contract:

```text
_snapshot/assets/app.js?v=<version>
_snapshot/assets/style.css?v=<version>
_snapshot/assets/roboto-combined.woff2?v=<version>
_snapshot/data/manifest.json?v=<version>
_snapshot/data/suites/shop.json?v=<version>
_snapshot/images/shop/reference/home.png?v=<version>
_snapshot/images/shop/test/home.png?v=<version>
_snapshot/images/shop/diff/home_diff.png?v=<version>
```

Suite JSON is loaded on demand. [Frontend schemas](src/data/report.ts) reject
unversioned, unsafe, or noncanonical artifact URLs. Changing an image updates its
version, its suite JSON version, and the manifest version without renaming files.
The build uses one JavaScript bundle to avoid circular chunk-version dependencies;
CSS font references are versioned before the CSS itself. Captured website links
are external source pages, not local report artifacts, and remain unchanged.

Configure any CDN to **include the `v` query parameter in its cache key** and keep
it on origin requests. Generated asset policies use immutable one-year caching.
**The entry HTML must revalidate** so visitors discover new versions; do not cache
`/`, `/index.html`, or SPA route responses indefinitely. Publish the complete output
atomically, as the desktop Azure/Netlify deployers do. Old versions are not retained
at the origin; reload an old report after a deployment. The local preview rejects
stale `v` values, making accidental version/content mismatches visible.

Snapshot generation stages and validates the complete report before publishing,
replaces individual files atomically, and writes HTML last. It removes obsolete
files only inside the generated `_snapshot` directory. Capture history and raw
comparison reports are untouched. Desktop deployment includes only dependencies
of the current manifest and refreshes query versions after image compression.

Deploy at the site's origin root. Routes are `/`, `/suites/:suiteId`, and
`/suites/:suiteId/images/:captureId`. Serve `/index.html` for suite-route requests,
but return real 404 responses for missing `/_snapshot/*` assets. Azure and Netlify
configuration is generated automatically. Opening HTML through `file://` is not
supported. Old `/suite/html_report/index.html` entry pages are removed.

Markdown subheadings allow text, emphasis, lists, code, and safe HTTP(S) links.
Raw HTML and embedded images are not rendered. Page links reject credentials and
non-HTTP(S) protocols. Theme preferences are local; no analytics run.

Suite and image pages show a left navigation pane with Test suites and Test
scenarios. Scenarios belong only to the currently selected suite; scenarios with
several captures expand into individual image links. Both lists are searchable.
The left pane and review area scroll independently. Drag the divider to resize it,
or focus it and use arrow keys, Home, and End. The preferred width is remembered;
phones use a drawer with the same two lists.

Images occupy their full scaled height inside the right page, with no inner
scrollbar or wheel/pan interception. Mouse-wheel, trackpad, and touch scrolling
move the page; zoom buttons, Fit, and Actual size change the rendered image size.
Side-by-side review shows aligned Reference/Test/Diff lanes when a diff exists.
Wipe follows mouse movement without dragging; its slider supports keyboard and touch.

Hover or focus the Info icon for mismatch/threshold statistics, dimensions, capture
and run details, source URLs, and engine errors. The bubble allows native text
selection and copying. Click/tap to pin it; Escape, outside click, or its close
button dismisses it. Metadata stays out of the main image area.

Suite pages have sticky previous/next failed-comparison arrows. They scroll to and
highlight failed cards without changing routes, respect the current search/status
filters, and disable at the first/last visible failure or when none are present.

## Generated-Assets Pull Request

[Snapshot assets workflow](../.github/workflows/snapshot-assets.yml) runs on pushes
to `release` that change `frontend/**`, or by manual dispatch. It installs the
locked dependencies, runs lint and unit tests, then executes `bun run build`.
A separate write-permission job starts from **`master`**, replaces only
`src/snapshot/data`, and creates or updates `automation/snapshot-frontend-assets`.
The PR contains generated assets only; no release-branch source commits are merged.
It is never automatically merged, and a no-change build creates no empty PR.
The job uses the runner's GitHub CLI: `gh pr create` for a new PR and `gh pr edit`
for an existing one. Branch updates use an explicit `--force-with-lease` against
the observed remote commit. If assets already match `master`, the job closes any
stale automation PR and deletes its branch without creating a commit. Lookup,
diff, commit, and push failures stop the job rather than creating a misleading PR.

Enable **Settings > Actions > General > Workflow permissions > Allow GitHub
Actions to create and approve pull requests**. The default `GITHUB_TOKEN` needs
the declared contents/PR permissions. PRs opened with that token do not normally
trigger other Actions workflows; an optional `SNAPSHOT_ASSETS_TOKEN` secret can
provide a suitably scoped GitHub App/PAT token when required checks must trigger.
No token is passed to frontend build steps. Merge the supporting Rust integration
before using generated-only updates with this new report format.
