---
name: browser-storage-state
description: 'Use when changing persist/restore, cookies, origin-scoped localStorage, state-name validation, authentication, or environment-specific browser state files.'
---

# Browser Storage State

## Start Here

[Origin tracking](../../../src/test/storage_state.rs),
[state paths and files](../../../src/test/capture.rs), and
[authentication](../../../src/test/authentication.rs).

## Procedure and Invariants

1. Validate portable state names before filesystem access. Use the active output
   directory's states folder, including site-env/custom-output isolation.
1. Validate cookies and exact HTTP(S) origins before browser injection. Keep each
   origin's localStorage isolated and restore it at the correct document lifecycle.
1. Preserve legacy-state compatibility deliberately; do not assign another
   origin's values to the current page during navigation or multi-origin capture.
1. Surface state-stream overflow/closure, malformed files, and write failures.
   Keep tracker shutdown tied to browser/scenario ownership.
1. Share origin tracking across a scenario's tabs. Seed popup document scripts
   from live tracked state, register storage/auth listeners before resuming the
   target, and preserve origins from other tabs when persisting the selected page.
1. Treat state, cookies, and Basic Auth as secrets. Never log their contents or
   place real account data in fixtures, reports, examples, or completion help.

## Checks

```sh
cargo test --lib test::storage_state::tests
cargo test --lib test::capture::tests
cargo test --lib config::basic_auth::tests
```

Use synthetic state and local origins. Cover legacy/new formats, multiple origins,
invalid origin/path/name, environment isolation, navigation, and tracker failures.
Run ignored live-browser tests explicitly only after checking prerequisites.

## Maintain

Update persist/restore sections in [action docs](../../../docs/create-action.md),
[scenario docs](../../../docs/create-scenario.md), and this skill. Explain origin
and restoration-order security decisions with concise comments.
