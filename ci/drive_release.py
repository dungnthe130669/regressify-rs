"""Tag-gated Windows desktop publishing to the Tuyen Drive downloads API."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


BASE_URL = "https://drive.tuyen.blog"
API = "/api/downloads/versions"
ONE_SHOT_LIMIT = 90 * 1024 * 1024
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


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        return None


def request(method, path, headers=None, body_factory=None, length=None):
    headers = dict(headers or {})
    if length is not None:
        headers["Content-Length"] = str(length)
    opener = urllib.request.build_opener(NoRedirect())
    for attempt in range(4):
        body = body_factory() if body_factory else None
        try:
            message = urllib.request.Request(BASE_URL + path, data=body, headers=headers, method=method)
            try:
                with opener.open(message, timeout=300) as response:
                    status, content = response.status, response.read(1024 * 1024 + 1)
            except urllib.error.HTTPError as error:
                with error:
                    status, content = error.code, error.read(1024 * 1024 + 1)
            if len(content) > 1024 * 1024:
                raise RuntimeError("Tuyen Drive response exceeded 1 MiB")
            if status not in (429, 500, 502, 503, 504) or attempt == 3:
                return status, content
        except (urllib.error.URLError, TimeoutError, ConnectionError, OSError) as error:
            if attempt == 3:
                raise RuntimeError("Cannot reach Tuyen Drive after four attempts") from error
        finally:
            if hasattr(body, "close"):
                body.close()
        time.sleep(2 ** attempt)
    raise RuntimeError("Tuyen Drive request failed")


def json_request(method, path, token, body):
    payload = json.dumps(body, separators=(",", ":")).encode()
    return request(method, path, {
        "Authorization": f"Bearer {token}", "Content-Type": "application/json",
    }, lambda: payload, len(payload))


def one_shot(file, version, digest, token):
    boundary = "regressify-" + uuid.uuid4().hex
    fields = {"version": version, f"role_{file.name}": "installer", f"sha256_{file.name}": digest}
    prefix = b"".join(
        f'--{boundary}\r\nContent-Disposition: form-data; name="{key}"\r\n\r\n{value}\r\n'.encode()
        for key, value in fields.items()
    )
    prefix += (
        f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{file.name}"\r\n'
        "Content-Type: application/octet-stream\r\n\r\n"
    ).encode()
    suffix = f"\r\n--{boundary}--\r\n".encode()

    def body():
        yield prefix
        with file.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                yield chunk
        yield suffix

    return request("POST", API, {
        "Authorization": f"Bearer {token}", "Content-Type": f"multipart/form-data; boundary={boundary}",
    }, body, len(prefix) + file.stat().st_size + len(suffix))


def multipart(file, version, digest, token):
    status, content = json_request("POST", API + "/init", token, {
        "version": version,
        "assets": [{"name": file.name, "size": file.stat().st_size, "sha256": digest,
                    "content_type": "application/octet-stream", "role": "installer"}],
    })
    if status == 409:
        return status, content
    if status != 200:
        raise RuntimeError(f"Multipart initialization failed (HTTP {status})")
    initialized = json.loads(content)
    version_id = initialized.get("version_id")
    part_size = initialized.get("part_size")
    matches = [upload for upload in initialized.get("uploads", []) if upload.get("name") == file.name]
    asset_id = matches[0].get("asset_id") if len(matches) == 1 else None
    if (not isinstance(version_id, str) or not version_id or not isinstance(asset_id, str) or not asset_id
            or type(part_size) is not int or not 5 * 1024 * 1024 <= part_size <= ONE_SHOT_LIMIT):
        raise RuntimeError("Invalid multipart upload identifiers or part size")
    version_path = API + "/" + urllib.parse.quote(version_id, safe="")
    asset_path = version_path + "/assets/" + urllib.parse.quote(asset_id, safe="")
    parts = []
    with file.open("rb") as stream:
        while chunk := stream.read(part_size):
            part_number = len(parts) + 1
            if part_number > 10000:
                raise RuntimeError("Installer exceeds the multipart part limit")
            status, content = request("PUT", f"{asset_path}/parts/{part_number}", {
                "Authorization": f"Bearer {token}", "Content-Type": "application/octet-stream",
            }, lambda: chunk, len(chunk))
            if status != 200:
                raise RuntimeError(f"Multipart part {part_number} failed (HTTP {status})")
            part = json.loads(content)
            if part.get("partNumber") != part_number or not isinstance(part.get("etag"), str) or not part["etag"]:
                raise RuntimeError("Invalid multipart part acknowledgement")
            parts.append({"partNumber": part_number, "etag": part["etag"]})
            print(f"Uploaded installer part {part_number}")
    return json_request("POST", version_path + "/complete", token, {
        "assets": [{"asset_id": asset_id, "parts": parts}],
    })


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
    path = f"/downloads/{app}/latest.txt"
    expected_url = f"{BASE_URL}/downloads/{app}/{file.name}"
    for attempt in range(5):
        status, content = request("GET", path, {"Cache-Control": "no-cache"})
        if status == 200:
            fields = manifest_fields(content)
            if (version_parts(str(fields.get("version", ""))) == version_parts(version)
                    and fields.get("installer") == expected_url
                    and str(fields.get("sha256", "")).lower() == digest
                    and str(fields.get("size")) == str(file.stat().st_size)):
                return
        elif status != 404:
            raise RuntimeError(f"Cannot verify published manifest (HTTP {status})")
        if attempt < 4:
            time.sleep(2 ** attempt)
    raise RuntimeError("Published manifest does not match this installer version, URL, size and SHA-256")


def publish(file, version, app, token):
    validate_app(app)
    if not token.strip():
        raise ValueError("TUYEN_DRIVE_TOKEN is required to publish")
    if file.name != installer_name(version):
        raise ValueError("Installer filename must contain the canonical release version")
    size = file.stat().st_size
    if not 0 < size <= MAX_INSTALLER_SIZE:
        raise ValueError("Installer size must be between 1 byte and 1 GiB")
    with file.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    upload = one_shot if size <= ONE_SHOT_LIMIT else multipart
    status, _ = upload(file, version, digest, token)
    if status not in (200, 409):
        raise RuntimeError(f"Publishing failed (HTTP {status})")
    verify_publish(app, version, file, digest)
    print(f"Verified published installer {file.name} ({size} bytes, SHA-256 {digest})")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "publish"))
    parser.add_argument("--installer", type=Path)
    args = parser.parse_args()
    version = tag_version(os.environ.get("GITHUB_REF_TYPE", ""), os.environ.get("GITHUB_REF_NAME", ""))
    if args.command == "prepare":
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(f"version={version}\n")
        return
    if not version:
        raise ValueError("Publishing requires a version tag; branch builds cannot upload")
    if args.installer is None:
        raise ValueError("--installer is required to publish")
    publish(args.installer, version, os.environ.get("TUYEN_DRIVE_APP", ""), os.environ.get("TUYEN_DRIVE_TOKEN", ""))


if __name__ == "__main__":
    main()