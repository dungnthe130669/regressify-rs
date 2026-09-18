"""Exercise the release workflow and Cargo arguments without building or publishing."""

import itertools
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
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
        for name in ("Build Windows desktop release", "Build Windows installer", "Retain Windows installer", "Publish Windows installer to Tuyen Drive"):
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
        installer = (ROOT / "installer/regressify-desktop.iss").read_text(encoding="utf-8")
        updater = (ROOT / "src/desktop/updates.rs").read_text(encoding="utf-8")
        for setting in ("AppMutex", "SetupMutex"):
            mutex = next(line.split("=", 1)[1] for line in installer.splitlines() if line.startswith(setting + "="))
            self.assertIn(mutex.replace("\\", "\\\\"), updater)
        self.assertIn("PrivilegesRequired=lowest", installer)
        self.assertIn("CloseApplications=no", installer)
        self.assertIn("if CheckForMutexes(", installer)
        self.assertNotIn("[UninstallDelete]", installer)

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

    def test_one_shot_fields_and_an_idempotent_conflict_require_matching_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / drive_release.installer_name("1.2.3.0")
            file.write_bytes(b"MZ synthetic installer")
            calls = []

            def request(method, path, headers=None, body_factory=None, length=None):
                calls.append((method, path))
                if method == "GET":
                    self.assertNotIn("Authorization", headers)
                    return 200, self.manifest(file)
                body = b"".join(body_factory())
                self.assertEqual(len(body), length)
                self.assertEqual(headers["Authorization"], "Bearer test-token")
                self.assertIn(f'name="role_{file.name}"\r\n\r\ninstaller'.encode(), body)
                self.assertIn(hashlib.sha256(file.read_bytes()).hexdigest().encode(), body)
                self.assertIn(file.read_bytes(), body)
                self.assertNotIn(b"latest.txt", body)
                return 409, b'{"error":"Version already published"}'

            with mock.patch.object(drive_release, "request", side_effect=request):
                drive_release.publish(file, "1.2.3.0", "regressify", "test-token")
            self.assertEqual([call[0] for call in calls], ["POST", "GET"])
            for invalid in (self.manifest(file, "1.2.4.0"), self.manifest(file, digest="0" * 64)):
                with mock.patch.object(drive_release, "request", return_value=(200, invalid)), \
                     mock.patch.object(drive_release.time, "sleep"), self.assertRaises(RuntimeError):
                    drive_release.verify_publish("regressify", "1.2.3.0", file, hashlib.sha256(file.read_bytes()).hexdigest())

    def test_multipart_obeys_part_size_and_completes_with_returned_etags(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / drive_release.installer_name("1.2.3.0")
            payload = b"x" * (5 * 1024 * 1024) + b"tail"
            file.write_bytes(payload)
            chunks = []

            def request(method, path, headers=None, body_factory=None, length=None):
                if method == "GET":
                    return 200, self.manifest(file)
                body = body_factory()
                self.assertEqual(len(body), length)
                if path.endswith("/init"):
                    asset = json.loads(body)["assets"][0]
                    self.assertEqual(asset["role"], "installer")
                    self.assertEqual(asset["size"], len(payload))
                    return 200, json.dumps({"version_id": "version", "part_size": 5 * 1024 * 1024,
                                            "uploads": [{"name": file.name, "asset_id": "asset"}]}).encode()
                if method == "PUT":
                    chunks.append(body)
                    return 200, json.dumps({"partNumber": len(chunks), "etag": f"etag-{len(chunks)}"}).encode()
                self.assertTrue(path.endswith("/complete"))
                self.assertEqual(json.loads(body)["assets"], [{"asset_id": "asset", "parts": [
                    {"partNumber": 1, "etag": "etag-1"}, {"partNumber": 2, "etag": "etag-2"}]}])
                return 200, b"{}"

            with mock.patch.object(drive_release, "request", side_effect=request), \
                 mock.patch.object(drive_release, "one_shot", side_effect=drive_release.multipart):
                drive_release.publish(file, "1.2.3.0", "regressify", "test-token")
            self.assertEqual(b"".join(chunks), payload)
            self.assertEqual([len(chunk) for chunk in chunks], [5 * 1024 * 1024, 4])

    def test_branch_publish_is_rejected_before_network_access(self):
        env = dict(os.environ, GITHUB_REF_TYPE="branch", GITHUB_REF_NAME="master")
        result = subprocess.run([sys.executable, str(ROOT / "ci/drive_release.py"), "publish", "--installer", "missing.exe"],
                                env=env, capture_output=True, text=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Publishing requires a version tag", result.stderr)


if __name__ == "__main__":
    unittest.main()