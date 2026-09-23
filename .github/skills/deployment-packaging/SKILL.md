---
name: deployment-packaging
description: 'Use when changing deployment staging, report asset collection, image compression, ZIP creation, digest manifests, provider size/file limits, or artifact path validation.'
---

# Deployment Packaging

## Start Here

[Staging/artifact selection](../../../src/desktop/deployment.rs), [Azure ZIP package](../../../src/desktop/deployment/package.rs), [Netlify manifests](../../../src/desktop/deployment/netlify.rs), and [snapshot generation](../../../src/snapshot/report.rs).

## Procedure and Invariants

1. Package only selected report artifacts from the reviewed output root. Validate report image paths, source ownership, and each existing path component. Select capture PNGs from `reference`, `test`, and `diffImage` in each selected suite's `html_report/config.js`, not by walking capture folders or choosing the newest run. Retain referenced older-run images and deduplicate paths.
1. Keep the shared asset allowlist aligned with report templates and their script, worker, and font dependencies. WOFF/WOFF2 fonts are used; TTF fonts are not. Unreferenced files must not enter staging, Azure ZIPs, or Netlify manifests.
1. Reject symlink escapes and non-regular files. Preserve nested paths in archives/manifests and require the entry report; never include arbitrary workspace files, persisted browser state, credentials, or caches.
1. Preserve separate source/staging copies. Optional deployment compression must not rewrite reference/test captures or change the CLI optimizer's lossless contract.
1. Enforce provider file limits and compressed/uncompressed byte bounds. Detect content changing during copy and clean staging/temporary packages on failure.
1. Keep deterministic ordering and cancellation/progress updates during packaging/hashing. Azure uploads a ZIP; Netlify uploads requested content from a full SHA-1 manifest. Neither a completed ZIP nor a submitted manifest proves publication succeeded.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::package::tests
cargo test --features desktop --lib desktop::deployment::tests
cargo test --features desktop --lib desktop::deployment::netlify::tests
```

Cover selected suites only, nested assets, symlinks, malformed report paths, missing index/images, source preservation, file/size limits, and failed/cancelled staging. Assert exact ZIP entry sets with unused old/current-run captures, references, diffs, metadata, shared assets, and unselected suites present in the source. Cover referenced older-run images and duplicate cache-busted paths too.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill for content or compression changes. Explain artifact allowlists and provider-limit decisions.
