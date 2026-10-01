---
name: deployment-packaging
description: 'Use when changing deployment staging, report asset collection, image compression, ZIP creation, digest manifests, provider size/file limits, or artifact path validation.'
---

# Deployment Packaging

## Start Here

[Staging/artifact selection](../../../src/desktop/deployment.rs), [Azure ZIP package](../../../src/desktop/deployment/package.rs), [Netlify manifests](../../../src/desktop/deployment/netlify.rs), [Cloudflare manifests](../../../src/desktop/deployment/cloudflare.rs), and [snapshot generation](../../../src/snapshot/site.rs). Azure and Cloudflare share [image optimization](../../../src/desktop/deployment/optimization.rs).

## Procedure and Invariants

1. Regenerate the selected snapshot first. Stage only the active manifest, its suite JSON and versioned image dependencies (including optional `previews`), embedded frontend files, and hosting policies. Require the manifest's suite set to match selection; never walk capture folders or choose the newest run independently.
1. Verify data/image `?v=` values with `get_asset_version(bytes, 20)` before and after staging. Decode canonical URL paths and strip queries before filesystem access; reject traversal, duplicate versions, or missing versions. `FRONTEND_FILES` comes from the Vite build. Raw reports, captures, stale generated files, and state must not enter ZIPs or Netlify manifests.
1. Reject symlink escapes and non-regular files. Preserve nested paths in archives/manifests and require the entry report; never include arbitrary workspace files, persisted browser state, credentials, or caches.
1. Preserve separate source/staging copies. Azure uses 250 MiB for Free/unknown and 500 MiB for Standard; Cloudflare uses 2 GiB of selected public assets. Invoke the shared optimizer only above the provider budget. It must not rewrite reference/test captures or change the CLI optimizer's lossless contract. Netlify remains unchanged.
1. Classify images from suite data, taking the most protective role for shared paths. Keep diffs/errors and generated previews lossless. After compression, refresh all original/preview image query versions, then suite JSON and manifest versions; never rename image files. Revalidate final storage size and preserve source captures.
1. Enforce provider file limits and compressed/uncompressed byte bounds. Detect content changing during copy and clean staging/temporary packages on failure.
1. Keep deterministic ordering and cancellation/progress updates during packaging/hashing. Azure uploads a ZIP; Netlify uploads requested content from a full SHA-1 manifest. Neither a completed ZIP nor a submitted manifest proves publication succeeded.
1. Cloudflare uses content/extension/MIME hashes and raw single-asset or Base64 multipart uploads. Validate and select assets before optimization, deferring only the total-byte limit. Enforce 25 MiB per file and exclude Azure configuration and hosting-policy files from the public manifest. Rebuild hashes after optimization and retain the temporary copy through upload/verification. Read hosting policies from the validated original staging root and pass them as metadata; translate index-page proxies to `/` only when HTML normalization is enabled. Retain the private-file and exclusion checks in the Cloudflare skill.
1. Include the validated aggregate `/_snapshot/status.json` in report inventory. Landing staging instead contains only pre-rendered HTML and embedded public frontend/hosting assets; never package its local definition, catalog, target settings, or credentials.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::package::tests
cargo test --features desktop --lib desktop::deployment::tests
cargo test --features desktop --lib desktop::deployment::netlify::tests
cargo test --features desktop --lib desktop::deployment::cloudflare::tests
cargo test --features desktop --lib desktop::deployment::optimization::tests
cargo test --features desktop --lib snapshot::site::tests
```

Cover selected suites only, symlinks, malformed paths/hashes, missing index/images, source preservation, file/size limits, and failed/cancelled staging. Assert exact ZIP entry sets with stale generated files, captures, state, and unselected suites present. Test unchanged filenames after compression, cascading versions, identical bytes at different image paths, and duplicate path/version conflicts.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill for content or compression changes. Explain artifact allowlists and provider-limit decisions.
