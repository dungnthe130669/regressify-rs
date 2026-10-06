---
name: windows-installer
description: 'Use when changing Inno Setup packaging, Windows installer versions, shell shortcuts/context menus, Chromium preparation, silent progress, upgrade/uninstall, or installer smoke tests.'
---

# Windows Installer

## Start Here

[Inno script](../../../installer/regressify-desktop.iss),
[builder/smoke](../../../ci/build-desktop-installer.ps1),
[release tests](../../../ci/test-release.py), and [resources](../../../build.rs).

## Procedure and Invariants

1. Preserve canonical four-part `N.N.N.N` installer/PE versions, matching executable
   resources, x64 artifact naming, and static MSVC runtime verification.
1. Require both `regressify-desktop.exe` and `regressify.exe` in the payload source
   directory and install them together in `{app}`. Verify each static runtime and
   payload hash; the CLI keeps its Cargo version, not the desktop tag version.
1. Register `{app}` only in the user PATH. Preserve raw values, registry types,
   pre-existing entries and later edits; keep ownership across upgrades and remove
   only the owned entry on uninstall. Notify Explorer of environment changes.
1. Keep setup/app exclusion, worker-startup guards, per-user shell integration,
   AppUserModel identity, and quoted workspace relaunch paths aligned.
1. Attempt managed Chromium preparation through the intended headless helper.
   Cache reuse and recoverable offline failure must remain distinguishable in logs.
1. Inno `WizardSilent` includes both `/SILENT` and `/VERYSILENT`. Visible progress
   must follow actual wizard visibility; failure dialogs retain their silent guard.
1. `ExecAndLogOutput` callback Error means capture failure/limit, not stderr. Normal
   Chromium progress uses stderr; do not remove error checks to display progress.
1. Preserve user data on upgrade/uninstall. Smoke tests must refuse an existing
   app registration, shell integration or PATH ownership record; never bypass
   these guards on this machine.

## Checks

```sh
python ci/test-release.py
```

Requires PyYAML. For authorized isolated Windows installer testing, build the
matching desktop executable and companion CLI, then run `./ci/build-desktop-installer.ps1` with `-Version`,
`-Compiler`, `-RequireStaticRuntime`, and `-SmokeTest`. Use the actual four-part
release version, not the three-part Cargo package version. Inno Setup is required.

Set `ISCC_PATH` to an Inno Setup 6 compiler and run
`python ci/test-release.py InstallerTests` for native PATH fixtures and a
synthetic-payload compile without installing the app or changing the real user
PATH. Both packaging jobs require this gate. The Inno 6 setup engine is 32-bit
even in x64 install mode; keep external Win32 declarations compatible. Inno's
`RegWriteStringValue` preserves an existing `REG_EXPAND_SZ` type, so remove only
the isolated fixture value before seeding a `REG_SZ` test case.

## Maintain

Update [desktop docs](../../../docs/desktop.md), release tests, and this skill.
Comment silent-mode/platform workarounds and verify fresh install, upgrade,
uninstall, running-app rejection, shell paths, and offline preparation failure.
