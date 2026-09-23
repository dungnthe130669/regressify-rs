---
name: desktop-updates
description: 'Use when changing desktop update feeds, manifest/version validation, installer downloads, cache/retry state, SHA-256 verification, or launching updates.'
---

# Desktop Self-Updates

## Start Here

[Update model](../../../src/desktop/updates.rs),
[desktop startup](../../../src/bin/desktop.rs), and
[installer](../../../installer/regressify-desktop.iss).

## Procedure and Invariants

1. Preserve Windows x64 availability and validated app slugs. Require the exact
   immutable app/version HTTPS installer URL, bounded manifest/size, and valid hash.
1. Keep HTTPS-only production clients with redirects disabled. Reject malformed,
   duplicate, mismatched, or oversized manifest fields; do not silently trust cache.
1. Bound total/idle download time and received bytes. Verify length and SHA-256
   before promotion; never launch partial or failed downloads. Preserve retry,
   cancellation, manual retry reset, and offline-cache semantics.
1. Reverify immediately before launch and retain the Windows read-sharing handle
   that prevents payload replacement while spawning. Do not remove it as redundant.
1. Preserve app/setup mutex coordination and workspace argument boundaries.
   Launching an installer changes the user's installation and requires authorization.

## Checks

```sh
cargo test --features desktop --lib desktop::updates::tests
```

Use existing fake feeds/local responses. Cover URL/version/hash/size mismatch,
truncated downloads, cancellation, retry deadlines, cache corruption, and launch
guards. Ordinary tests must not fetch or execute a real application update.

## Maintain

Update [desktop docs](../../../docs/desktop.md), release/installer contracts, and
this skill together. Comment handle/mutex ownership and integrity assumptions.
