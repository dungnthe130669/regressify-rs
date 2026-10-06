# Release Pipelines

Verified: 2026-10-06

- [GitHub](../../.github/workflows/tuyen-release.yml) and
  [Azure](../../.ado/workflows/tuyen-release.yaml) keep tag-only automatic Windows
  desktop releases and manual CLI/desktop target selection. Desktop selections
  schedule companion CLI jobs on supported targets. Tagged Windows releases build
  both binaries in parallel, retain separate payload artifacts, and join them in
  a dependent installer job; CLI-only archive releases remain separate.
  Azure omits macOS ARM64;
  GitHub retains it. Azure Intel macOS uses `macos-15`, not GitHub's `macos-15-intel`.
- Azure's `Plan.SetMatrix` validates the full source tag and emits `Matrix.matrix`,
  `Matrix.version` and `Matrix.buildInstaller` without Library secrets. `Release` alone links the `GitHub`
  and `Regressify` variable groups, and skips pull-request runs.
- Stage-level `lockBehavior: sequential` creates an Azure stage lock without a
  separate Library check, per the [Azure documentation](https://learn.microsoft.com/en-us/azure/devops/pipelines/process/approvals#exclusive-lock).
  GitHub and Azure locks are independent; publishing the same immutable version
  from both providers is not supported.
- Azure Windows uses Tuyen's native PowerShell installer with `CLI_INSTALL` under
  the agent temporary directory and `task.prependpath`. Shared Bash scripts run
  in explicit Git Bash. `ci/install-rust.sh` writes cross-compilation settings to
  `GITHUB_ENV`; the Azure wrapper forwards the linker and `CC` to subsequent tasks.
- Azure MSBuild discovery buffers `vswhere` output before selecting the first path.
  Direct piping into `Select-Object -First 1` can leave `$LASTEXITCODE` stale even
  with a valid MSBuild result; native Windows reproduction and the offline
  discovery regression verify the failure and fix.
- [Offline tests](../../ci/test-release.py) exercise 192 GitHub and 96 Azure
  selections, real publish scripts with a stub CLI, and Azure PowerShell wrappers.
  Use `python ci/test-release.py`; PyYAML, Bash, and PowerShell 7 are required.
  On Windows, set `BASH_PATH` to Git Bash for the build-argument tests.
  [Setup details](../../docs/desktop.md#azure-pipelines) include Library permissions
  and the hosted-job timeout limit. Hosted Azure execution remains a separate gate.
- The [installer](../../installer/regressify-desktop.iss) installs both executables
  directly in `{app}` and registers that shared directory only in the user PATH.
  Ownership metadata survives upgrades; uninstall preserves unowned entries,
  later PATH edits and the registry value type. Both packaging jobs set
  `ISCC_PATH` and run `python ci/test-release.py InstallerTests`: native fixtures
  use isolated registry keys and exit before installation. The separate release
  smoke test verifies actual CLI lookup from a fresh process and uninstall cleanup.
