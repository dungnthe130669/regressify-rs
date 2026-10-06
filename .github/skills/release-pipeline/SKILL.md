---
name: release-pipeline
description: 'Use when changing GitHub Actions or Azure Pipelines release matrices, Rust target builds, artifact names, Tuyen Drive manifests, installer hashes, or publish verification.'
---

# Release Pipeline

## Start Here

[GitHub release](../../workflows/tuyen-release.yml), [Azure release](../../../.ado/workflows/tuyen-release.yaml), [build script](../../../ci/make-release.sh), [Drive helper](../../../ci/drive_release.py), and [offline release tests](../../../ci/test-release.py). Frontend asset publication uses [snapshot-assets.yml](../../workflows/snapshot-assets.yml).

## Procedure and Invariants

1. Trace matrix inputs to target/toolchain/features, artifact names, upload fields, installer versions, and update manifests. Do not assume CLI/desktop targets match.
1. Desktop selections also schedule a CLI job for each supported desktop target without duplicates; desktop-only Musl selections still fail. Tagged Windows payloads build independently with static runtimes. A dependent installer job must download both exact artifacts from the same run and stop if either build fails. CLI-only archive publishing stays separate.
1. Preserve `--locked`, static-runtime flags where required, archive contents, executable resource versions, and immutable installer naming/hash/size contracts.
1. Keep preparation/verification separate from publication. A script or workflow edit does not authorize a tag, release upload, real remote operation, or install.
1. Validate external responses and URLs with bounded reads and clear errors. Never print publishing secrets or embed credentials in artifacts/manifests.
1. Azure `Plan` must stay secret-free. Load `GitHub` and `Regressify` Library groups only in `Release`, map tokens only into their publishing steps, and reject pull-request releases. Distinguish automatic tag CI from manual runs on tags when building the matrix.
1. Keep Azure's stage-level `lockBehavior: sequential`; Azure DevOps Services creates the stage lock without an additional Library check. GitHub and Azure locks do not coordinate, so only one provider may publish a given version tag.
1. Azure Windows jobs use the native Tuyen PowerShell installer and explicit Git Bash, not the Windows Bash task's WSL environment. Preserve the Rust script's `GITHUB_ENV` forwarding for AArch64 Musl. Azure macOS ARM64 is intentionally omitted; do not assume GitHub image labels or hosted capabilities transfer to Azure.
1. Buffer native `vswhere` output before selecting the first MSBuild path. Direct piping into `Select-Object -First 1` can stop discovery before `$LASTEXITCODE` updates. Cover single/multiple matches, empty results, and nonzero exits; failures must not emit a PATH update.
1. Inspect path filters so changed release files actually trigger their intended offline gates. Do not claim a release-only installer test is a pull-request gate.
1. Frontend pushes on `release` build with the frozen Bun lockfile. The asset PR job must check out `master` independently and commit only `src/snapshot/data`. Keep build permissions read-only and PR credentials out of the build job. PRs are never automatically merged. Document GitHub Actions PR permissions and the optional `SNAPSHOT_ASSETS_TOKEN` for triggering downstream checks.
1. Snapshot PR publication uses `gh pr list/create/edit/close` with `GH_TOKEN`, not a PR action. Push or delete the automation branch only with an explicit observed-SHA lease. No asset diff means no commit; close the stale PR and remove its branch. Lookup, diff, commit, and push errors must propagate. Test this Bash path with mocked `git`/`gh`, never real remotes.

## Checks

```sh
python ci/test-release.py
```

Requires PyYAML. Tests should use synthetic credentials/local fixture responses, not live publishing commands. Run affected target build checks only when the toolchain exists; explicitly report untested cross-platform packaging.

For the Azure slice, use `python ci/test-release.py AzureReleaseTests`; the full suite also executes both providers' publish scripts with a stub CLI. Bash and PowerShell 7.4+ are required; on Windows, set `BASH_PATH` to Git Bash for the build-argument tests. Offline checks cannot validate Library permissions or execute a hosted Azure run.

Both Windows packaging jobs set `ISCC_PATH` and run `python ci/test-release.py InstallerTests` before packaging/smoke. This runs isolated native PATH fixtures and a synthetic setup/uninstall cycle through the smoke helper without installing the app; the production installer is also compiled with synthetic payloads. Process fixtures cover exit codes and timeout cleanup. Missing Inno skips the native checks outside the packaging jobs.

## Maintain

Update [README](../../../README.md), [desktop docs](../../../docs/desktop.md), applicable install docs, and this skill for release-contract changes. Keep scripts, workflow inputs, updater expectations, and test fixtures synchronized.
