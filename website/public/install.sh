#!/bin/sh
# Install the Regressify CLI and desktop app (where supported) on macOS and Linux.
# On macOS the desktop app is also installed as Regressify.app in /Applications
# (or ~/Applications when /Applications is not writable).
# Usage: curl -fsSL https://regressify.eshn.dev/install.sh | sh
regressify_install() (
set -eu

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

origin="${REGRESSIFY_ORIGIN:-https://regressify.eshn.dev}"
case "$origin" in
  https://*[!a-z0-9.-]*|https://|https://*/*)
    fail "Invalid REGRESSIFY_ORIGIN '$origin'; expected https://host without a path"
    ;;
  https://*) ;;
  *)
    fail "Invalid REGRESSIFY_ORIGIN '$origin'; expected https://host without a path"
    ;;
esac

os_name="${REGRESSIFY_OS:-$(uname -s)}"
arch_name="${REGRESSIFY_ARCH:-$(uname -m)}"
install_desktop=1

case "$os_name" in
  Darwin)
    case "$arch_name" in
      arm64|aarch64) target="aarch64-apple-darwin" ;;
      x86_64|amd64) target="x86_64-apple-darwin" ;;
      *) fail "Unsupported macOS architecture '$arch_name'" ;;
    esac
    ;;
  Linux)
    case "$arch_name" in
      x86_64|amd64)
        libc="${REGRESSIFY_LIBC:-}"
        if [ -z "$libc" ]; then
          if command -v ldd >/dev/null 2>&1; then
            libc_version="$(ldd --version 2>&1 || true)"
            case "$libc_version" in
              *musl*) libc="musl" ;;
              *GLIBC*|*glibc*|*'GNU libc'*|*'Free Software Foundation'*) libc="gnu" ;;
            esac
          fi
          if [ -z "$libc" ] && { [ -e /lib64/ld-linux-x86-64.so.2 ] || [ -e /lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 ]; }; then
            libc="gnu"
          fi
        fi
        [ "$libc" = "gnu" ] || fail "Prebuilt Linux releases require x86_64 glibc; Musl distributions are not supported"
        target="x86_64-unknown-linux-gnu"
        ;;
      aarch64|arm64)
        fail "Prebuilt Linux ARM64 releases are not supported; managed Chromium is unavailable for Linux ARM64. Use Linux x86_64 with glibc"
        ;;
      *) fail "Unsupported Linux architecture '$arch_name'" ;;
    esac
    ;;
  *)
    fail "Unsupported operating system '$os_name'. On Windows, run: irm https://regressify.eshn.dev/install.ps1 | iex"
    ;;
esac

if [ "${REGRESSIFY_CLI_ONLY:-0}" = "1" ]; then
  install_desktop=0
fi

command -v unzip >/dev/null 2>&1 || fail "'unzip' is required to extract Regressify archives"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print tolower($1)}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print tolower($1)}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$1" | awk '{print tolower($NF)}'
  else
    fail "sha256sum, shasum, or openssl is required to verify downloads"
  fi
}

fetch_file() {
  url="$1"
  dest="$2"
  limit="$3"
  curl -q -fsSL --proto '=https' --proto-redir '=https' --max-redirs 5 \
    --connect-timeout 10 --max-time 900 --max-filesize "$limit" "$url" -o "$dest"
}

command -v curl >/dev/null 2>&1 || fail "curl is required to download Regressify"
tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t regressify-install)"
stage_dir=""
backup_app=""
backup_dest=""
lock_dir=""
cleanup() {
  rm -rf "$tmp_dir"
  keep_stage=0
  # A replaced app that was never swapped back in is restored before cleanup;
  # if that fails the backup is kept and its location is reported.
  if [ -n "$backup_app" ] && [ -e "$backup_app" ] && [ ! -e "$backup_dest" ]; then
    if ! mv "$backup_app" "$backup_dest"; then
      printf 'warning: could not restore the previous app; it is kept at %s\n' "$backup_app" >&2
      keep_stage=1
    fi
  fi
  if [ -n "$stage_dir" ] && [ "$keep_stage" = "0" ]; then
    rm -rf "$stage_dir"
  fi
  if [ -n "$lock_dir" ]; then
    rmdir "$lock_dir" 2>/dev/null || true
  fi
}
trap cleanup 0
trap 'exit 1' INT TERM

