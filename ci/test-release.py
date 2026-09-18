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
                record = output.read_text(encoding="utf-8").strip()
                self.assertTrue(record.startswith("matrix="))
                rows = json.loads(record.removeprefix("matrix="))["include"]
            return result, rows

    def test_tag_push_builds_windows_desktop(self):
        result, rows = self.matrix("", [], event="push", ref_type="tag")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            [(row["application"], row["target"]) for row in rows],
            [("desktop", "x86_64-pc-windows-msvc")],
        )
        self.assertNotIn("regressify", json.dumps(rows))

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
        steps = WORKFLOW["jobs"]["release"]["steps"]
        for name in ("Setup Tuyen CLI", "Check Tuyen Drive upload support", "Build Windows desktop release", "Build Windows installer", "Retain Windows installer", "Publish Windows installer to Tuyen Drive", "Verify Windows installer publish"):
            step = next(step for step in steps if step.get("name") == name)
            self.assertIn("github.ref_type == 'tag'", step["if"])
            self.assertIn("matrix.application == 'desktop'", step["if"])
            self.assertIn("matrix.target == 'x86_64-pc-windows-msvc'", step["if"])
        build = next(step for step in steps if step.get("name") == "Build Windows desktop release")
        self.assertEqual(build["env"]["REGRESSIFY_DESKTOP_VERSION"], "${{ needs.set-matrix.outputs.version }}")
        self.assertEqual(build["env"]["TUYEN_DRIVE_APP"], "${{ secrets.TUYEN_DRIVE_APP }}")
        self.assertNotIn("TUYEN_DRIVE_TOKEN", build["env"])
        self.assertIn("+crt-static", build["env"]["RUSTFLAGS"])
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
        self.assertLess(steps.index(support), steps.index(build))
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

    def test_tuyen_upload_arguments_and_failures(self):
        shell = shutil.which("pwsh")
        self.assertIsNotNone(shell, "PowerShell 7 is required for the Windows publish-step test")
        step = next(step for step in WORKFLOW["jobs"]["release"]["steps"]
                    if step.get("name") == "Publish Windows installer to Tuyen Drive")
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
            for app, token, version, cli_exit, invoked, succeeds in [
                ("regressify", "test-token", "1.2.3.0", 0, True, True),
                ("regressify", "test-token", "1.2.3.0", 7, True, False),
                ("", "test-token", "1.2.3.0", 0, False, False),
                ("regressify", "", "1.2.3.0", 0, False, False),
                ("regressify", "test-token", "1.2.4.0", 0, False, False),
            ]:
                with self.subTest(app=app, has_token=bool(token), version=version, cli_exit=cli_exit):
                    arguments.unlink(missing_ok=True)
                    env = dict(os.environ, TUYEN_DRIVE_APP=app, TUYEN_DRIVE_TOKEN=token,
                               INSTALLER=str(file), INSTALLER_VERSION=version,
                               TUYEN_ARGUMENTS_FILE=str(arguments), TUYEN_TEST_EXIT=str(cli_exit))
                    result = subprocess.run([shell, "-NoProfile", "-NonInteractive", "-Command", stub + step["run"]],
                                            env=env, capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode == 0, succeeds, result.stderr)
                    self.assertEqual(arguments.exists(), invoked)
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
        self.assertEqual(build["env"]["FILE_NAME"], "${{ matrix.application == 'desktop' && 'regressify-desktop' || 'regressify' }}")
        self.assertEqual(build["env"]["PACKAGE_NAME"], "${{ matrix.application == 'desktop' && 'regressify-desktop' || 'regressify-cli' }}")
        self.assertEqual(build["env"]["CARGO_FEATURES"], "${{ matrix.features }}")
        self.assertEqual(build["env"]["NO_DEFAULT_FEATURES"], "1")
        cache = next(step for step in job["steps"] if step.get("uses", "").startswith("actions/cache@"))
        self.assertIn("matrix.application", cache["with"]["key"])

    def test_every_application_and_platform_selection(self):
        for build in ("cli", "desktop", "both"):
            applications = ("cli", "desktop") if build == "both" else (build,)
            for flags in itertools.product((False, True), repeat=len(TARGETS)):
                selected = [key for key, enabled in zip(TARGETS, flags) if enabled]
                with self.subTest(build=build, selected=selected):
                    targets = [TARGETS[key] for key in selected] or [TARGETS["LINUX_GNU_X86_64"]]
                    expected = {
                        (application, target)
                        for application in applications for target in targets
                        if application != "desktop" or not target.endswith("-musl")
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