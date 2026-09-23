---
name: desktop-instance
description: 'Use when changing per-workspace desktop ownership, activation IPC, endpoint locks/tokens, recent workspaces, Jump Lists, or native launch arguments.'
---

# Desktop Ownership and Activation

## Start Here

[Instance protocol](../../../src/desktop/instance.rs),
[Windows Jump Lists](../../../src/desktop/jump_list.rs), and
[startup dispatch](../../../src/bin/desktop.rs).

## Procedure and Invariants

1. Own one instance per canonical workspace, not one global desktop. Preserve
   Windows case-normalized identity and hold the workspace lock for its lifetime.
1. Publish endpoints atomically and bound startup retries, message bytes, reads,
   and acknowledgements. Keep localhost binding and per-owner activation tokens.
1. Activation requests only foreground the existing owner; do not turn the
   channel into arbitrary command execution or accept a different workspace path.
1. Clean endpoint/listener state on failure and close while still respecting lock
   ownership. A readable local token is not remote authentication or privilege isolation.
1. Keep Jump List app identity, portable argument quoting, recent-workspace
   deduplication, and preference merging consistent with installer shortcuts.

## Checks

```sh
cargo test --features desktop --lib desktop::instance::tests
cargo test --features gui --lib desktop::jump_list::tests
```

Jump Lists are Windows-only. Cover simultaneous opens, malformed/oversized IPC,
incorrect tokens, stale endpoints, close/reopen, distinct workspaces, and paths
with spaces/trailing separators. Use native smoke for foreground behavior.

## Maintain

Update [desktop docs](../../../docs/desktop.md), installer shell-integration tests,
and this skill. Explain local IPC trust limits and platform locking rules.