manifest_file="$tmp_dir/latest.txt"
if [ -n "${REGRESSIFY_MANIFEST_FILE:-}" ]; then
  cp "$REGRESSIFY_MANIFEST_FILE" "$manifest_file"
else
  printf 'Fetching release manifest from %s/downloads/latest.txt...\n' "$origin"
  curl -q -fsS --proto '=https' --max-redirs 0 --connect-timeout 10 --max-time 30 \
    --max-filesize 16384 -H 'Cache-Control: no-cache' "$origin/downloads/latest.txt" -o "$manifest_file"
fi

[ "$(wc -c < "$manifest_file")" -le 16384 ] || fail "Release manifest exceeds 16 KiB"
tr -d '\r' < "$manifest_file" > "$tmp_dir/manifest.clean"
mv "$tmp_dir/manifest.clean" "$manifest_file"
awk -F= '
  function version(value, required, count, parts, result, component) {
    count = split(value, parts, ".")
    if (count < required || count > 4) return ""
    result = ""
    for (component = 1; component <= 4; component++) {
      if (component <= count && (parts[component] !~ /^[0-9]+$/ || length(parts[component]) > 4)) return ""
      result = result (component == 1 ? "" : ".") (parts[component] + 0)
    }
    return result
  }
  NF {
    if (NF != 2 || $1 !~ /^[a-zA-Z0-9_.-]+$/ || seen[$1]++) exit 1
    fields[$1] = $2
  }
  END {
    normalized = version(fields["version"], 4)
    if (normalized == "" || normalized != fields["version"] || fields["tag"] !~ /^v/ ||
        version(substr(fields["tag"], 2), 3) != normalized ||
        fields["sha256"] !~ /^[0-9a-fA-F]+$/ || length(fields["sha256"]) != 64 ||
        fields["size"] !~ /^[0-9]+$/ || fields["size"] + 0 < 1 || fields["size"] + 0 > 1073741824 ||
        fields["installer"] != "https://github.com/precise-alloy/regressify-rs/releases/download/" fields["tag"] "/Regressify-Setup-" normalized "-x64.exe") exit 1
  }
' "$manifest_file" || fail "Invalid or ambiguous release manifest"

read_field() {
  key="$1"
  awk -F'=' -v k="$key" '$1 == k { sub(/^[^=]*=/, ""); print; exit }' "$manifest_file"
}

version="$(read_field version)"
tag="$(read_field tag)"
installer_url="$(read_field installer)"
[ -n "$version" ] || fail "Release manifest is missing version"
[ -n "$installer_url" ] || fail "Release manifest is missing installer URL"

if [ -z "$tag" ]; then
  tag="$(printf '%s' "$installer_url" | sed -n 's|^https://github\.com/precise-alloy/regressify-rs/releases/download/\(v[0-9.]*\)/.*$|\1|p')"
fi
case "$tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) fail "Invalid release tag '$tag' in manifest" ;;
esac

download_and_extract() {
  archive_name="$1"
  binary_name="$2"
  meta="$(read_field "archive.$archive_name")"
  [ -n "$meta" ] || fail "Release manifest does not list '$archive_name'"
  expected_size="${meta%%:*}"
  expected_hash="$(printf '%s' "${meta#*:}" | tr 'A-F' 'a-f')"
  case "$expected_size" in
    ''|*[!0-9]*) fail "Invalid size for '$archive_name' in manifest" ;;
  esac
  [ "$expected_size" -ge 1 ] && [ "$expected_size" -le 1073741824 ] || fail "Archive size out of range"
  case "$expected_hash" in *[!0-9a-f]*) fail "Invalid SHA-256 for '$archive_name'" ;; esac
  case "${#expected_hash}" in
    64) ;;
    *) fail "Invalid SHA-256 for '$archive_name' in manifest" ;;
  esac

  archive_path="$tmp_dir/$archive_name"
  if [ -n "${REGRESSIFY_ASSETS_DIR:-}" ]; then
    cp "$REGRESSIFY_ASSETS_DIR/$archive_name" "$archive_path"
  else
    archive_url="https://github.com/precise-alloy/regressify-rs/releases/download/$tag/$archive_name"
    printf 'Downloading %s (%s)...\n' "$archive_name" "$tag"
    fetch_file "$archive_url" "$archive_path" "$expected_size"
  fi

  actual_size="$(wc -c < "$archive_path" | tr -d '[:space:]')"
  [ "$actual_size" = "$expected_size" ] || fail "Size mismatch for '$archive_name' (expected $expected_size, got $actual_size)"

  actual_hash="$(sha256_file "$archive_path")"
  [ "$actual_hash" = "$expected_hash" ] || fail "SHA-256 mismatch for '$archive_name'"

  extract_dir="$tmp_dir/extract-$binary_name"
  mkdir -p "$extract_dir"
  entries="$(unzip -Z1 "$archive_path" | LC_ALL=C sort)"
  expected_entries="$(printf 'LICENSE\nLICENSING.md\n%s\n' "$binary_name" | LC_ALL=C sort)"
  [ "$entries" = "$expected_entries" ] || fail "Unexpected archive entries in '$archive_name'"
  for entry in "$binary_name" LICENSE LICENSING.md; do
    unzip -p "$archive_path" "$entry" > "$extract_dir/$entry"
  done
  chmod +x "$extract_dir/$binary_name"
}

