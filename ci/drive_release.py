"""Resolve release tags and verify installers published by Tuyen CLI."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import time
import urllib.error
import urllib.request


BASE_URL = "https://drive.tuyen.blog"
MAX_INSTALLER_SIZE = 1024 * 1024 * 1024


def version_parts(value):
    if not re.fullmatch(r"[0-9]{1,4}(?:\.[0-9]{1,4}){0,3}", value):
        raise ValueError("Version must have one to four numeric components, each 0-9999")
    parts = tuple(int(part) for part in value.split("."))
    return parts + (0,) * (4 - len(parts))


def tag_version(ref_type, ref_name):
    if ref_type != "tag":
        return ""
    if not re.fullmatch(r"v[0-9]{1,4}(?:\.[0-9]{1,4}){2,3}", ref_name):
        raise ValueError("Release tag must be vN.N.N or vN.N.N.N, each component 0-9999")
    return ".".join(map(str, version_parts(ref_name[1:])))


def validate_app(app):
    if not re.fullmatch(r"[a-z0-9_-]{1,64}", app):
        raise ValueError("TUYEN_DRIVE_APP must contain the app slug")
    return app


def installer_name(version):
    return f"Regressify-Desktop-Setup-{version}-x64.exe"


def response_error(operation, status, content):
    detail = content.decode("utf-8", errors="replace")
    try:
        payload = json.loads(detail)
        if isinstance(payload, dict) and isinstance(payload.get("error"), str):
            detail = payload["error"]
    except ValueError:
        pass
    secret = os.environ.get("TUYEN_DRIVE_TOKEN", "")
    if secret:
        detail = detail.replace(secret, "[redacted]")
    detail = " ".join("".join(character if character.isprintable() or character.isspace() else " "
                              for character in detail).split())
    if len(detail) > 1000:
        detail = detail[:1000] + "..."
    return RuntimeError(f"{operation} (HTTP {status}): {detail or 'Empty response body'}")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        return None


def fetch_manifest(app):
    opener = urllib.request.build_opener(NoRedirect())
    message = urllib.request.Request(
        f"{BASE_URL}/downloads/{app}/latest.txt",
        headers={"Cache-Control": "no-cache", "User-Agent": "Regressify-Release/1.0"},
    )
    try:
        response = opener.open(message, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        return response.status, response.read(16 * 1024 + 1)


def manifest_fields(content):
    text = content.decode("utf-8-sig").strip()
    if len(content) > 16 * 1024:
        raise ValueError("Manifest exceeds 16 KiB")
    if text.startswith("{"):
        return json.loads(text)
    fields = {}
    for line in text.splitlines():
        if not line.strip():
            continue
        key, value = line.split("=", 1)
        key, value = key.strip(), value.strip()
        if key in fields:
            raise ValueError("Duplicate manifest field")
        fields[key] = value
    return fields


def verify_publish(app, version, file, digest):
    expected_url = f"{BASE_URL}/downloads/{app}/{file.name}"
    failure = "Published manifest does not match this installer version, URL, size and SHA-256"
    for attempt in range(5):
        try:
            status, content = fetch_manifest(app)
        except (urllib.error.URLError, TimeoutError, OSError):
            failure = "Cannot reach Tuyen Drive to verify the published manifest"
        else:
            if status == 200:
                fields = manifest_fields(content)
                if (version_parts(str(fields.get("version", ""))) == version_parts(version)
                        and fields.get("installer") == expected_url
                        and str(fields.get("sha256", "")).lower() == digest
                        and str(fields.get("size")) == str(file.stat().st_size)):
                    return
                failure = "Published manifest does not match this installer version, URL, size and SHA-256"
            else:
                error = response_error("Cannot verify published manifest", status, content)
                if status not in (404, 429, 500, 502, 503, 504):
                    raise error
                failure = str(error)
        if attempt < 4:
            time.sleep(2 ** attempt)
    raise RuntimeError(failure)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "verify"))
    parser.add_argument("--installer", type=Path)
    args = parser.parse_args()
    version = tag_version(os.environ.get("GITHUB_REF_TYPE", ""), os.environ.get("GITHUB_REF_NAME", ""))
    if args.command == "prepare":
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(f"version={version}\n")
        return
    if not version:
        raise ValueError("Release verification requires a version tag")
    if args.installer is None:
        raise ValueError("--installer is required to verify")
    if args.installer.name != installer_name(version):
        raise ValueError("Installer filename must contain the canonical release version")
    if not 0 < args.installer.stat().st_size <= MAX_INSTALLER_SIZE:
        raise ValueError("Installer size must be between 1 byte and 1 GiB")
    app = validate_app(os.environ.get("TUYEN_DRIVE_APP", ""))
    with args.installer.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    verify_publish(app, version, args.installer, digest)
    print("Verified published installer version, URL, size and SHA-256")


if __name__ == "__main__":
    main()