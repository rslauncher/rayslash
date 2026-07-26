#!/usr/bin/env sh
set -eu

if [ "$#" -ne 4 ]; then
    echo "usage: $0 RAYSLASH_BINARY MODULE_HOST_BINARY LINUXDEPLOY OUTPUT_DIRECTORY" >&2
    exit 2
fi

root_dir="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
rayslash_binary="$1"
module_host_binary="$2"
linuxdeploy="$3"
output_dir="$4"
version="$(awk -F '"' '$1 ~ /^version = / { print $2; exit }' "$root_dir/crates/rayslash-ui/Cargo.toml")"

case "$(uname -m)" in
    x86_64) architecture=x86_64 ;;
    aarch64) architecture=aarch64 ;;
    *)
        echo "unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

app_dir="$(mktemp -d)/Rayslash.AppDir"
mkdir -p "$output_dir"
install -Dm0755 "$rayslash_binary" "$app_dir/usr/bin/rayslash"
install -Dm0755 "$module_host_binary" \
    "$app_dir/usr/libexec/rayslash/rayslash-module-host"
install -Dm0755 "$root_dir/packaging/appimage/AppRun" "$app_dir/AppRun"
install -Dm0644 "$root_dir/packaging/linux/dev.rayan6ms.rayslash.desktop" \
    "$app_dir/usr/share/applications/dev.rayan6ms.rayslash.desktop"
install -Dm0644 "$root_dir/icons/rayslash-icon.svg" \
    "$app_dir/usr/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg"
install -Dm0644 "$root_dir/packaging/linux/dev.rayan6ms.rayslash.metainfo.xml" \
    "$app_dir/usr/share/metainfo/dev.rayan6ms.rayslash.metainfo.xml"

build_dir="$(mktemp -d)"
(
    cd "$build_dir"
    ARCH="$architecture" "$linuxdeploy" --appimage-extract-and-run \
        --appdir "$app_dir" \
        --executable "$app_dir/usr/bin/rayslash" \
        --desktop-file "$app_dir/usr/share/applications/dev.rayan6ms.rayslash.desktop" \
        --icon-file "$app_dir/usr/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg" \
        --output appimage
)

generated="$(find "$build_dir" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
if [ -z "$generated" ]; then
    echo "linuxdeploy did not produce an AppImage" >&2
    exit 1
fi

output="$output_dir/rayslash-${version}-${architecture}.AppImage"
install -Dm0755 "$generated" "$output"
printf '%s\n' "$output"
