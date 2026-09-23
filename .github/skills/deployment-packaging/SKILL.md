---
name: deployment-packaging
description: 'Use when changing deployment staging, report asset collection, image compression, ZIP creation, provider size/file limits, or artifact path validation.'
---

# Deployment Packaging

## Start Here

[Staging/artifact selection](../../../src/desktop/deployment.rs),
[ZIP package](../../../src/desktop/deployment/package.rs), and
[snapshot generation](../../../src/snapshot/report.rs).

## Procedure and Invariants

1. Package only selected report artifacts from the reviewed output root. Validate
   report image paths, source ownership, and each existing path component.
1. Reject symlink escapes and non-regular files. Preserve nested paths inside the
   archive and require the entry report; never include arbitrary workspace files,
   persisted browser state, credentials, or caches.
1. Preserve separate source/staging copies. Optional deployment compression must
   not rewrite reference/test captures or change the CLI optimizer's lossless contract.
1. Enforce provider file limits and compressed/uncompressed byte bounds. Detect
   content changing during copy and clean staging/temporary packages on failure.
1. Keep deterministic ordering and cancellation/progress updates during large
   packages. A completed ZIP is not evidence that remote deployment succeeded.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::package::tests
cargo test --features desktop --lib desktop::deployment::tests
```

Cover selected suites only, nested assets, symlinks, malformed report paths, missing
index/images, source preservation, file/size limits, and failed/cancelled staging.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill for content or
compression changes. Explain artifact allowlists and provider-limit decisions.
