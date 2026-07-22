#!/usr/bin/env sh
set -eu

if [ "$#" -ne 3 ]; then
    echo "usage: $0 RAYSLASH_BINARY MODULE_HOST_BINARY OUTPUT_DIRECTORY" >&2
    exit 2
fi

root_dir="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
rayslash_binary="$1"
module_host_binary="$2"
output_dir="$3"
version="$(awk -F '"' '$1 ~ /^version = / { print $2; exit }' "$root_dir/crates/rayslash-ui/Cargo.toml")"

case "$(uname -m)" in
    x86_64) architecture=amd64 ;;
    aarch64) architecture=arm64 ;;
    *)
        echo "unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

for file in "$rayslash_binary" "$module_host_binary"; do
    if [ ! -x "$file" ]; then
        echo "required executable is missing: $file" >&2
        exit 1
    fi
done

package_root="$(mktemp -d)"
trap 'rm -rf "$package_root"' EXIT HUP INT TERM
mkdir -p "$output_dir"

install -Dm0755 "$rayslash_binary" "$package_root/usr/bin/rayslash"
install -Dm0755 "$module_host_binary" \
    "$package_root/usr/libexec/rayslash/rayslash-module-host"
install -Dm0644 "$root_dir/packaging/linux/dev.rayan6ms.rayslash.desktop" \
    "$package_root/usr/share/applications/dev.rayan6ms.rayslash.desktop"
install -Dm0644 "$root_dir/icons/rayslash-icon.svg" \
    "$package_root/usr/share/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg"
install -Dm0644 "$root_dir/packaging/linux/dev.rayan6ms.rayslash.metainfo.xml" \
    "$package_root/usr/share/metainfo/dev.rayan6ms.rayslash.metainfo.xml"
install -Dm0644 "$root_dir/LICENSE" \
    "$package_root/usr/share/doc/rayslash/copyright"
install -Dm0644 "$root_dir/docs/INSTALL.md" \
    "$package_root/usr/share/doc/rayslash/INSTALL.md"

mkdir -p "$package_root/DEBIAN"
cat >"$package_root/DEBIAN/control" <<EOF
Package: rayslash
Version: $version
Section: utils
Priority: optional
Architecture: $architecture
Maintainer: RaySlash contributors <rayan6ms@users.noreply.github.com>
Depends: libc6, libfontconfig1
Homepage: https://github.com/rslauncher/rayslash
Description: Fast native Linux desktop launcher
 rayslash is a lightweight keyboard-first launcher for Linux desktops.
 Optional capabilities are installed on demand as verified modules.
EOF

output="$output_dir/rayslash_${version}_${architecture}.deb"
dpkg-deb --root-owner-group --build "$package_root" "$output"
dpkg-deb --info "$output" >/dev/null
package_contents="$(dpkg-deb --contents "$output")"
case "$package_contents" in
    *'./usr/bin/rayslash'*) ;;
    *) echo "Debian package is missing /usr/bin/rayslash" >&2; exit 1 ;;
esac
case "$package_contents" in
    *'./usr/libexec/rayslash/rayslash-module-host'*) ;;
    *) echo "Debian package is missing the module host" >&2; exit 1 ;;
esac
printf '%s\n' "$output"
