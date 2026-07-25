#!/usr/bin/env sh
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: $0 ASSET_DIRECTORY VERSION" >&2
    exit 2
fi

asset_dir="$1"
version="$2"

for asset in \
    "rayslash-${version}-x86_64.rpm" \
    "rayslash-${version}-aarch64.rpm" \
    "rayslash_${version}_amd64.deb" \
    "rayslash_${version}_arm64.deb" \
    "rayslash-${version}-x86_64.AppImage" \
    "rayslash-${version}-aarch64.AppImage" \
    "rayslash-${version}-x86_64.flatpak" \
    "rayslash-${version}-aarch64.flatpak"
do
    if [ ! -s "$asset_dir/$asset" ]; then
        echo "release asset is missing or empty: $asset" >&2
        exit 1
    fi
done

actual_count="$(find "$asset_dir" -maxdepth 1 -type f ! -name SHA256SUMS | wc -l)"
if [ "$actual_count" -ne 8 ]; then
    echo "expected 8 release binaries, found $actual_count" >&2
    exit 1
fi

(
    cd "$asset_dir"
    sha256sum \
        rayslash-* rayslash_* \
        > SHA256SUMS
    test "$(wc -l < SHA256SUMS)" -eq 8
    sha256sum --check --strict SHA256SUMS
)
