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
1. Share the eight-hour automatic feed schedule through the per-user update cache.
   Acquire `coordinator.lock` before network checks and retain its handle across
   the UI handoff, download, and final state write. The existing `update.lock`
   protects state/payload operations inside that lease. Treat another owner as
   normal waiting, not a failure; do not create a second network operation.
1. Poll shared state every five seconds off the UI thread while windows stay open.
   Reuse and verify the cached installer in each instance before showing its
   banner; preserve ready banners during idle polls and keep checking for newer
   releases. Stop the watcher when its window closes.
1. Preserve explicit pause globally with a deferred preparation deadline, but let
   owner shutdown hand partial work to a peer. Keep checksum backoff separate.
   Clear expired local retry timers when a peer owns the cycle to avoid spinning.
   Manual checks can bypass deadlines only after acquiring ownership. Mixed old
   builds that do not use the coordinator cannot provide shared-check guarantees.

## Checks

```sh
cargo test --features desktop --lib desktop::updates::tests
cargo test --features gui-tests --lib desktop::ui::updates::tests -- --test-threads=1
```

Use existing fake feeds/local responses. Cover URL/version/hash/size mismatch,
truncated downloads, cancellation, retry deadlines, cache corruption, and launch
guards. Ordinary tests must not fetch or execute a real application update.
Cover the eight-hour boundary, restart/clock rollback, simultaneous owners,
cross-process lock release, one-transfer cache reuse, two-window notification,
newer releases after a ready banner, owner shutdown versus pause, and expired
retries while a peer owns preparation. Use injected time, fake feeds, local
servers, and temporary caches; GUI tests can shorten only the local poll interval.

## Maintain

Update [desktop docs](../../../docs/desktop.md), release/installer contracts, and
this skill together. Comment handle/mutex ownership and integrity assumptions.