install_dir="${REGRESSIFY_INSTALL_DIR:-$HOME/.local/bin}"
share_dir="${REGRESSIFY_SHARE_DIR:-$HOME/.local/share/regressify}"
download_and_extract "regressify-cli-$target.zip" "regressify"
if [ "$install_desktop" = "1" ]; then
  download_and_extract "regressify-desktop-$target.zip" "regressify-desktop"
fi

mkdir -p "$install_dir" "$share_dir"
install_file() {
  source_file="$1"
  destination="$2"
  [ ! -L "$destination" ] || fail "Refusing to replace symlink '$destination'"
  [ ! -d "$destination" ] || fail "Refusing to replace directory '$destination'"
  temporary="$(mktemp "${destination}.XXXXXX")"
  if ! cp -p "$source_file" "$temporary" || ! mv -f "$temporary" "$destination"; then
    rm -f "$temporary"
    fail "Cannot install '$destination'"
  fi
}
install_file "$tmp_dir/extract-regressify/regressify" "$install_dir/regressify"
if [ "$install_desktop" = "1" ]; then
  install_file "$tmp_dir/extract-regressify-desktop/regressify-desktop" "$install_dir/regressify-desktop"
fi
for license_file in LICENSE LICENSING.md; do
  install_file "$tmp_dir/extract-regressify/$license_file" "$share_dir/$license_file"
done

app_path=""
# Wrap the desktop binary in a Regressify.app bundle so it shows up in Finder,
# Launchpad and Spotlight. The bundle is staged next to its destination and
# swapped in last, so a failure leaves any existing app untouched.
install_macos_app() {
  app_parent="${REGRESSIFY_APP_DIR:-/Applications}"
  if [ -z "${REGRESSIFY_APP_DIR:-}" ] && [ ! -w "$app_parent" ]; then
    app_parent="$HOME/Applications"
  fi
  mkdir -p "$app_parent" || fail "Cannot create '$app_parent'"
  destination="$app_parent/Regressify.app"

  # Serialize installs per destination: mkdir is atomic, so only one installer can
  # move the current app aside and promote its own bundle at a time.
  lock_path="$app_parent/.Regressify.lock"
  lock_wait="${REGRESSIFY_LOCK_TIMEOUT:-30}"
  case "$lock_wait" in ''|*[!0-9]*) lock_wait=30 ;; esac
  until mkdir "$lock_path" 2>/dev/null; do
    lock_wait=$((lock_wait - 1))
    [ "$lock_wait" -ge 0 ] || fail "Another Regressify install is replacing '$destination'; if none is running, remove '$lock_path' and retry"
    sleep 1
  done
  lock_dir="$lock_path"

  [ ! -L "$destination" ] || fail "Refusing to replace symlink '$destination'"

  stage_dir="$(mktemp -d "$app_parent/.Regressify.XXXXXX")" || fail "Cannot write to '$app_parent'"
  staged_app="$stage_dir/Regressify.app"
  mkdir -p "$staged_app/Contents/MacOS" "$staged_app/Contents/Resources"
  cp -p "$tmp_dir/extract-regressify-desktop/regressify-desktop" "$staged_app/Contents/MacOS/regressify-desktop" \
    || { rm -rf "$stage_dir"; fail "Cannot stage Regressify.app"; }

  icon_plist=""
  icon_png="$tmp_dir/icon.png"
  if [ -n "${REGRESSIFY_ICON_FILE:-}" ]; then
    cp "$REGRESSIFY_ICON_FILE" "$icon_png" 2>/dev/null || true
  elif [ -z "${REGRESSIFY_ASSETS_DIR:-}" ]; then
    fetch_file "$origin/icon.png" "$icon_png" 1048576 2>/dev/null || true
  fi
  # Best effort: a missing icon must never fail the install.
  if [ -s "$icon_png" ] && command -v sips >/dev/null 2>&1 && command -v iconutil >/dev/null 2>&1; then
    iconset="$tmp_dir/Regressify.iconset"
    mkdir -p "$iconset"
    if sips -z 16 16 "$icon_png" --out "$iconset/icon_16x16.png" >/dev/null 2>&1 &&
       sips -z 32 32 "$icon_png" --out "$iconset/icon_16x16@2x.png" >/dev/null 2>&1 &&
       sips -z 32 32 "$icon_png" --out "$iconset/icon_32x32.png" >/dev/null 2>&1 &&
       sips -z 64 64 "$icon_png" --out "$iconset/icon_32x32@2x.png" >/dev/null 2>&1 &&
       sips -z 128 128 "$icon_png" --out "$iconset/icon_128x128.png" >/dev/null 2>&1 &&
       sips -z 256 256 "$icon_png" --out "$iconset/icon_128x128@2x.png" >/dev/null 2>&1 &&
       sips -z 256 256 "$icon_png" --out "$iconset/icon_256x256.png" >/dev/null 2>&1 &&
       iconutil -c icns "$iconset" -o "$staged_app/Contents/Resources/Regressify.icns" >/dev/null 2>&1; then
      icon_plist='  <key>CFBundleIconFile</key>
  <string>Regressify</string>
