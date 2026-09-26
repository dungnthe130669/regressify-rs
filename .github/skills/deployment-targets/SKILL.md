---
name: deployment-targets
description: 'Use when changing desktop deployment targets, saved target configuration, OS keyring credentials, target identity, token replacement, or provider switching.'
---

# Deployment Targets and Credentials

## Start Here

[Target model](../../../src/desktop/deployment.rs), [credential store](../../../src/desktop/deployment/credentials.rs), [named tokens](../../../src/desktop/deployment/tokens.rs), and [desktop deployment UI](../../../src/desktop/ui).

## Procedure and Invariants

1. Keep public target settings separate from secrets. Direct-entry credentials remain scoped to canonical workspace/landing identity and target ID. Explicit named tokens use a separate per-user `named:<uuid>` keyring namespace; never implicitly share a direct-entry credential.
1. Use the OS credential abstraction and zeroizing secret buffers. Never serialize tokens into target files, run snapshots, progress events, logs, or diagnostic text.
1. Preserve existing credentials when an edit does not replace them. Make target save/provider-switch/delete failure paths explicit; do not leave a saved target referencing a removed secret or silently lose a previously usable credential.
1. Validate target/provider/suite selection again at execution. Missing credentials or unavailable secure storage must produce actionable secret-free errors.
1. Cloudflare uses `provider: cloudflareWorkers` with a `cloudflare` object containing `accountId` and `workerName`; never repurpose Azure `environment` or Netlify `siteId`. Keep that object absent for other providers and preserve legacy defaults.
1. Keep production Windows credential persistence semantics separate from the mock backend used in tests. Do not validate against the user's actual keyring.
1. Named tokens support Netlify and Cloudflare, with immutable provider/account identity and editable name/secret. Serialize only metadata and `namedTokenId` references. Revalidate provider/account when linking and executing; deleting a target must never remove a named token.
1. Catalog writes lock and compare the loaded catalog, rejecting stale windows. Preserve secrets on failed metadata saves and clear masked drafts on cancel/close. Named-token removal must warn that referencing targets will require another token.
1. Landing targets use app-managed identity, not linked-site workspaces. See the [landing skill](../desktop-landing-apps/SKILL.md).
1. Store the optional `publishedUrl` with its target ID after successful publication. Reload current settings before updating, preserve other targets/selection, and reject removed or changed destinations. Accept only credential-free HTTPS URLs without query/fragment data. Failure/cancellation retains the previous URL.
1. Name-only edits and Netlify/Cloudflare token changes preserve the URL. Destination changes clear it; replacing an Azure token also clears it because the token identifies the app. Surface persistence errors without hiding a successful URL. Report completion writes in the runner's background persistence task; landing completion writes before returning to the UI. Both dialogs read the selected target.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::credentials::tests
cargo test --features desktop --lib desktop::deployment::tests
cargo test --features desktop --lib desktop::deployment::tokens::tests
cargo test --features desktop --lib completed_deployments_persist_urls
cargo test --features gui-tests --lib saved_deployment_urls_reopen_and_copy -- --test-threads=1
cargo test --features gui-tests --lib desktop::ui::tokens::tests -- --test-threads=1
```

Cover workspace isolation, create/edit/delete, blank/missing tokens, provider changes, failed saves, corrupt target files, and errors that omit synthetic secrets. Run `./ci/desktop-smoke.ps1 -PublishingFlows` for named-token and landing controls; run the smoke harness self-tests first and preserve the interactive-desktop guards.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill when storage or target workflow changes. Comment credential ownership and rollback decisions.
