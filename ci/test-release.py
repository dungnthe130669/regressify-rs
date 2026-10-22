"""Exercise the release workflow and Cargo arguments without building or publishing."""

import itertools
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import urllib.error
from unittest import mock

import drive_release
import yaml


ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = yaml.load(
    (ROOT / ".github/workflows/tuyen-release.yml").read_text(encoding="utf-8"),
    Loader=yaml.BaseLoader,
)
MATRIX_STEP = next(
    step for step in WORKFLOW["jobs"]["set-matrix"]["steps"] if step.get("id") == "matrix"
)
TARGETS = {
    "MACOS_X86_64": "x86_64-apple-darwin",
    "MACOS_AARCH64": "aarch64-apple-darwin",
    "WINDOWS_X86_64": "x86_64-pc-windows-msvc",
    "LINUX_MUSL_X86_64": "x86_64-unknown-linux-musl",
    "LINUX_MUSL_AARCH64": "aarch64-unknown-linux-musl",
    "LINUX_GNU_X86_64": "x86_64-unknown-linux-gnu",
}


class ReleaseTests(unittest.TestCase):
    def test_snapshot_frontend_workflow_builds_release_with_locked_dependencies(self):
        workflow = yaml.load(
            (ROOT / ".github/workflows/snapshot-assets.yml").read_text(encoding="utf-8"),
            Loader=yaml.BaseLoader,
        )
        self.assertEqual(workflow["on"]["push"]["branches"], ["release"])
        self.assertEqual(workflow["on"]["push"]["paths"], ["frontend/**"])
        self.assertEqual(workflow["permissions"], {"contents": "read"})
        self.assertEqual(workflow["concurrency"]["group"], "snapshot-frontend-assets")
        steps = workflow["jobs"]["build"]["steps"]
        checkout = next(step for step in steps if step.get("uses", "").startswith("actions/checkout@"))
        self.assertIn("github.sha", checkout["with"]["ref"])
        self.assertIn("'release'", checkout["with"]["ref"])
        self.assertEqual(checkout["with"]["persist-credentials"], "false")
        commands = [step["run"] for step in steps if step.get("working-directory") == "frontend"]
        self.assertEqual(commands, ["bun install --frozen-lockfile", "bun run lint", "bun run test", "bun run build"])
        upload = next(step for step in steps if step.get("uses", "").startswith("actions/upload-artifact@"))
        self.assertEqual(upload["with"]["path"], "src/snapshot/data/")

    def test_snapshot_frontend_pr_contains_generated_assets_only(self):
        workflow = yaml.load(
            (ROOT / ".github/workflows/snapshot-assets.yml").read_text(encoding="utf-8"),
            Loader=yaml.BaseLoader,
        )
        job = workflow["jobs"]["pull-request"]
        self.assertEqual(job["needs"], "build")
        self.assertEqual(job["permissions"], {"contents": "write", "pull-requests": "write"})
        checkout = next(step for step in job["steps"] if step.get("uses", "").startswith("actions/checkout@"))
        self.assertEqual(checkout["with"]["ref"], "master")
        copy = next(step for step in job["steps"] if step.get("name") == "Replace generated assets only")
        self.assertIn('rsync -a --delete "$RUNNER_TEMP/snapshot-assets/" src/snapshot/data/', copy["run"])
        self.assertIn("git diff --cached --name-only", copy["run"])
        pr = next(step for step in job["steps"] if step.get("name") == "Open or update generated-assets PR")
        self.assertNotIn("uses", pr)
        self.assertEqual(pr["shell"], "bash")
        self.assertEqual(pr["env"]["GH_TOKEN"], "${{ secrets.SNAPSHOT_ASSETS_TOKEN || github.token }}")
        self.assertEqual(pr["env"]["GH_REPO"], "${{ github.repository }}")
        self.assertEqual(pr["env"]["SOURCE_SHA"], "${{ needs.build.outputs.source }}")
        self.assertIn("branch=automation/snapshot-frontend-assets", pr["run"])
        self.assertIn('gh pr create --repo "$GH_REPO" --base master --head "$branch"', pr["run"])
        self.assertIn('gh pr edit "$pr_number" --repo "$GH_REPO" --base master', pr["run"])
        self.assertIn('git push --force-with-lease="$ref:$remote_sha" origin "HEAD:$ref"', pr["run"])
        self.assertIn("git diff --cached --quiet", pr["run"])
        self.assertNotIn("gh pr merge", pr["run"])
        self.assertNotIn("${{", pr["run"])

    def snapshot_pr(self, *, changed=True, pr_number="", remote_sha="", failure=""):
        bash = shutil.which("bash")
        if not bash:
            self.skipTest("Bash is required for the offline snapshot workflow checks")
        workflow = yaml.load(
            (ROOT / ".github/workflows/snapshot-assets.yml").read_text(encoding="utf-8"),
            Loader=yaml.BaseLoader,
        )
        script = next(step["run"] for step in workflow["jobs"]["pull-request"]["steps"]
                      if step.get("name") == "Open or update generated-assets PR")
        stub = r'''
set -euo pipefail
export GH_TOKEN=synthetic-offline-token GH_REPO=example/report SOURCE_SHA=abc123
record() { printf '%s ' "$1" >&2; shift; printf '<%s>' "$@" >&2; printf '\n' >&2; }
git() {
  record git "$@"
  if [ "$1" = "$failure" ]; then return 23; fi
  case "$1" in
    ls-remote) if [ -n "$remote_sha_fixture" ]; then printf '%s\trefs/heads/automation/snapshot-frontend-assets\n' "$remote_sha_fixture"; fi ;;
    diff) return "$diff_status" ;;
    config|commit|push) ;;
    *) return 97 ;;
  esac
}
gh() {
  record gh "$@"
  if [ "$failure" = "gh" ]; then return 24; fi
  case "$1 $2" in
    'pr list') printf '%s' "$pr_number_fixture" ;;
    'pr create'|'pr edit'|'pr close') ;;
    *) return 98 ;;
  esac
}
'''
        # Feed the script through stdin so Windows checkout line endings and WSL
        # path conversion cannot change which workflow commands are exercised.
        values = f"diff_status={1 if changed else 0}\npr_number_fixture={pr_number}\nremote_sha_fixture={remote_sha}\nfailure={failure}\n"
        result = subprocess.run(
            [bash, "--noprofile", "--norc", "-s"],
            input=(values + stub + script).encode("utf-8"), capture_output=True,
            check=False, timeout=15,
        )
        return subprocess.CompletedProcess(
            result.args, result.returncode,
            result.stdout.decode("utf-8"), result.stderr.decode("utf-8"),
        )

    def test_snapshot_frontend_gh_creates_and_updates_prs_after_safe_push(self):
        for pr_number, remote_sha in [("", ""), ("42", "old-head")]:
            with self.subTest(pr_number=pr_number):
                result = self.snapshot_pr(pr_number=pr_number, remote_sha=remote_sha)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("git <commit><-m><build(snapshot): refresh generated frontend assets>", result.stderr)
                push = f"git <push><--force-with-lease=refs/heads/automation/snapshot-frontend-assets:{remote_sha}><origin><HEAD:refs/heads/automation/snapshot-frontend-assets>"
                command = "gh <pr><edit><42>" if pr_number else "gh <pr><create>"
                self.assertIn(push, result.stderr)
                self.assertIn(command, result.stderr)
                self.assertLess(result.stderr.index(push), result.stderr.index(command))
                self.assertIn("<--base><master>", result.stderr)
                self.assertIn("release commit `abc123`", result.stderr)
                self.assertNotIn("<merge>", result.stderr)
                self.assertNotIn("synthetic-offline-token", result.stderr)

    def test_snapshot_frontend_gh_skips_empty_updates_and_cleans_stale_prs(self):
        for pr_number, remote_sha in [("", ""), ("42", "old-head"), ("", "merged-head")]:
            with self.subTest(pr_number=pr_number, remote_sha=remote_sha):
                result = self.snapshot_pr(changed=False, pr_number=pr_number, remote_sha=remote_sha)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertNotIn("git <commit>", result.stderr)
                self.assertNotIn("gh <pr><create>", result.stderr)
                self.assertNotIn("gh <pr><edit>", result.stderr)
                self.assertEqual("gh <pr><close><42>" in result.stderr, bool(pr_number))
                self.assertEqual("git <push>" in result.stderr, bool(remote_sha))
                if remote_sha:
                    self.assertIn(f"<--force-with-lease=refs/heads/automation/snapshot-frontend-assets:{remote_sha}><origin><:refs/heads/automation/snapshot-frontend-assets>", result.stderr)

    def test_snapshot_frontend_gh_stops_on_lookup_commit_or_push_failure(self):
        for failure in ["gh", "ls-remote", "diff", "commit", "push"]:
            with self.subTest(failure=failure):
                result = self.snapshot_pr(pr_number="42", remote_sha="old-head", failure=failure)
                self.assertEqual(result.returncode, 24 if failure == "gh" else 23, result.stderr)
                for action in ["create", "edit", "close"]:
                    self.assertNotIn(f"gh <pr><{action}>", result.stderr)
                if failure != "push":
                    self.assertNotIn("git <push>", result.stderr)

    def matrix(self, build, selected, event="workflow_dispatch", ref_type="branch"):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            env = dict(
                os.environ, BUILD=build, GITHUB_OUTPUT=str(output),
                GITHUB_EVENT_NAME=event, GITHUB_REF_TYPE=ref_type,
            )
            env.update({key: str(key in selected).lower() for key in TARGETS})
            result = subprocess.run(
                [sys.executable, "-c", MATRIX_STEP["run"]],
                env=env, capture_output=True, text=True, check=False,
            )
            rows = None
            if output.exists():
                records = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                rows = json.loads(records["matrix"])["include"]
                self.assertEqual(records["installer"], str(any(row["installer"] for row in rows)).lower())
            return result, rows

    def test_tag_push_builds_windows_desktop_and_cli(self):
        result, rows = self.matrix("", [], event="push", ref_type="tag")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            [(row["application"], row["target"]) for row in rows],
            [("cli", "x86_64-pc-windows-msvc"), ("desktop", "x86_64-pc-windows-msvc")],
        )
        self.assertNotIn("regressify", json.dumps(rows))

    def test_manual_desktop_also_selects_companion_cli(self):
        result, rows = self.matrix("desktop", ["WINDOWS_X86_64"])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            {(row["application"], row["target"]) for row in rows},
            {("cli", "x86_64-pc-windows-msvc"), ("desktop", "x86_64-pc-windows-msvc")},
        )

    def test_matrix_job_does_not_load_secrets(self):
        self.assertNotIn(
            "secrets.", json.dumps(WORKFLOW["jobs"]["set-matrix"]),
            "Release planning must not load secrets that can cause GitHub to suppress its job outputs",
        )

    def test_tagged_source_and_serial_publish(self):
        self.assertIn("v*.*.*", WORKFLOW["on"]["push"]["tags"])
        self.assertEqual(WORKFLOW["concurrency"]["cancel-in-progress"], "false")
        self.assertNotIn("github.ref", WORKFLOW["concurrency"]["group"])
        for job in WORKFLOW["jobs"].values():
            for step in job["steps"]:
                if step.get("uses", "").startswith("actions/checkout@"):
                    self.assertNotEqual(step.get("with", {}).get("ref"), "release")

    def test_windows_publishing_is_tag_gated_and_keeps_token_out_of_the_build(self):
        job = WORKFLOW["jobs"]["installer"]
        self.assertEqual(job["needs"], ["set-matrix", "release"])
        self.assertEqual(job["if"], "${{ needs.set-matrix.outputs.installer == 'true' }}")
        self.assertEqual(job["runs-on"], "windows-latest")
        self.assertEqual(WORKFLOW["jobs"]["set-matrix"]["outputs"]["installer"], "${{ steps.matrix.outputs.installer }}")
        steps = job["steps"]
        builds = WORKFLOW["jobs"]["release"]["steps"]
        build = next(step for step in builds if step.get("name") == "Build Windows desktop release")
        self.assertEqual(build["if"], "${{ matrix.installer && matrix.application == 'desktop' }}")
        self.assertEqual(build["env"]["REGRESSIFY_DESKTOP_VERSION"], "${{ needs.set-matrix.outputs.version }}")
        self.assertEqual(build["env"]["TUYEN_DRIVE_APP"], "${{ secrets.TUYEN_DRIVE_APP }}")
        self.assertNotIn("TUYEN_DRIVE_TOKEN", build["env"])
        self.assertIn("+crt-static", build["env"]["RUSTFLAGS"])
        cli = next(step for step in builds if step.get("name") == "Build Windows CLI release")
        self.assertEqual(cli["if"], "${{ matrix.installer && matrix.application == 'cli' }}")
        self.assertIn("cargo build --locked --no-default-features --bin regressify --release --target x86_64-pc-windows-msvc", cli["run"])
        self.assertIn("if ($LASTEXITCODE -ne 0)", cli["run"])
        self.assertEqual(cli["env"]["BINARY_GIT_HASH"], "${{ github.sha }}")
        self.assertIn("+crt-static", cli["env"]["RUSTFLAGS"])
        self.assertNotIn("secrets.", json.dumps(cli))
        payload = next(step for step in builds if step.get("name") == "Retain installer payload")
        self.assertEqual(payload["if"], "${{ matrix.installer }}")
        self.assertEqual(payload["with"]["name"], "regressify-installer-${{ matrix.application }}")
        self.assertEqual(payload["with"]["path"], "target/x86_64-pc-windows-msvc/release/${{ matrix.application == 'desktop' && 'regressify-desktop' || 'regressify' }}.exe")
        self.assertEqual(payload["with"]["if-no-files-found"], "error")
        downloads = [step for step in steps if step.get("uses", "").startswith("actions/download-artifact@")]
        self.assertEqual({step["with"]["name"] for step in downloads}, {"regressify-installer-cli", "regressify-installer-desktop"})
        package = next(step for step in steps if step.get("name") == "Build Windows installer")
        self.assertIn('$env:ISCC_PATH = $compiler', package["run"])
        self.assertIn('python ci/test-release.py InstallerTests', package["run"])
        for download in downloads:
            self.assertEqual(download["with"]["path"], "target/x86_64-pc-windows-msvc/release")
            self.assertLess(steps.index(download), steps.index(package))
        for step in builds + steps:
            self.assertNotIn("continue-on-error", step)
            self.assertNotIn("always()", step.get("if", ""))
            if step.get("name") != "Publish Windows installer to Tuyen Drive":
                self.assertNotIn("TUYEN_DRIVE_TOKEN", step.get("env", {}))
        publish = next(step for step in steps if step.get("name") == "Publish Windows installer to Tuyen Drive")
        self.assertEqual(publish["env"]["TUYEN_DRIVE_TOKEN"], "${{ secrets.TUYEN_DRIVE_TOKEN }}")
        self.assertEqual(publish["env"]["INSTALLER_VERSION"], "${{ needs.set-matrix.outputs.version }}")
        self.assertIn("tuyen drive upload-app", publish["run"])
        self.assertNotIn("python", publish["run"])
        setup = next(step for step in steps if step.get("name") == "Setup Tuyen CLI")
        self.assertEqual(setup["uses"], "tuyen-at-work/tuyen-cli-action@master")
        support = next(step for step in steps if step.get("name") == "Check Tuyen Drive upload support")
        self.assertIn("tuyen drive upload-app --help", support["run"])
        self.assertLess(steps.index(setup), steps.index(support))
        self.assertLess(steps.index(support), steps.index(package))
        retain = next(step for step in steps if step.get("name") == "Retain Windows installer")
        self.assertLess(steps.index(package), steps.index(retain))
        self.assertLess(steps.index(retain), steps.index(publish))
        verify = next(step for step in steps if step.get("name") == "Verify Windows installer publish")
        self.assertNotIn("TUYEN_DRIVE_TOKEN", verify["env"])
        self.assertIn("ci/drive_release.py verify", verify["run"])
        self.assertLess(steps.index(setup), steps.index(publish))
        self.assertLess(steps.index(publish), steps.index(verify))
        installer = (ROOT / "installer/regressify-desktop.iss").read_text(encoding="utf-8")
        updater = (ROOT / "src/desktop/updates.rs").read_text(encoding="utf-8")
        for setting in ("AppMutex", "SetupMutex"):
            mutex = next(line.split("=", 1)[1] for line in installer.splitlines() if line.startswith(setting + "="))
            self.assertIn(mutex.replace("\\", "\\\\"), updater)
        self.assertIn("PrivilegesRequired=lowest", installer)
        self.assertIn("CloseApplications=no", installer)
        self.assertIn("if CheckForMutexes(", installer)
        self.assertNotIn("[UninstallDelete]", installer)

    def test_windows_installer_foreground_handoff_is_smoke_tested(self):
        step = next(step for step in WORKFLOW["jobs"]["installer"]["steps"]
                    if step.get("name") == "Build Windows installer")
        self.assertIn("-SmokeTest", step["run"])
        installer = (ROOT / "installer/regressify-desktop.iss").read_text(encoding="utf-8")
        self.assertIn("procedure CurStepChanged(CurStep: TSetupStep);", installer)
        self.assertIn("(CurStep = ssInstall) and WizardForm.Visible", installer)
        self.assertIn("WizardForm.BringToFront;", installer)
        self.assertIn("SetForegroundWindow(WizardForm.Handle)", installer)
        smoke = (ROOT / "ci/build-desktop-installer.ps1").read_text(encoding="utf-8")
        self.assertIn("Fully silent installation attempted foreground activation.", smoke)
        self.assertIn("Visible update did not request foreground activation after the app closed.", smoke)

    def test_windows_installer_prepares_chromium_with_run_time_fallback(self):
        installer = (ROOT / "installer/regressify-desktop.iss").read_text(encoding="utf-8")
        desktop = (ROOT / "src/bin/desktop.rs").read_text(encoding="utf-8")
        smoke = (ROOT / "ci/build-desktop-installer.ps1").read_text(encoding="utf-8")
        self.assertIn("BeforeInstall: PrepareChromium", installer)
        self.assertIn("RegisterCliPath(ExpandConstant('{app}'));\n    PrepareChromium;", installer)
        self.assertIn("if ChromiumPrepared then", installer)
        self.assertIn("{param:SKIPCHROMIUM|0}", installer)
        self.assertIn("ExecAndLogOutput(", installer)
        self.assertIn("'--install-chromium'", installer)
        self.assertIn("ewWaitUntilTerminated, ResultCode, @ChromiumInstallLog", installer)
        self.assertIn("app installation remains successful and Reference/Test can retry", installer)
        self.assertIn("if not WizardSilent then", installer)
        self.assertLess(desktop.index("if args.install_chromium"), desktop.index("protect_installation(args.after_install)"))
        self.assertIn("block_on(regressify::test::fetcher())", desktop)
        self.assertIn("Assert-ChromiumPreparation $installLog", smoke)
        self.assertIn("Assert-ChromiumPreparation $foregroundLog", smoke)
        self.assertIn("http://127.0.0.1:9", smoke)
        self.assertIn("'/SKIPCHROMIUM=1'", smoke)

    def test_windows_installer_shows_chromium_progress_for_visible_updates(self):
        installer = (ROOT / "installer/regressify-desktop.iss").read_text(encoding="utf-8")
        updater = (ROOT / "src/desktop/updates.rs").read_text(encoding="utf-8")
        smoke = (ROOT / "ci/build-desktop-installer.ps1").read_text(encoding="utf-8")
        preparation = installer.split("procedure PrepareChromium;", 1)[1].split("Started := False;", 1)[0]
        self.assertIn('command.args(["/SP-", "/SILENT", "/NORESTART"]);', updater)
        self.assertIn("if WizardForm.Visible then", preparation)
        self.assertNotIn("WizardSilent", preparation)
        self.assertIn("ChromiumProgress.Show;", preparation)
        self.assertIn("ChromiumProgress.Animate;", preparation)
        self.assertIn("ChromiumProgress.SetText(S, '');", installer)
        self.assertIn("Log('Chromium progress: ' + S);", installer)
        self.assertIn("Assert-ChromiumPreparation $installLog $false", smoke)
        self.assertIn("Assert-ChromiumPreparation $foregroundLog $true", smoke)
        self.assertIn("Chromium progress: Checking Chromium cache", smoke)

    def test_tuyen_upload_arguments_and_failures(self):
        shell = shutil.which("pwsh")
        self.assertIsNotNone(shell, "PowerShell 7 is required for the Windows publish-step test")
        step = next(step for step in WORKFLOW["jobs"]["installer"]["steps"]
                    if step.get("name") == "Publish Windows installer to Tuyen Drive")
        pipeline = yaml.load(
            (ROOT / ".ado/workflows/tuyen-release.yaml").read_text(encoding="utf-8"),
            Loader=yaml.BaseLoader,
        )
        release = next(stage for stage in pipeline["stages"] if stage["stage"] == "Release")
        azure_step = next(step for job in release["jobs"] for step in job["steps"]
                          if step.get("displayName") == "Publish Windows installer to Tuyen Drive")
        scripts = {"GitHub": step["run"], "Azure": azure_step["pwsh"]}
        stub = '''
$ErrorActionPreference = 'Stop'
function tuyen {
    @($args) | ConvertTo-Json -Compress | Set-Content -LiteralPath $env:TUYEN_ARGUMENTS_FILE
    $global:LASTEXITCODE = [int]$env:TUYEN_TEST_EXIT
}
'''
        with tempfile.TemporaryDirectory(prefix="tuyen publish ") as directory:
            file = Path(directory) / drive_release.installer_name("1.2.3.0")
            file.write_bytes(b"MZ synthetic installer")
            arguments = Path(directory) / "arguments.json"
            for (provider, script), (app, token, version, cli_exit, invoked, succeeds) in itertools.product(scripts.items(), [
                ("regressify", "test-token", "1.2.3.0", 0, True, True),
                ("regressify", "test-token", "1.2.3.0", 7, True, False),
                ("", "test-token", "1.2.3.0", 0, False, False),
                ("regressify", "", "1.2.3.0", 0, False, False),
                ("regressify", "test-token", "1.2.4.0", 0, False, False),
            ]):
                with self.subTest(provider=provider, app=app, has_token=bool(token), version=version, cli_exit=cli_exit):
                    arguments.unlink(missing_ok=True)
                    env = dict(os.environ, TUYEN_DRIVE_APP=app, TUYEN_DRIVE_TOKEN=token,
                               INSTALLER=str(file), INSTALLER_VERSION=version,
                               TUYEN_ARGUMENTS_FILE=str(arguments), TUYEN_TEST_EXIT=str(cli_exit))
                    result = subprocess.run([shell, "-NoProfile", "-NonInteractive", "-Command", stub + script],
                                            env=env, capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode == 0, succeeds, result.stderr)
                    self.assertEqual(arguments.exists(), invoked)
                    self.assertNotIn("test-token", result.stdout + result.stderr)
                    if invoked:
                        self.assertEqual(json.loads(arguments.read_text(encoding="utf-8-sig")), [
                            "drive", "upload-app", "--name", app, "--token", token,
                            "--version", version, "--file", str(file), "--role", f"{file.name}=installer",
                        ])

    def test_selector_and_parallel_job_contract(self):
        selector = WORKFLOW["on"]["workflow_dispatch"]["inputs"]["build"]
        self.assertEqual(selector["type"], "choice")
        self.assertEqual(selector["default"], "both")
        self.assertEqual(set(selector["options"]), {"cli", "desktop", "both"})
        self.assertEqual(MATRIX_STEP["shell"], "python")
        self.assertEqual(MATRIX_STEP["env"]["BUILD"], "${{ inputs.build }}")
        for variable, input_name in [
            ("MACOS_X86_64", "macosX86_64"), ("MACOS_AARCH64", "macosAarch64"),
            ("WINDOWS_X86_64", "windowsX86_64"), ("LINUX_GNU_X86_64", "linuxGnuX86_64"),
            ("LINUX_MUSL_X86_64", "linuxMuslX86_64"), ("LINUX_MUSL_AARCH64", "linuxMuslAarch64"),
        ]:
            self.assertEqual(MATRIX_STEP["env"][variable], "${{ inputs." + input_name + " }}")
        job = WORKFLOW["jobs"]["release"]
        self.assertEqual(job["needs"], "set-matrix")
        self.assertEqual(job["strategy"]["fail-fast"], "false")
        self.assertNotIn("max-parallel", job["strategy"])
        self.assertEqual(job["strategy"]["matrix"], "${{ fromJson(needs.set-matrix.outputs.matrix) }}")
        build = next(step for step in job["steps"] if step.get("name") == "Build artifacts")
        self.assertEqual(build["if"], "${{ !matrix.installer }}")
        self.assertEqual(build["env"]["FILE_NAME"], "${{ matrix.application == 'desktop' && 'regressify-desktop' || 'regressify' }}")
        self.assertEqual(build["env"]["PACKAGE_NAME"], "${{ matrix.application == 'desktop' && 'regressify-desktop' || 'regressify-cli' }}")
        self.assertEqual(build["env"]["CARGO_FEATURES"], "${{ matrix.features }}")
        self.assertEqual(build["env"]["NO_DEFAULT_FEATURES"], "1")
        cache = next(step for step in job["steps"] if step.get("uses", "").startswith("actions/cache@"))
        self.assertIn("matrix.application", cache["with"]["key"])

    def test_every_application_and_platform_selection(self):
        for build in ("cli", "desktop", "both"):
            applications = ("cli",) if build == "cli" else ("cli", "desktop")
            for flags in itertools.product((False, True), repeat=len(TARGETS)):
                selected = [key for key, enabled in zip(TARGETS, flags) if enabled]
                with self.subTest(build=build, selected=selected):
                    targets = [TARGETS[key] for key in selected] or [TARGETS["LINUX_GNU_X86_64"]]
                    expected = {
                        (application, target)
                        for application in applications for target in targets
                        if application != "desktop" or not target.endswith("-musl")
                        if build != "desktop" or not target.endswith("-musl")
                    }
                    result, rows = self.matrix(build, selected)
                    if not expected:
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn("Musl is CLI-only", result.stderr)
                        self.assertIsNone(rows)
                        continue
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual({(row["application"], row["target"]) for row in rows}, expected)
                    self.assertEqual(len(rows), len(expected))
                    self.assertNotIn("regressify", json.dumps(rows))
                    for row in rows:
                        desktop = row["application"] == "desktop"
                        self.assertNotIn("binary", row)
                        self.assertNotIn("package", row)
                        self.assertEqual(row["features"], "gui" if desktop else "")
                        if desktop:
                            self.assertGreaterEqual(row["timeout"], 60)
                        if row["target"] == "x86_64-unknown-linux-gnu":
                            self.assertEqual(row["os"], "ubuntu-24.04" if desktop else "ubuntu-22.04")
                        if row["target"] == "x86_64-apple-darwin":
                            self.assertEqual(row["os"], "macos-15-intel")
                        if row["target"] == "aarch64-apple-darwin":
                            self.assertEqual(row["os"], "macos-15")

    def test_missing_selector_defaults_to_both_and_unknown_values_fail(self):
        result, rows = self.matrix("", [])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual({row["application"] for row in rows}, {"cli", "desktop"})
        result, rows = self.matrix("invalid", [])
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(rows)

    def test_installer_payloads_require_a_tagged_windows_desktop_selection(self):
        for build, ref_type, selected in itertools.product(
            ("cli", "desktop", "both"), ("branch", "tag"), ([], ["WINDOWS_X86_64"], ["WINDOWS_X86_64", "MACOS_AARCH64"]),
        ):
            with self.subTest(build=build, ref_type=ref_type, selected=selected):
                result, rows = self.matrix(build, selected, ref_type=ref_type)
                self.assertEqual(result.returncode, 0, result.stderr)
                payloads = [row for row in rows if row["installer"]]
                expected = {"cli", "desktop"} if ref_type == "tag" and build != "cli" and selected else set()
                self.assertEqual({row["application"] for row in payloads}, expected)
                self.assertTrue(all(row["target"] == "x86_64-pc-windows-msvc" for row in payloads))

    def test_release_build_arguments(self):
        bash = os.environ.get("BASH_PATH") or shutil.which("bash")
        self.assertIsNotNone(bash, "Bash is required; on Windows set BASH_PATH to Git Bash")
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            stubs = temporary / "stubs"
            stubs.mkdir()
            for name, source in {
                "rustc": "#!/usr/bin/env bash\nprintf 'host: x86_64-unknown-linux-gnu\\n'\n",
                "cargo": '#!/usr/bin/env bash\nprintf "%s\\n" "$@" > "$BUILD_ARGS_FILE"\nexit 73\n',
            }.items():
                path = stubs / name
                path.write_text(source, encoding="utf-8", newline="\n")
                path.chmod(0o755)
            arguments = temporary / "arguments"
            env = dict(
                os.environ, TEST_MODE="1", GITHUB_REF="refs/tags/test", OS="ubuntu-22.04",
                TARGET="x86_64-unknown-linux-gnu", BINARY_GIT_HASH="test", SKIP_BUILD="",
                REPO="", TOKEN="", STUB_DIR=stubs.as_posix(), BUILD_ARGS_FILE=arguments.as_posix(),
            )
            script = 'if command -v cygpath >/dev/null; then STUB_DIR="$(cygpath -u "$STUB_DIR")"; fi\n'
            script += 'export PATH="$STUB_DIR:$PATH"\nbash ci/make-release.sh'
            for binary, features, no_defaults in [
                ("regressify", "", "1"), ("regressify-desktop", "gui", "1"),
                ("regressify-desktop", "gui", ""), ("regressify", "local,cloud", ""),
            ]:
                with self.subTest(binary=binary, features=features, no_defaults=no_defaults):
                    env.update(FILE_NAME=binary, PACKAGE_NAME=binary, CARGO_FEATURES=features, NO_DEFAULT_FEATURES=no_defaults)
                    result = subprocess.run([bash, "-c", script], cwd=ROOT, env=env, capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 73, result.stderr)
                    expected = ["build", "--locked", "--bin", binary, "--release", "--target", env["TARGET"]]
                    if no_defaults:
                        expected.append("--no-default-features")
                    if features:
                        expected.extend(["--features", features])
                    self.assertEqual(arguments.read_text(encoding="utf-8").splitlines(), expected)

    def test_release_archives_include_license_files(self):
        bash = os.environ.get("BASH_PATH") or shutil.which("bash")
        self.assertIsNotNone(bash, "Bash is required; on Windows set BASH_PATH to Git Bash")
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            stubs = temporary / "stubs"
            stubs.mkdir()
            for name, source in {
                "rustc": "#!/usr/bin/env bash\nprintf 'host: x86_64-unknown-linux-gnu\\n'\n",
                "upx": "#!/usr/bin/env bash\n",
                "7z": '#!/usr/bin/env bash\nprintf "%s\\n" "$*" >> "$ARCHIVE_ARGS_FILE"\n: > "$2"\n',
            }.items():
                path = stubs / name
                path.write_text(source, encoding="utf-8", newline="\n")
                path.chmod(0o755)
            release = temporary / "target/x86_64-unknown-linux-gnu/release"
            release.mkdir(parents=True)
            (release / "regressify").write_bytes(b"synthetic binary")
            arguments = temporary / "archive-arguments"
            env = dict(
                os.environ, TEST_MODE="1", GITHUB_REF="refs/tags/test", OS="ubuntu-22.04",
                TARGET="x86_64-unknown-linux-gnu", BINARY_GIT_HASH="test", SKIP_BUILD="1",
                REPO="", TOKEN="", FILE_NAME="regressify", PACKAGE_NAME="regressify-cli",
                CARGO_TARGET_DIR=(temporary / "target").as_posix(), STUB_DIR=stubs.as_posix(),
                ARCHIVE_ARGS_FILE=arguments.as_posix(),
            )
            script = 'if command -v cygpath >/dev/null; then\n'
            script += '  STUB_DIR="$(cygpath -u "$STUB_DIR")"; CARGO_TARGET_DIR="$(cygpath -u "$CARGO_TARGET_DIR")"\n'
            script += '  ARCHIVE_ARGS_FILE="$(cygpath -u "$ARCHIVE_ARGS_FILE")"\nfi\n'
            script += 'export PATH="$STUB_DIR:$PATH"\nbash ci/make-release.sh'
            for password in ("", "synthetic-archive-password"):
                with self.subTest(password=bool(password)):
                    arguments.unlink(missing_ok=True)
                    result = subprocess.run([bash, "-c", script], cwd=ROOT, env=dict(env, PASSWORD=password),
                                            capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    archives = [line.split()[:5] for line in arguments.read_text(encoding="utf-8").splitlines()]
                    self.assertEqual(archives, [
                        ["a", f"regressify-cli-{env['TARGET']}.{extension}", "regressify", "LICENSE", "LICENSING.md"]
                        for extension in ("7z", "zip")
                    ])
                    for name in ("LICENSE", "LICENSING.md"):
                        self.assertEqual((release / name).read_bytes(), (ROOT / name).read_bytes())


class AzureReleaseTests(unittest.TestCase):
    def setUp(self):
        self.pipeline = yaml.load(
            (ROOT / ".ado/workflows/tuyen-release.yaml").read_text(encoding="utf-8"),
            Loader=yaml.BaseLoader,
        )
        self.plan = next(stage for stage in self.pipeline["stages"] if stage["stage"] == "Plan")
        self.release = next(stage for stage in self.pipeline["stages"] if stage["stage"] == "Release")
        self.steps = self.release["jobs"][0]["steps"]
        self.installer = next(job for job in self.release["jobs"] if job["job"] == "Installer")
        self.installer_steps = self.installer["steps"]

    def matrix(self, build, selected, reason="Manual", ref="refs/heads/master"):
        step = next(step for step in self.plan["jobs"][0]["steps"] if step.get("name") == "Matrix")
        env = dict(os.environ, BUILD=build, BUILD_REASON=reason, BUILD_SOURCEBRANCH=ref)
        env.update({key: str(key in selected) for key in TARGETS})
        result = subprocess.run(
            [sys.executable, "-c", step["inputs"]["script"]],
            cwd=ROOT, env=env, capture_output=True, text=True, check=False,
        )
        prefix = "##vso[task.setvariable variable=matrix;isOutput=true]"
        outputs = [line.removeprefix(prefix) for line in result.stdout.splitlines() if line.startswith(prefix)]
        rows = None
        if outputs:
            self.assertEqual(len(outputs), 1)
            matrix = json.loads(outputs[0])
            for name in matrix:
                self.assertRegex(name, r"^[A-Za-z][A-Za-z0-9_]{0,99}$")
            rows = list(matrix.values())
            installer = str(any(row["installerPayload"] == "true" for row in rows)).lower()
            self.assertIn(f"##vso[task.setvariable variable=buildInstaller;isOutput=true]{installer}", result.stdout)
        return result, rows

    def test_tag_push_builds_windows_desktop_and_cli(self):
        for reason in ("IndividualCI", "BatchedCI"):
            with self.subTest(reason=reason):
                result, rows = self.matrix("both", list(TARGETS), reason, "refs/tags/v1.2.3")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    [(row["application"], row["target"]) for row in rows],
                    [("cli", "x86_64-pc-windows-msvc"), ("desktop", "x86_64-pc-windows-msvc")],
                )
                self.assertTrue(all(row["installerPayload"] == "true" for row in rows))
                self.assertIn("##vso[task.setvariable variable=version;isOutput=true]1.2.3.0", result.stdout)

    def test_manual_runs_preserve_selection_and_linux_fallback(self):
        for ref in ("refs/heads/master", "refs/tags/v1.2.3"):
            for selected, targets in [([], [TARGETS["LINUX_GNU_X86_64"]]),
                                      (["WINDOWS_X86_64"], [TARGETS["WINDOWS_X86_64"]])]:
                with self.subTest(ref=ref, selected=selected):
                    result, rows = self.matrix("both", selected, ref=ref)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(
                        {(row["application"], row["target"]) for row in rows},
                        {(application, target) for application in ("cli", "desktop") for target in targets},
                    )
                    for row in rows:
                        installer = ref.startswith("refs/tags/") and row["target"] == "x86_64-pc-windows-msvc"
                        self.assertEqual(row["installerPayload"], str(installer).lower())

    def test_installer_payloads_require_a_tagged_windows_desktop_selection(self):
        for build, ref, selected in itertools.product(
            ("cli", "desktop", "both"), ("refs/heads/master", "refs/tags/v1.2.3"), ([], ["WINDOWS_X86_64"], ["WINDOWS_X86_64", "MACOS_X86_64"]),
        ):
            with self.subTest(build=build, ref=ref, selected=selected):
                result, rows = self.matrix(build, selected, ref=ref)
                self.assertEqual(result.returncode, 0, result.stderr)
                payloads = [row for row in rows if row["installerPayload"] == "true"]
                expected = {"cli", "desktop"} if ref.startswith("refs/tags/") and build != "cli" and selected else set()
                self.assertEqual({row["application"] for row in payloads}, expected)
                self.assertTrue(all(row["target"] == "x86_64-pc-windows-msvc" for row in payloads))

    def test_every_application_and_supported_platform_selection(self):
        supported = {key: target for key, target in TARGETS.items() if key != "MACOS_AARCH64"}
        for build in ("cli", "desktop", "both"):
            applications = ("cli",) if build == "cli" else ("cli", "desktop")
            for flags in itertools.product((False, True), repeat=len(supported)):
                selected = [key for key, enabled in zip(supported, flags) if enabled]
                with self.subTest(build=build, selected=selected):
                    targets = [supported[key] for key in selected] or [supported["LINUX_GNU_X86_64"]]
                    expected = {(application, target) for application in applications for target in targets
                                if (application != "desktop" or not target.endswith("-musl"))
                                and (build != "desktop" or not target.endswith("-musl"))}
                    result, rows = self.matrix(build, selected)
                    if not expected:
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn("Musl is CLI-only", result.stderr)
                        self.assertIsNone(rows)
                        continue
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual({(row["application"], row["target"]) for row in rows}, expected)
                    self.assertEqual(len(rows), len(expected))
                    for row in rows:
                        desktop = row["application"] == "desktop"
                        self.assertEqual(row["features"], "gui" if desktop else "")
                        self.assertEqual(row["fileName"], "regressify-desktop" if desktop else "regressify")
                        self.assertEqual(row["packageName"], "regressify-desktop" if desktop else "regressify-cli")
                        self.assertEqual(row["installerPayload"], "false")
                        if row["target"].endswith("-darwin"):
                            self.assertEqual(row["os"], "macos-15")
                        elif row["target"].endswith("-msvc"):
                            self.assertEqual(row["os"], "windows-latest")
                        else:
                            self.assertEqual(row["os"], "ubuntu-24.04" if desktop else "ubuntu-22.04")

    def test_invalid_selections_and_tags_fail_without_outputs(self):
        for build, ref, message in [
            ("invalid", "refs/heads/master", "Build must be"),
            ("both", "refs/tags/v1.2.3-beta", "Release tag must be"),
            ("both", "refs/tags/v1.10000.0", "Release tag must be"),
            ("both", "refs/tags/path/v1.2.3", "Release tag must be"),
        ]:
            with self.subTest(build=build, ref=ref):
                result, rows = self.matrix(build, [], ref=ref)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)
                self.assertIsNone(rows)
                self.assertNotIn("##vso[task.setvariable", result.stdout)
        result, rows = self.matrix("", [])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual({row["application"] for row in rows}, {"cli", "desktop"})
        result, _ = self.matrix("desktop", ["WINDOWS_X86_64"], ref="refs/tags/v01.02.03.4")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("##vso[task.setvariable variable=version;isOutput=true]1.2.3.4", result.stdout)

    def test_pipeline_scopes_secrets_and_consumes_plan_outputs(self):
        self.assertEqual(self.pipeline["trigger"], {"branches": {"exclude": ["*"]}, "tags": {"include": ["v*.*.*"]}})
        self.assertEqual(self.pipeline["pr"], "none")
        self.assertNotIn("variables", self.pipeline)
        self.assertNotIn("group", json.dumps(self.plan))
        self.assertNotIn("TUYEN_", json.dumps(self.plan))
        self.assertEqual(self.release["dependsOn"], "Plan")
        self.assertEqual(self.release["condition"], "and(succeeded(), ne(variables['Build.Reason'], 'PullRequest'))")
        self.assertEqual(self.release["lockBehavior"], "sequential")
        variables = self.release["variables"]
        self.assertEqual([variable["group"] for variable in variables if "group" in variable], ["GitHub", "Regressify"])
        version = next(variable["value"] for variable in variables if variable.get("name") == "releaseVersion")
        self.assertEqual(version, "$[ stageDependencies.Plan.SetMatrix.outputs['Matrix.version'] ]")
        installer = next(variable["value"] for variable in variables if variable.get("name") == "buildInstaller")
        self.assertEqual(installer, "$[ stageDependencies.Plan.SetMatrix.outputs['Matrix.buildInstaller'] ]")
        job = self.release["jobs"][0]
        self.assertEqual(job["strategy"]["matrix"], "$[ stageDependencies.Plan.SetMatrix.outputs['Matrix.matrix'] ]")
        self.assertNotIn("maxParallel", job["strategy"])
        self.assertEqual(job["pool"]["vmImage"], "$(os)")
        self.assertGreaterEqual(int(job["timeoutInMinutes"]), 90)
        parameters = {parameter["name"]: parameter for parameter in self.pipeline["parameters"]}
        self.assertNotIn("macosAarch64", parameters)
        self.assertEqual(parameters["build"]["values"], ["cli", "desktop", "both"])
        self.assertEqual(parameters["build"]["default"], "both")
        matrix = next(step for step in self.plan["jobs"][0]["steps"] if step.get("name") == "Matrix")
        for variable, parameter in [("MACOS_X86_64", "macosX86_64"), ("WINDOWS_X86_64", "windowsX86_64"),
                                    ("LINUX_GNU_X86_64", "linuxGnuX86_64"), ("LINUX_MUSL_X86_64", "linuxMuslX86_64"),
                                    ("LINUX_MUSL_AARCH64", "linuxMuslAarch64")]:
            self.assertEqual(matrix["env"][variable], "${{ parameters." + parameter + " }}")
            self.assertEqual(parameters[parameter]["type"], "boolean")
            self.assertEqual(parameters[parameter]["default"], str(parameter == "linuxGnuX86_64").lower())
        for step in self.steps:
            if step.get("task") == "Cache@2":
                self.assertIn('"$(application)"', step["inputs"]["key"])
                self.assertIn('"$(target)"', step["inputs"]["key"])
            if step.get("checkout") == "self":
                self.assertEqual(step["submodules"], "recursive")
        archives = next(step for step in self.steps if step.get("displayName") == "Build artifacts")
        self.assertEqual(archives["condition"], "and(succeeded(), ne(variables['installerPayload'], 'true'))")
        for name, value in {"FILE_NAME": "$(fileName)", "PACKAGE_NAME": "$(packageName)",
                            "CARGO_FEATURES": "$(features)", "NO_DEFAULT_FEATURES": "1",
                            "GITHUB_REF": "$(Build.SourceBranch)", "GITHUB_SHA": "$(Build.SourceVersion)",
                            "REPO": "$(TUYEN_GITHUB_ACTION_DEV_TOOLS_REPO)",
                            "TOKEN": "$(TUYEN_GITHUB_ACTION_PAT_TOKEN)"}.items():
            self.assertEqual(archives["env"][name], value)

    def test_windows_release_is_gated_and_retained_before_publishing(self):
        self.assertEqual(self.installer["dependsOn"], "Build")
        self.assertEqual(self.installer["condition"], "and(succeeded(), eq(variables['buildInstaller'], 'true'))")
        self.assertEqual(self.installer["pool"]["vmImage"], "windows-latest")
        names = ("Setup Tuyen CLI", "Check Tuyen Drive upload support",
                 "Build Windows installer", "Retain Windows installer", "Publish Windows installer to Tuyen Drive",
                 "Verify Windows installer publish")
        steps = {name: next(step for step in self.installer_steps if step.get("displayName") == name) for name in names}
        indexes = [self.installer_steps.index(steps[name]) for name in names]
        self.assertEqual(indexes, sorted(indexes))
        for step in steps.values():
            self.assertNotIn("condition", step)
        self.assertIn("https://cli.tuyen.blog/install.ps1", steps[names[0]]["pwsh"])
        self.assertIn("##vso[task.prependpath]", steps[names[0]]["pwsh"])
        self.assertIn("tuyen drive upload-app --help", steps[names[1]]["pwsh"])
        build = next(step for step in self.steps if step.get("displayName") == "Build Windows desktop release")
        self.assertEqual(build["condition"], "and(succeeded(), eq(variables['installerPayload'], 'true'), eq(variables['application'], 'desktop'))")
        self.assertIn("cargo build --locked --no-default-features --features gui --bin regressify-desktop", build["pwsh"])
        self.assertEqual(build["env"]["REGRESSIFY_DESKTOP_VERSION"], "$(releaseVersion)")
        self.assertEqual(build["env"]["TUYEN_DRIVE_APP"], "$(TUYEN_DRIVE_APP)")
        self.assertEqual(build["env"]["BINARY_GIT_HASH"], "$(Build.SourceVersion)")
        self.assertIn("+crt-static", build["env"]["RUSTFLAGS"])
        cli = next(step for step in self.steps if step.get("displayName") == "Build Windows CLI release")
        self.assertEqual(cli["condition"], "and(succeeded(), eq(variables['installerPayload'], 'true'), eq(variables['application'], 'cli'))")
        self.assertIn("cargo build --locked --no-default-features --bin regressify --release --target x86_64-pc-windows-msvc", cli["pwsh"])
        self.assertIn("if ($LASTEXITCODE -ne 0)", cli["pwsh"])
        self.assertEqual(cli["env"]["BINARY_GIT_HASH"], "$(Build.SourceVersion)")
        self.assertIn("+crt-static", cli["env"]["RUSTFLAGS"])
        self.assertNotIn("TUYEN_", json.dumps(cli))
        payload = next(step for step in self.steps if step.get("displayName") == "Retain installer payload")
        self.assertEqual(payload["condition"], "and(succeeded(), eq(variables['installerPayload'], 'true'))")
        self.assertEqual(payload["inputs"]["artifact"], "regressify-installer-$(application)")
        self.assertEqual(payload["inputs"]["targetPath"], "$(Build.SourcesDirectory)/target/x86_64-pc-windows-msvc/release/$(fileName).exe")
        downloads = [step for step in self.installer_steps if step.get("task") == "DownloadPipelineArtifact@2"]
        self.assertEqual({step["inputs"]["artifactName"] for step in downloads}, {"regressify-installer-cli", "regressify-installer-desktop"})
        for download in downloads:
            self.assertEqual(download["inputs"]["buildType"], "current")
            self.assertEqual(download["inputs"]["targetPath"], "$(Build.SourcesDirectory)/target/x86_64-pc-windows-msvc/release")
            self.assertLess(self.installer_steps.index(download), self.installer_steps.index(steps[names[2]]))
        self.assertIn("-RequireStaticRuntime -SmokeTest", steps[names[2]]["pwsh"])
        self.assertIn('$env:ISCC_PATH = $compiler', steps[names[2]]["pwsh"])
        self.assertIn('python ci/test-release.py InstallerTests', steps[names[2]]["pwsh"])
        self.assertEqual(steps[names[3]]["task"], "PublishPipelineArtifact@1")
        self.assertEqual(steps[names[3]]["inputs"]["targetPath"], "$(installer)")
        self.assertEqual(steps[names[4]]["env"]["TUYEN_DRIVE_TOKEN"], "$(TUYEN_DRIVE_TOKEN)")
        self.assertEqual(steps[names[4]]["env"]["INSTALLER_VERSION"], "$(releaseVersion)")
        verify = steps[names[5]]
        self.assertIn("ci/drive_release.py verify", verify["pwsh"])
        self.assertEqual(verify["env"]["GITHUB_REF_TYPE"], "tag")
        self.assertEqual(verify["env"]["GITHUB_REF_NAME"], "$(Build.SourceBranchName)")
        for step in self.steps + self.installer_steps:
            self.assertNotIn("continueOnError", step)
            self.assertNotIn("always()", step.get("condition", ""))
            if step is not steps[names[4]]:
                self.assertNotIn("TUYEN_DRIVE_TOKEN", step.get("env", {}))

    def test_powershell_steps_parse(self):
        shell = shutil.which("pwsh")
        self.assertIsNotNone(shell, "PowerShell 7 is required for Azure release checks")
        parser = '''
$ErrorActionPreference = 'Stop'
$scripts = [Console]::In.ReadToEnd() | ConvertFrom-Json
foreach ($script in $scripts) {
    $tokens = $null
    $parseErrors = $null
    [System.Management.Automation.Language.Parser]::ParseInput($script, [ref]$tokens, [ref]$parseErrors) | Out-Null
    if ($parseErrors) { throw ($parseErrors | Out-String) }
}
'''
        result = subprocess.run(
            [shell, "-NoProfile", "-NonInteractive", "-Command", parser],
            input=json.dumps([step["pwsh"] for step in self.steps + self.installer_steps if "pwsh" in step]),
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_msbuild_discovery_waits_for_exit_before_selecting_a_path(self):
        shell = shutil.which("pwsh")
        self.assertIsNotNone(shell, "PowerShell 7 is required for Azure release checks")
        step = next(step for step in self.steps
                    if step.get("displayName") == "Prepare build tools and cache directories")
        stub = r'''
$ErrorActionPreference = 'Stop'
function Invoke-VswhereFixture {
    if (($args -join ' ') -cne '-latest -products * -requires Microsoft.Component.MSBuild -find MSBuild/**/Bin/MSBuild.exe') {
        throw 'Incorrect MSBuild discovery arguments.'
    }
    foreach ($path in (ConvertFrom-Json -InputObject $env:MSBUILD_TEST_PATHS)) {
        Write-Output $path
    }
    $global:LASTEXITCODE = [int]$env:MSBUILD_TEST_EXIT
}
$workflow = [ScriptBlock]::Create([Console]::In.ReadToEnd())
$assignment = $workflow.Ast.Find({
    param($node)
    $node -is [System.Management.Automation.Language.AssignmentStatementAst] -and
        $node.Left.Extent.Text -eq '$locator'
}, $true)
if (-not $assignment) { throw 'MSBuild locator assignment was not found.' }
$statements = @($assignment.Parent.Statements)
$discovery = ($statements | Select-Object -Skip ([Array]::IndexOf($statements, $assignment) + 1)).Extent.Text -join "`n"
$locator = 'Invoke-VswhereFixture'
$global:LASTEXITCODE = 91
& ([ScriptBlock]::Create($discovery))
'''
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "Visual Studio" / "MSBuild" / "Current" / "Bin" / "MSBuild.exe"
            second = Path(directory) / "Build Tools" / "MSBuild" / "Current" / "Bin" / "MSBuild.exe"
            for paths, exit_code, succeeds in [([str(first)], 0, True), ([str(first), str(second)], 0, True),
                                               ([], 0, False), ([], 7, False), ([str(first)], 7, False)]:
                with self.subTest(paths=paths, exit_code=exit_code):
                    env = dict(os.environ, MSBUILD_TEST_PATHS=json.dumps(paths), MSBUILD_TEST_EXIT=str(exit_code))
                    result = subprocess.run(
                        [shell, "-NoProfile", "-NonInteractive", "-Command", stub],
                        input=step["pwsh"], env=env, capture_output=True, text=True, check=False, timeout=15,
                    )
                    self.assertEqual(result.returncode == 0, succeeds, result.stderr)
                    if succeeds:
                        self.assertIn(f"##vso[task.prependpath]{first.parent}", result.stdout)
                        self.assertNotIn(str(second.parent), result.stdout)
                    else:
                        self.assertNotIn("##vso[task.prependpath]", result.stdout)
                        message = f"vswhere exit code {exit_code}" if exit_code else "MSBuild was not found."
                        self.assertIn(message, result.stderr)

    def test_rust_environment_is_forwarded_only_after_success(self):
        shell = shutil.which("pwsh")
        self.assertIsNotNone(shell, "PowerShell 7 is required for Azure release checks")
        step = next(step for step in self.steps if step.get("displayName") == "Install Rust")
        stub = '''
$ErrorActionPreference = 'Stop'
function Invoke-RustFixture {
    if (($args -join ' ') -ne 'ci/install-rust.sh') { throw 'Incorrect Rust installer script.' }
    [System.IO.File]::WriteAllText($env:GITHUB_ENV, $env:RUST_TEST_ENV)
    $global:LASTEXITCODE = [int]$env:RUST_TEST_EXIT
}
'''
        settings = "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld\nCC=aarch64-linux-gnu-gcc\n"
        for content, exit_code, succeeds in [("", 0, True), (settings, 0, True), (settings, 7, False),
                                            ("UNEXPECTED=value", 0, False), ("CC", 0, False)]:
            with self.subTest(content=content, exit_code=exit_code), tempfile.TemporaryDirectory() as directory:
                env = dict(os.environ, AGENT_TEMPDIRECTORY=directory, BASH_PATH="Invoke-RustFixture",
                           RUST_TEST_ENV=content, RUST_TEST_EXIT=str(exit_code))
                result = subprocess.run([shell, "-NoProfile", "-NonInteractive", "-Command", stub + step["pwsh"]],
                                        env=env, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode == 0, succeeds, result.stderr)
                if succeeds and content:
                    for setting in content.splitlines():
                        name, value = setting.split("=", 1)
                        self.assertIn(f"##vso[task.setvariable variable={name}]{value}", result.stdout)
                else:
                    self.assertNotIn("##vso[task.setvariable", result.stdout)


class InstallerTests(unittest.TestCase):
    def test_payload_preflight_requires_both_files_and_matching_desktop_version(self):
        shell = shutil.which("pwsh")
        self.assertIsNotNone(shell, "PowerShell 7 is required for installer checks")
        script = r'''
$ErrorActionPreference = 'Stop'
function Get-Item([string]$LiteralPath) {
    $file = Microsoft.PowerShell.Management\Get-Item -LiteralPath $LiteralPath
    [pscustomobject]@{
        FullName = $file.FullName
        PSIsContainer = $file.PSIsContainer
        VersionInfo = @{ FileVersion = $env:PAYLOAD_TEST_VERSION }
    }
}
$source = [System.IO.File]::ReadAllText('ci/build-desktop-installer.ps1')
$parsed = [ScriptBlock]::Create($source)
$boundary = $parsed.Ast.EndBlock.Statements | Where-Object { $_.Extent.Text.StartsWith('if (-not $Compiler)') } | Select-Object -First 1
if (-not $boundary) { throw 'Installer preflight boundary was not found.' }
$preflight = Join-Path $env:PAYLOAD_TEST_DIRECTORY 'preflight.ps1'
[System.IO.File]::WriteAllText($preflight, $source.Substring(0, $boundary.Extent.StartOffset))
& $preflight -Version '1.2.3.0' -BinaryDirectory $env:PAYLOAD_TEST_DIRECTORY
'''
        for desktop, cli, version, succeeds in [
            ("file", "file", "1.2.3.0", True), ("file", "missing", "1.2.3.0", False),
            ("missing", "file", "1.2.3.0", False), ("file", "directory", "1.2.3.0", False),
            ("directory", "file", "1.2.3.0", False), ("file", "file", "1.2.4.0", False),
        ]:
            with self.subTest(desktop=desktop, cli=cli, version=version), tempfile.TemporaryDirectory() as directory:
                for name, kind in [("regressify-desktop.exe", desktop), ("regressify.exe", cli)]:
                    path = Path(directory) / name
                    if kind == "file":
                        path.write_bytes(b"synthetic payload")
                    elif kind == "directory":
                        path.mkdir()
                result = subprocess.run(
                    [shell, "-NoProfile", "-NonInteractive", "-Command", script], cwd=ROOT,
                    env=dict(os.environ, PAYLOAD_TEST_DIRECTORY=directory, PAYLOAD_TEST_VERSION=version),
                    capture_output=True, text=True, check=False, timeout=15,
                )
                self.assertEqual(result.returncode == 0, succeeds, result.stderr)

    def test_static_runtime_is_required_for_each_payload(self):
        shell = shutil.which("pwsh")
        self.assertIsNotNone(shell, "PowerShell 7 is required for installer checks")
        script = r'''
$ErrorActionPreference = 'Stop'
$binary = [pscustomobject]@{ Name = 'regressify-desktop.exe'; FullName = 'regressify-desktop.exe' }
$cliBinary = [pscustomobject]@{ Name = 'regressify.exe'; FullName = 'regressify.exe' }
function Invoke-DumpbinFixture {
    if ($args[0] -ne '/DEPENDENTS') { throw 'Incorrect dumpbin arguments.' }
    [Console]::WriteLine('Checked ' + $args[1])
    $global:LASTEXITCODE = 0
    if ($args[1] -eq $env:PAYLOAD_TEST_BAD_BINARY) {
        if ($env:PAYLOAD_TEST_FAILURE -eq 'exit') { $global:LASTEXITCODE = 7 }
        else { Write-Output 'VCRUNTIME140.dll' }
    } else { Write-Output 'KERNEL32.dll' }
}
$dumpbin = 'Invoke-DumpbinFixture'
$parsed = [ScriptBlock]::Create([System.IO.File]::ReadAllText('ci/build-desktop-installer.ps1'))
$loop = $parsed.Ast.Find({
    param($node)
    $node -is [System.Management.Automation.Language.ForEachStatementAst] -and $node.Variable.Extent.Text -eq '$payload'
}, $true)
if (-not $loop) { throw 'Installer payload dependency loop was not found.' }
& ([ScriptBlock]::Create($loop.Extent.Text))
'''
        for binary, failure in itertools.product(("", "regressify-desktop.exe", "regressify.exe"), ("exit", "runtime")):
            with self.subTest(binary=binary, failure=failure):
                result = subprocess.run(
                    [shell, "-NoProfile", "-NonInteractive", "-Command", script], cwd=ROOT,
                    env=dict(os.environ, PAYLOAD_TEST_BAD_BINARY=binary, PAYLOAD_TEST_FAILURE=failure),
                    capture_output=True, text=True, check=False, timeout=15,
                )
                self.assertEqual(result.returncode == 0, not binary, result.stderr)
                if not binary:
                    self.assertIn("Checked regressify-desktop.exe", result.stdout)
                    self.assertIn("Checked regressify.exe", result.stdout)
                else:
                    self.assertIn(binary, result.stderr)

    def test_installer_copies_and_smoke_tests_both_executables(self):
        installer = (ROOT / "installer/regressify-desktop.iss").read_text(encoding="utf-8")
        self.assertIn('Source: "{#SourceDir}\\regressify.exe"; DestDir: "{app}"; Flags: ignoreversion', installer)
        self.assertIn('Source: "{#SourceDir}\\regressify-desktop.exe"; DestDir: "{app}"; Flags: ignoreversion', installer)
        for name in ("LICENSE", "LICENSING.md"):
            self.assertIn(f'Source: "..\\{name}"; DestDir: "{{app}}"; Flags: ignoreversion', installer)
        smoke = (ROOT / "ci/build-desktop-installer.ps1").read_text(encoding="utf-8")
        self.assertIn("$licenseFiles = @('LICENSE', 'LICENSING.md')", smoke)
        self.assertIn('throw "Installer did not install the repository $name."', smoke)
        self.assertIn('throw "Uninstall left $name behind."', smoke)
        self.assertIn('Get-FileHash -LiteralPath $installedCli -Algorithm SHA256', smoke)
        self.assertIn('& $installedCli --version', smoke)
        self.assertIn("$installedCli = Join-Path $destination 'regressify.exe'", smoke)
        self.assertEqual(smoke.count('        Assert-InstallerCliPath\n'), 3)
        self.assertIn('where.exe regressify && regressify --version', smoke)
        self.assertIn('DoNotExpandEnvironmentNames', smoke)
        self.assertIn('Uninstall did not preserve the original user PATH data and type.', smoke)
        self.assertIn('Uninstall left the installed CLI behind.', smoke)

    def test_installer_builder_powershell_parses(self):
        script = r'''
$tokens = $null
$parseErrors = $null
[System.Management.Automation.Language.Parser]::ParseFile((Join-Path (Get-Location) 'ci/build-desktop-installer.ps1'), [ref]$tokens, [ref]$parseErrors) | Out-Null
if ($parseErrors) { throw ($parseErrors | Out-String) }
'''
        result = subprocess.run(
            [shutil.which("pwsh"), "-NoProfile", "-NonInteractive", "-Command", script],
            cwd=ROOT, capture_output=True, text=True, check=False, timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def installer_process_helper(self):
        return r'''
$ErrorActionPreference = 'Stop'
$parsed = [ScriptBlock]::Create([System.IO.File]::ReadAllText('ci/build-desktop-installer.ps1'))
$helper = $parsed.Ast.Find({
    param($node)
    $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Invoke-InstallerFixture'
}, $true)
if (-not $helper) { throw 'Installer process helper was not found.' }
. ([ScriptBlock]::Create($helper.Extent.Text))
'''

    @unittest.skipUnless(os.name == "nt", "Installer process-tree waits require Windows")
    def test_installer_process_wait_preserves_exits_and_cleans_up_timeouts(self):
        child_script = '''
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

mode = os.environ["INSTALLER_TEST_MODE"]
if mode == "second-phase":
    subprocess.Popen([sys.executable, __file__, *sys.argv[1:]],
                     env=dict(os.environ, INSTALLER_TEST_MODE="hang"),
                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    raise SystemExit(0)
Path(os.environ["INSTALLER_TEST_REPORT"]).write_text(json.dumps({
    "pid": os.getpid(), "arguments": sys.argv[1:],
    "proxy": {name: os.environ.get(name, "") for name in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY")},
}), encoding="utf-8")
if mode == "hang":
    threading.Event().wait()
raise SystemExit(7 if mode == "failure" else 0)
'''
        invoke = self.installer_process_helper() + r'''
$parameters = @(('"' + $env:INSTALLER_TEST_SCRIPT + '"'), '"two words"')
try {
    $code = Invoke-InstallerFixture $env:INSTALLER_TEST_PYTHON $parameters -TimeoutMilliseconds 3000
    if ($env:INSTALLER_TEST_MODE -in @('hang', 'second-phase')) { throw 'Expected a fixture timeout.' }
    Write-Output "Exit code: $code"
} catch {
    if ($env:INSTALLER_TEST_MODE -notin @('hang', 'second-phase') -or $_ -notmatch 'Installer fixture timed out') { throw }
    $report = Get-Content -LiteralPath $env:INSTALLER_TEST_REPORT -Raw | ConvertFrom-Json
    if (Get-Process -Id $report.pid -ErrorAction SilentlyContinue) { throw 'Timed-out installer process survived.' }
    Write-Output 'Timed out and cleaned up'
}
'''
        for mode in ("success", "failure", "hang", "second-phase"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory(prefix="installer process ") as directory:
                fixture = Path(directory)
                script = fixture / "child script.py"
                script.write_text(child_script, encoding="utf-8")
                report = fixture / "report.json"
                result = subprocess.run(
                    [shutil.which("pwsh"), "-NoProfile", "-NonInteractive", "-Command", invoke], cwd=ROOT,
                    env=dict(os.environ, INSTALLER_TEST_MODE=mode, INSTALLER_TEST_SCRIPT=str(script),
                             INSTALLER_TEST_REPORT=str(report), INSTALLER_TEST_PYTHON=sys.executable),
                    capture_output=True, text=True, check=False, timeout=30,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                expected = "Timed out and cleaned up" if mode in ("hang", "second-phase") else f"Exit code: {7 if mode == 'failure' else 0}"
                self.assertIn(expected, result.stdout)
                recorded = json.loads(report.read_text(encoding="utf-8"))
                self.assertEqual(recorded["arguments"], ["two words"])
                self.assertEqual(recorded["proxy"], {"HTTP_PROXY": "http://127.0.0.1:9", "HTTPS_PROXY": "http://127.0.0.1:9",
                                                     "ALL_PROXY": "http://127.0.0.1:9", "NO_PROXY": ""})

    @unittest.skipUnless(os.name == "nt" and os.environ.get("ISCC_PATH"), "Set ISCC_PATH on Windows for isolated Inno PATH tests")
    def test_inno_path_registration_preserves_user_data(self):
        source = (ROOT / "installer/regressify-desktop.iss").read_text(encoding="utf-8")
        self.assertIn("ChangesEnvironment=yes", source)
        self.assertIn("RegisterCliPath(ExpandConstant('{app}'));", source)
        self.assertIn("if CurUninstallStep = usPostUninstall then", source)
        self.assertIn("NotifyEnvironmentChange(BroadcastWindow, EnvironmentChangeMessage", source)
        path_code = source.split("[Code]\n", 1)[1].split("procedure ChromiumInstallLog", 1)[0]
        with tempfile.TemporaryDirectory(prefix="regressify-inno-") as directory:
            fixture = Path(directory)
            registry_root = "Software\\Regressify\\InstallerPathTests\\" + fixture.name
            for name, key in [("UserEnvironmentKey", "Environment"), ("CliPathOwnerKey", "Software\\Regressify\\DesktopInstaller")]:
                declaration = f"{name} = '{key}';"
                self.assertEqual(path_code.count(declaration), 1)
                path_code = path_code.replace(declaration, f"{name} = '{registry_root}\\{name}';")
            results = fixture / "results.txt"
            tests = r'''
procedure Check(Condition: Boolean; const Message: String);
begin
  if not Condition then
    RaiseException(Message);
end;

procedure CheckPath(const Expected: String; ExpectedType: Cardinal);
var
  Actual: String;
  ValueType: Cardinal;
begin
  Check(ReadUserPath(Actual, ValueType), 'Cannot read fixture PATH');
  Check(Actual = Expected, 'PATH entries were changed or duplicated');
  Check(ValueType = ExpectedType, 'PATH registry type ' + IntToStr(ValueType) + ' instead of ' + IntToStr(ExpectedType) + ', value length ' + IntToStr(Length(Expected)));
end;

procedure RunPathCases;
var
  Original, Directory, Alias, Current: String;
  Failed: Boolean;
  ValueType: Cardinal;
begin
  Directory := 'C:\Regressify Path Test';
  RegisterCliPath(Directory);
  CheckPath(Directory, 2);
  RemoveCliPath;
  CheckPath('', 0);
  Check(not RegKeyExists(HKCU, CliPathOwnerKey), 'Ownership was not removed');

  Check(RegWriteStringValue(HKCU, UserEnvironmentKey, 'Path', ''), 'Cannot seed empty PATH');
  RegisterCliPath(Directory);
  RemoveCliPath;
  CheckPath('', 1);

  Original := 'C:\tools;;%SystemRoot%\System32;C:\tools-extra;';
  Check(RegWriteExpandStringValue(HKCU, UserEnvironmentKey, 'Path', Original), 'Cannot seed PATH');
  RegisterCliPath(Directory);
  RegisterCliPath(Directory);
  CheckPath(Original + ';' + Directory, 2);
  Check(RegWriteExpandStringValue(HKCU, UserEnvironmentKey, 'Path', Original + ';' + Directory + ';C:\later'), 'Cannot append unrelated entry');
  RemoveCliPath;
  RemoveCliPath;
  CheckPath(Original + ';C:\later', 2);

  Check(RegDeleteValue(HKCU, UserEnvironmentKey, 'Path'), 'Cannot reset PATH type');
  Check(RegWriteStringValue(HKCU, UserEnvironmentKey, 'Path', Original), 'Cannot seed literal PATH');
  RegisterCliPath(Directory);
  CheckPath(Original + ';' + Directory, 1);
  RemoveCliPath;
  CheckPath(Original, 1);

  Original := '"' + Uppercase(Directory) + '\";C:\other';
  Check(RegWriteExpandStringValue(HKCU, UserEnvironmentKey, 'Path', Original), 'Cannot seed existing entry');
  RegisterCliPath(Directory);
  Check(not RegKeyExists(HKCU, CliPathOwnerKey), 'Pre-existing entry was claimed');
  RemoveCliPath;
  CheckPath(Original, 2);

  Alias := '%TEMP%\Regressify Path Test';
  Directory := AddBackslash(GetEnv('TEMP')) + 'Regressify Path Test';
  Check(RegWriteExpandStringValue(HKCU, UserEnvironmentKey, 'Path', Alias), 'Cannot seed environment alias');
  RegisterCliPath(Directory);
  RemoveCliPath;
  CheckPath(Alias, 2);

  Original := Directory + '-other';
  Check(RegWriteExpandStringValue(HKCU, UserEnvironmentKey, 'Path', Original), 'Cannot seed prefix entry');
  RegisterCliPath(Directory);
  CheckPath(Original + ';' + Directory, 2);
  RegisterCliPath('C:\new directory');
  CheckPath(Original + ';C:\new directory', 2);
  RemoveCliPath;
  CheckPath(Original, 2);

  Directory := 'C:\semicolon;directory';
  RegisterCliPath(Directory);
  CheckPath(Original + ';"' + Directory + '"', 2);
  RemoveCliPath;
  CheckPath(Original, 2);

  Check(RegWriteDWordValue(HKCU, UserEnvironmentKey, 'Path', 42), 'Cannot seed invalid PATH type');
  Failed := False;
  try
    RegisterCliPath(Directory);
  except
    Failed := True;
  end;
  Check(Failed, 'Invalid PATH type was overwritten');
  Check(RegQueryDWordValue(HKCU, UserEnvironmentKey, 'Path', ValueType) and (ValueType = 42), 'Invalid PATH data was lost');
  Check(not RegKeyExists(HKCU, CliPathOwnerKey), 'Failed registration claimed PATH');

  Original := StringOfChar('x', 32766);
  Check(RegWriteStringValue(HKCU, UserEnvironmentKey, 'Path', Original), 'Cannot seed long PATH');
  Failed := False;
  try
    RegisterCliPath(Directory);
  except
    Failed := True;
  end;
  Check(Failed, 'Oversized PATH was accepted');
  CheckPath(Original, 1);

  Current := 'C:\first;C:\owned;C:\owned;C:\last';
  Check(RemovePathEntry(Current, 'C:\owned', False), 'Owned entry was not found');
  Check(Current = 'C:\first;C:\owned;C:\last', 'Removal deleted unrelated duplicate entries');
  Current := 'C:\owned;C:\last';
  Check(RemovePathEntry(Current, 'C:\owned', False) and (Current = 'C:\last'), 'Leading entry removal failed');
end;
'''
            result_path = str(results).replace("'", "''")
            harness = fixture / "path-tests.iss"
            harness.write_text(
                "[Setup]\nAppName=Regressify PATH Tests\nAppVersion=1\nPrivilegesRequired=lowest\n"
                "CreateAppDir=no\nCreateUninstallRegKey=no\nUninstallable=no\nOutputBaseFilename=path-tests\n"
                f"OutputDir={fixture}\n[Code]\n" + path_code + tests + f'''
function InitializeSetup(): Boolean;
begin
  Result := False;
  try
    try
      RunPathCases;
      SaveStringToFile('{result_path}', 'ok', False);
    except
      SaveStringToFile('{result_path}', GetExceptionMessage, False);
    end;
  finally
    RegDeleteKeyIncludingSubkeys(HKCU, '{registry_root}');
  end;
end;
''', encoding="utf-8",
            )
            compiler = os.environ["ISCC_PATH"]
            for command in [
                [compiler, "/Qp", str(harness)],
                [str(fixture / "path-tests.exe"), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/SP-", "/NORESTART"],
            ]:
                result = subprocess.run(command, capture_output=True, text=True, check=False, timeout=90)
                if command[0] == compiler:
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertTrue(results.is_file(), "Inno PATH tests did not execute")
            self.assertEqual(results.read_text(encoding="utf-8-sig"), "ok")
            import winreg

            uninstall_code = "procedure CurUninstallStepChanged" + source.split("procedure CurUninstallStepChanged", 1)[1].split("function ShouldLaunch", 1)[0]
            lifecycle = fixture / "path-lifecycle.iss"
            destination = fixture / "installed"
            lifecycle.write_text(
                "[Setup]\nAppName=Regressify PATH Lifecycle Tests\nAppVersion=1\nPrivilegesRequired=lowest\n"
                "CreateUninstallRegKey=no\nArchitecturesAllowed=x64compatible\nArchitecturesInstallIn64BitMode=x64compatible\n"
                f"DefaultDirName={destination}\nOutputDir={fixture}\nOutputBaseFilename=path-lifecycle\n"
                "[Code]\n" + path_code + uninstall_code + """
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    RegisterCliPath(ExpandConstant('{app}'));
end;
""", encoding="utf-8",
            )
            uninstall_log = fixture / "uninstall.log"
            invoke = self.installer_process_helper() + r'''
$parameters = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/SP-', '/NORESTART')
if ($env:INSTALLER_TEST_LOG) { $parameters += '/LOG="' + $env:INSTALLER_TEST_LOG + '"' }
$result = Invoke-InstallerFixture $env:INSTALLER_TEST_EXECUTABLE $parameters
if ($result -eq 0 -and $env:INSTALLER_TEST_LOG) {
    if (Test-Path -LiteralPath ($env:INSTALLER_TEST_REGISTRY + '\CliPathOwnerKey')) {
        throw 'Uninstall helper returned before removing PATH ownership metadata.'
    }
}
exit $result
'''
            try:
                result = subprocess.run([compiler, "/Qp", str(lifecycle)], capture_output=True, text=True, check=False, timeout=90)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                for executable, log in [
                    (fixture / "path-lifecycle.exe", ""),
                    (destination / "unins000.exe", str(uninstall_log)),
                ]:
                    result = subprocess.run(
                        [shutil.which("pwsh"), "-NoProfile", "-NonInteractive", "-Command", invoke],
                        cwd=ROOT, capture_output=True, text=True, check=False, timeout=135,
                        env=dict(os.environ, INSTALLER_TEST_EXECUTABLE=str(executable), INSTALLER_TEST_LOG=log,
                                 INSTALLER_TEST_REGISTRY="HKCU:\\" + registry_root),
                    )
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                details = uninstall_log.read_text(encoding="utf-8-sig", errors="replace")
                with self.assertRaises(FileNotFoundError, msg="Uninstall left PATH ownership metadata behind.\n" + details):
                    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, registry_root + "\\CliPathOwnerKey"):
                        pass
                with winreg.OpenKey(winreg.HKEY_CURRENT_USER, registry_root + "\\UserEnvironmentKey") as environment:
                    with self.assertRaises(FileNotFoundError, msg="Uninstall left its PATH entry behind.\n" + details):
                        winreg.QueryValueEx(environment, "Path")
            finally:
                for key in ("\\CliPathOwnerKey", "\\UserEnvironmentKey", ""):
                    try:
                        winreg.DeleteKey(winreg.HKEY_CURRENT_USER, registry_root + key)
                    except FileNotFoundError:
                        pass
            for name in ("regressify.exe", "regressify-desktop.exe"):
                (fixture / name).write_bytes(b"MZ synthetic payload")
            result = subprocess.run(
                [compiler, "/Qp", str(ROOT / "installer/regressify-desktop.iss"), "/DAppVersion=1.2.3.0",
                f"/DSourceDir={fixture}", f"/DInstallerOutput={fixture}"],
                capture_output=True, text=True, check=False, timeout=90,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


class DriveReleaseTests(unittest.TestCase):
    def test_prepare_needs_only_the_ref_and_output_file(self):
        for ref_type, ref_name, version in [
            ("tag", "v0.2.0", "0.2.0.0"),
            ("branch", "master", ""),
            ("tag", "v0.2.0-beta", None),
        ]:
            with self.subTest(ref_type=ref_type, ref_name=ref_name), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "output"
                env = dict(os.environ, GITHUB_REF_TYPE=ref_type, GITHUB_REF_NAME=ref_name, GITHUB_OUTPUT=str(output))
                env.pop("TUYEN_DRIVE_APP", None)
                env.pop("TUYEN_DRIVE_TOKEN", None)
                result = subprocess.run(
                    [sys.executable, str(ROOT / "ci/drive_release.py"), "prepare"],
                    env=env, capture_output=True, text=True, check=False,
                )
                if version is None:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(output.exists())
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(output.read_text(encoding="utf-8"), f"version={version}\n")

    def test_version_requires_a_tag_and_normalizes_numeric_components(self):
        self.assertEqual(drive_release.tag_version("branch", "v1.2.3"), "")
        self.assertEqual(drive_release.tag_version("tag", "v1.2.3"), "1.2.3.0")
        self.assertEqual(drive_release.tag_version("tag", "v01.02.03.4"), "1.2.3.4")
        for tag in ("1.2.3", "v1.2", "v1.2.3-beta", "v1.10000.0", "v1.2.3.4.5", "v1.2.3\n"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                drive_release.tag_version("tag", tag)

    def manifest(self, file, version="1.2.3.0", digest=None):
        digest = digest or hashlib.sha256(file.read_bytes()).hexdigest()
        return (f"version={version}\ninstaller={drive_release.BASE_URL}/downloads/regressify/{file.name}\n"
                f"sha256={digest}\nsize={file.stat().st_size}\n").encode()

    def test_manifest_request_has_client_identity_and_no_credentials(self):
        body = b"error code: 1010\n"
        opener = mock.Mock()
        opener.open.side_effect = urllib.error.HTTPError(
            drive_release.BASE_URL, 403, "Forbidden", {}, io.BytesIO(body),
        )
        with mock.patch.object(drive_release.urllib.request, "build_opener", return_value=opener):
            self.assertEqual(drive_release.fetch_manifest("regressify"), (403, body))
        opener.open.assert_called_once()
        message = opener.open.call_args.args[0]
        self.assertEqual(message.get_method(), "GET")
        self.assertEqual(message.full_url, f"{drive_release.BASE_URL}/downloads/regressify/latest.txt")
        self.assertEqual(message.get_header("User-agent"), "Regressify-Release/1.0")
        self.assertEqual(message.get_header("Cache-control"), "no-cache")
        self.assertFalse(message.has_header("Authorization"))

    def test_verification_failure_preserves_server_reason_without_exposing_token(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / drive_release.installer_name("1.2.3.0")
            file.write_bytes(b"MZ synthetic installer")
            response = json.dumps({"error": "Upload denied for Bearer test-token"}).encode()
            with (
                mock.patch.object(drive_release, "fetch_manifest", return_value=(403, response)) as fetch,
                mock.patch.dict(os.environ, {"TUYEN_DRIVE_TOKEN": "test-token"}),
                mock.patch.object(drive_release.time, "sleep") as sleep,
                self.assertRaises(RuntimeError) as failure,
            ):
                drive_release.verify_publish("regressify", "1.2.3.0", file, "0" * 64)
            message = str(failure.exception)
            self.assertIn("HTTP 403", message)
            self.assertIn("Upload denied", message)
            self.assertNotIn("test-token", message)
            fetch.assert_called_once()
            sleep.assert_not_called()

    def test_verification_requires_matching_version_url_size_and_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / drive_release.installer_name("1.2.3.0")
            file.write_bytes(b"MZ synthetic installer")
            digest = hashlib.sha256(file.read_bytes()).hexdigest()
            for content in [self.manifest(file), json.dumps(drive_release.manifest_fields(self.manifest(file))).encode()]:
                with mock.patch.object(drive_release, "fetch_manifest", return_value=(200, content)):
                    drive_release.verify_publish("regressify", "1.2.3.0", file, digest)
            for invalid in (self.manifest(file, "1.2.4.0"), self.manifest(file, digest="0" * 64),
                            self.manifest(file).replace(b"size=22", b"size=23"),
                            self.manifest(file).replace(b"/regressify/", b"/another-app/")):
                with mock.patch.object(drive_release, "fetch_manifest", return_value=(200, invalid)), \
                     mock.patch.object(drive_release.time, "sleep"), self.assertRaises(RuntimeError):
                    drive_release.verify_publish("regressify", "1.2.3.0", file, digest)

    def test_verification_retries_transient_errors_with_a_bound(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / drive_release.installer_name("1.2.3.0")
            file.write_bytes(b"MZ synthetic installer")
            digest = hashlib.sha256(file.read_bytes()).hexdigest()
            with mock.patch.object(drive_release, "fetch_manifest", side_effect=[
                urllib.error.URLError("offline"), (503, b"Unavailable"), (404, b"Not found"), (200, self.manifest(file)),
            ]) as fetch, mock.patch.object(drive_release.time, "sleep"):
                drive_release.verify_publish("regressify", "1.2.3.0", file, digest)
                self.assertEqual(fetch.call_count, 4)
            with mock.patch.object(drive_release, "fetch_manifest", return_value=(404, b"Not found")) as fetch, \
                 mock.patch.object(drive_release.time, "sleep"), self.assertRaises(RuntimeError):
                drive_release.verify_publish("regressify", "1.2.3.0", file, digest)
            self.assertEqual(fetch.call_count, 5)

    def test_branch_verification_is_rejected_before_network_access(self):
        env = dict(os.environ, GITHUB_REF_TYPE="branch", GITHUB_REF_NAME="master")
        result = subprocess.run([sys.executable, str(ROOT / "ci/drive_release.py"), "verify", "--installer", "missing.exe"],
                                env=env, capture_output=True, text=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Release verification requires a version tag", result.stderr)


if __name__ == "__main__":
    unittest.main()