'
    fi
  fi

  cat > "$staged_app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>Regressify</string>
  <key>CFBundleDisplayName</key>
  <string>Regressify</string>
  <key>CFBundleIdentifier</key>
  <string>dev.eshn.regressify</string>
  <key>CFBundleExecutable</key>
  <string>regressify-desktop</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleVersion</key>
  <string>$version</string>
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
${icon_plist}  <key>NSHighResolutionCapable</key>
  <true/>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
</dict>
</plist>
PLIST

  # Ad-hoc sign the bundle so Apple Silicon accepts it; warn instead of failing
  # because the binary already carries its own ad-hoc signature from the linker.
  if command -v codesign >/dev/null 2>&1; then
    if ! codesign --force --deep --sign - "$staged_app" >/dev/null 2>&1; then
      printf 'warning: could not ad-hoc sign Regressify.app; macOS may ask you to allow it in System Settings.\n' >&2
    fi
  fi

  # Move the current app aside first so a failed swap can be rolled back.
  if [ -e "$destination" ]; then
    backup_dest="$destination"
    backup_app="$stage_dir/previous.app"
    mv "$destination" "$backup_app" || fail "Cannot replace '$destination'"
  fi
  # On failure or interruption from here, the exit trap restores the backup.
  mv "$staged_app" "$destination" || fail "Cannot install '$destination'"
  backup_app=""
  app_path="$destination"
}
if [ "$install_desktop" = "1" ] && [ "$os_name" = "Darwin" ]; then
  install_macos_app
fi

if [ "${REGRESSIFY_SKIP_CHROMIUM:-0}" != "1" ]; then
  printf 'Preparing managed Chromium build...\n'
  if ! "$install_dir/regressify" install; then
    printf 'warning: Chromium could not be prepared right now; "regressify install" or the first test run will retry.\n' >&2
  fi
fi

if [ "$install_desktop" = "1" ]; then
  printf 'Installed Regressify %s (regressify and regressify-desktop) to %s\n' "$version" "$install_dir"
  if [ -n "$app_path" ]; then
    printf 'Installed Regressify.app to %s (open it from Finder, Launchpad or Spotlight)\n' "$app_path"
  fi
else
  printf 'Installed Regressify %s (regressify CLI) to %s\n' "$version" "$install_dir"
fi

case ":${PATH:-}:" in
  *":$install_dir:"*) ;;
  *)
    printf '\nAdd %s to your PATH if it is not already configured:\n  export PATH="%s:$PATH"\n' "$install_dir" "$install_dir"
    ;;
esac
)

regressify_install
