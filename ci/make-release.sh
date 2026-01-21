#!/usr/bin/env bash
# Builds the release and creates an archive and optionally deploys to GitHub.
set -ex

if [ -z "$FILE_NAME" ]
then
  echo "FILE_NAME must be set"
  exit 1
fi

if [ -z "$GITHUB_REF" ]
then
  echo "GITHUB_REF must be set"
  exit 1
fi

if [ -z "$OS" ]
then
  echo "OS must be set"
  exit 1
fi

if [ -z "$PACKAGE_NAME" ]
then
  echo "PACKAGE_NAME must be set"
  exit 1
fi

if [ -z "$REPO" ] && [ -z "$TEST_MODE" ]
then
  echo "REPO must be set"
  exit 1
fi

if [ -z "$TARGET" ]
then
  echo "TARGET must be set"
  exit 1
fi

if [ -z "$TOKEN" ] && [ -z "$TEST_MODE" ]
then
  echo "TOKEN must be set"
  exit 1
fi

# Strip $FILE_NAME-refs/tags/ from the start of the ref.
TAG=${GITHUB_REF#*/tags/}

host=$(rustc -Vv | grep ^host: | sed -e "s/host: //g")
if [ "$host" != $TARGET ]
then
  export "CARGO_TARGET_$(echo $TARGET | tr a-z- A-Z_)_LINKER"=rust-lld
fi

export CARGO_PROFILE_RELEASE_LTO=true

if [ -z "$SKIP_BUILD" ]
then
  if [ -z "$NO_DEFAULT_FEATURES" ]
  then
    cargo build --locked --bin $FILE_NAME --release --target $TARGET
  else
    cargo build --locked --bin $FILE_NAME --release --target $TARGET --no-default-features
  fi
fi

cd target/$TARGET/release
case $OS in
  ubuntu*)
    # asset="$PACKAGE_NAME-$TAG-$TARGET.tar.gz"
    asset=$FILE_NAME
    tag="linux"
    ;;
  macos*)
    # asset="$PACKAGE_NAME-$TAG-$TARGET.tar.gz"
    asset=$FILE_NAME
    tag="macos"
    ;;
  windows*)
    # asset="$PACKAGE_NAME-$TAG-$TARGET.zip"
    asset="$FILE_NAME.exe"
    tag="windows"
    ;;
  *)
    echo "OS must be set"
    exit 1
    ;;
esac

# Try to compress with UPX if available
if command -v upx &> /dev/null
then
  echo "Compressing binary with UPX..."
  upx --best --lzma "$asset"
else
  echo "UPX not found, skipping compression."
fi

PACKAGE_7Z="$PACKAGE_NAME-$TARGET.7z"
PACKAGE_ZIP="$PACKAGE_NAME-$TARGET.zip"

# If package is already exists, remove it
if [ -f $PACKAGE_7Z ]
then
  rm $PACKAGE_7Z
fi

if [ -f $PACKAGE_ZIP ]
then
  rm $PACKAGE_ZIP
fi

if [ -n "$PASSWORD" ]
then
  # Password is set, enable header compression and encryption
  # -mhc: header compression
  # -mhe: header encryption
  # -mx[N] : set compression level: -mx1 (fastest) ... -mx9 (ultra)
  7z a $PACKAGE_7Z $asset -mhc=on -mhe=on -mx9 -p"$PASSWORD"

  # Also create a zip file
  7z a $PACKAGE_ZIP $asset -mx5 -p"$PASSWORD"
else
  # No password set, just compress
  7z a $PACKAGE_7Z $asset -mx9

  # Also create a zip file
  7z a $PACKAGE_ZIP $asset -mx5
fi

# Check if $TOKEN is set
if [ -n "$TOKEN" ]
then
  # Log in to GitHub
  echo $TOKEN | gh auth login --with-token

  ls

  # Upload the release
  gh release upload $tag $PACKAGE_7Z --repo $REPO --clobber
  gh release upload $tag $PACKAGE_ZIP --repo $REPO --clobber
else
  echo "TOKEN must be set"
fi