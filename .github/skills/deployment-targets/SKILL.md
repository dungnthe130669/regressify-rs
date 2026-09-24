---
name: deployment-targets
description: 'Use when changing desktop deployment targets, saved target configuration, OS keyring credentials, target identity, token replacement, or provider switching.'
---

# Deployment Targets and Credentials

## Start Here

[Target model](../../../src/desktop/deployment.rs), [credential store](../../../src/desktop/deployment/credentials.rs), and [desktop deployment UI](../../../src/desktop/ui).

## Procedure and Invariants

1. Keep public target settings separate from tokens. Scope credentials by canonical workspace identity and validated target credential ID; avoid cross-workspace reuse.
1. Use the OS credential abstraction and zeroizing secret buffers. Never serialize tokens into target files, run snapshots, progress events, logs, or diagnostic text.
1. Preserve existing credentials when an edit does not replace them. Make target save/provider-switch/delete failure paths explicit; do not leave a saved target referencing a removed secret or silently lose a previously usable credential.
1. Validate target/provider/suite selection again at execution. Missing credentials or unavailable secure storage must produce actionable secret-free errors.
1. Cloudflare uses `provider: cloudflareWorkers` with a `cloudflare` object containing `accountId` and `workerName`; never repurpose Azure `environment` or Netlify `siteId`. Keep that object absent for other providers and preserve legacy defaults.
1. Keep production Windows credential persistence semantics separate from the mock backend used in tests. Do not validate against the user's actual keyring.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::credentials::tests
cargo test --features desktop --lib desktop::deployment::tests
```

Cover workspace isolation, create/edit/delete, blank/missing tokens, provider changes, failed saves, corrupt target files, and errors that omit synthetic secrets.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill when storage or target workflow changes. Comment credential ownership and rollback decisions.
