# Desktop Publishing

Verified: 2026-10-05

- [Landing apps](../../src/desktop/landing.rs) are global per-user definitions,
  automatically stored by UUID. The [editor](../../src/desktop/ui/landing.rs)
  has no file picker or Save As. Linked URLs need no local workspace and are
  fetched only by the visitor's browser.
- `pulldown-cmark` renders both subheadings into static HTML. Shared frontend
  assets provide logo, theme, and client status; raw HTML/images and unsafe or
  credentialed links are excluded. The preview exposes only generated content.
- [Named tokens](../../src/desktop/deployment/tokens.rs) keep only metadata in
  JSON. Keyring identities use `named:<uuid>`, separate from direct target tokens.
  Provider/account identity is immutable, and target deletion preserves shared
  tokens. The catalog uses a file lock and stale-editor comparison.
- [Target settings](../../src/desktop/deployment.rs) retain optional `publishedUrl`
  per ID for reports and landing apps. Only successful publication updates it;
  destination edits (including Azure token replacement) clear it. The runner saves
  URLs with completed activity; landing completion saves before notifying the UI.
  Target/runner regressions and `saved_deployment_urls_reopen_and_copy` cover reload,
  provider isolation, unsafe links, failure preservation, and native copy controls.
- Report publications also retain suite output IDs in completion order. Runs links
  resolve the newest site containing the requested suites, encode single-suite
  paths, and reload asynchronously through artifact refresh. A subset redeployment
  replaces the site's membership; legacy URLs do not imply membership. The
  `published_report_sites_` and `runs_open_site_` regressions cover these contracts.
- The dialog's native link uses content-sized flex layout so Copy stays beside it;
  the toolkit's label wrapper centers text inside a stretched button. Deployment
  activity reuses the Log tab's read-only editor, fills available height, and keeps
  retained lines intact. Native tests cover full-height bounds and exact copying.
- Cloudflare publication may succeed without a public URL. Nullable progress and
  saved results retain compatibility with existing strings; confirmed private
  results clear stale URLs. Asset-option edits preserve URLs, routing edits clear
  them. The native target test covers production defaults and advanced settings.
- [Shared analytics](../../src/analytics.rs) emits `regressify_site_deployed` from
  provider-confirmed public completion. Stats reuse final Azure package and
  Netlify/Cloudflare manifest counts and bytes, without rescanning files. The
  digest fixture counts three paths/15 bytes although only one blob uploads;
  the cached Cloudflare fixture excludes hosting-policy files. Analytics tests
  cover silent failures and client reuse across concurrent calls and new runtimes.
  HTTP timeout cleanup must stay inside the silent tracing scope because request
  futures log on drop; see the [analytics skill](../../.github/skills/product-analytics/SKILL.md).
- [Status summaries](../../src/snapshot/site.rs) use `/_snapshot/status.json`
  with aggregate counts and generation time. They revalidate and permit anonymous
  CORS reads; exact ZIP/Netlify inventories include this extra file.
- [Publishing smoke](../../ci/desktop-smoke/flows/publishing.ps1) passes with
  automatic save, remote links, Markdown, draft guards, token reuse, target
  removal, and light/dark compact/large windows. Catalogs are isolated/restored.
- Keep selector widths bounded: long token names otherwise push toolbar actions
  beyond the dialog. Landing forms use a retained explicit scroll handle, and
  offscreen inputs may be absent from native accessibility until scrolling.
- Native password clicks require the existing post-click `Wait-Control` settling
  before typing. Menu selection also needs a rendered-state wait before checking
  hidden fields. GPUI scroll tests must settle dialog animations before geometry.
- GPUI `label()` returns an explicit accessibility name, not overridden visible
  button text. Assert selected routing in model state separately. Conditional
  scrollbar branches need type erasure because the wrapper changes element type.
- Six production landing browser checks pass on desktop/mobile, including
  no-JavaScript content, anonymous remote fetches, errors/retry, and accessibility.
  Live Azure/Cloudflare publication and Azure-to-Cloudflare status loading passed.
  The authorized Netlify check was blocked by provider account credits, not an
  upload-client failure; do not automatically retry billing restrictions.
- Use `bun run --cwd frontend biome ...` for the installed formatter. Putting
  `--cwd frontend` after `bun x` can interpret `frontend` as a package name. Scope
  formatting to changed files; this checkout has unrelated CRLF findings.
