#!/usr/bin/env sh
set -eu

root_dir="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
inventory="$root_dir/packaging/linux/inventory.toml"
desktop_file="$root_dir/packaging/linux/dev.rayan6ms.rayslash.desktop"
metainfo_file="$root_dir/packaging/linux/dev.rayan6ms.rayslash.metainfo.xml"
fedora_spec="$root_dir/packaging/fedora/rayslash.spec"
arch_pkgbuild="$root_dir/packaging/arch/PKGBUILD"
flatpak_manifest="$root_dir/packaging/flatpak/dev.rayan6ms.rayslash.yml"
debian_builder="$root_dir/packaging/debian/build-deb.sh"
appimage_builder="$root_dir/packaging/appimage/build-appimage.sh"
release_workflow="$root_dir/.github/workflows/release.yml"
host_fetcher="$root_dir/packaging/release/fetch-host-archive.sh"

require_inventory_value() {
    key="$1"
    value="$2"
    if ! grep -Fqx "$key = \"$value\"" "$inventory"; then
        echo "inventory mismatch: expected $key = \"$value\"" >&2
        exit 1
    fi
}

require_file_text() {
    file="$1"
    text="$2"
    if ! grep -Fq -- "$text" "$file"; then
        echo "metadata mismatch: $file does not contain $text" >&2
        exit 1
    fi
}

require_inventory_value "binary_name" "rayslash"
require_inventory_value "app_id" "dev.rayan6ms.rayslash"
require_inventory_value "desktop_entry_name" "dev.rayan6ms.rayslash.desktop"
require_inventory_value "icon_name" "dev.rayan6ms.rayslash"
require_inventory_value "metainfo_id" "dev.rayan6ms.rayslash"

require_file_text "$desktop_file" "Exec=rayslash toggle"
require_file_text "$desktop_file" "Icon=dev.rayan6ms.rayslash"
require_file_text "$desktop_file" "StartupWMClass=dev.rayan6ms.rayslash"
require_file_text "$metainfo_file" "<id>dev.rayan6ms.rayslash</id>"
require_file_text "$metainfo_file" "<binary>rayslash</binary>"
require_file_text "$metainfo_file" "<launchable type=\"desktop-id\">dev.rayan6ms.rayslash.desktop</launchable>"
require_file_text "$fedora_spec" "Release:        1%{?dist}"
require_file_text "$fedora_spec" "%global         module_host_version 0.1.4"
require_file_text "$fedora_spec" "%global         module_host_package_release 2"
require_file_text "$fedora_spec" "Provides:       rayslash-module-host = %{module_host_version}-%{module_host_package_release}"
require_file_text "$fedora_spec" "Provides:       bundled(rayslash-module-host) = %{module_host_version}"
require_file_text "$fedora_spec" "Obsoletes:      rayslash-module-host < %{module_host_version}-%{module_host_package_release}"
require_file_text "$fedora_spec" "Source1:        %{name}-%{version}-vendor.tar.xz"
require_file_text "$fedora_spec" "Source2:        https://github.com/rslauncher/rayslash-module-host/releases/download/v%{module_host_version}/rayslash-module-host-v%{module_host_version}-x86_64-unknown-linux-gnu.tar.xz"
require_file_text "$fedora_spec" "%{_libexecdir}/rayslash/rayslash-module-host"
require_file_text "$fedora_spec" "%{_libexecdir}/rayslash/rayslash-module-compiler"
require_file_text "$fedora_spec" "cargo build --release --frozen --jobs 2 -p rayslash"
require_file_text "$fedora_spec" "cargo test --release --frozen --jobs 2 --workspace"
require_file_text "$host_fetcher" "host_version=0.1.4"
require_file_text "$host_fetcher" "ad42b2bc7ab526d83b98784a5508d78dc024c04588b9924ed9f1b68eec245311"
require_file_text "$host_fetcher" "bc39524d3d066f59ec9fbc23b0c324bf79b325606a1e9bcdc5eb6e9900953329"
require_file_text "$arch_pkgbuild" "depends=('fontconfig' 'rayslash-module-host>=0.1.4')"
require_file_text "$arch_pkgbuild" "pkgrel=1"
require_file_text "$flatpak_manifest" "runtime-version: '25.08'"
require_file_text "$flatpak_manifest" "install -Dm0755 rayslash-module-host /app/libexec/rayslash/rayslash-module-host"
require_file_text "$flatpak_manifest" "install -Dm0755 rayslash-module-compiler /app/libexec/rayslash/rayslash-module-compiler"
require_file_text "$flatpak_manifest" "--talk-name=org.freedesktop.Flatpak"
require_file_text "$flatpak_manifest" "--filesystem=xdg-data/applications:ro"
require_file_text "$debian_builder" 'rayslash_${version}_${architecture}.deb'
require_file_text "$appimage_builder" 'rayslash-${version}-${architecture}.AppImage'
require_file_text "$release_workflow" 'packaging/release/validate-assets.sh'
require_file_text "$release_workflow" 'release-assets/rayslash-${VERSION}-${{ matrix.architecture }}.rpm'
require_file_text "$release_workflow" 'org.freedesktop.Sdk.Extension.rust-stable//25.08'
require_file_text "$release_workflow" 'flatpak run --command=test dev.rayan6ms.rayslash -x /app/bin/rayslash'
require_file_text "$release_workflow" '-x /app/libexec/rayslash/rayslash-module-host'
require_file_text "$release_workflow" '-x /app/libexec/rayslash/rayslash-module-compiler'

app_version="$(awk -F '"' '$1 ~ /^version = / { print $2; exit }' "$root_dir/crates/rayslash-ui/Cargo.toml")"
core_version="$(awk -F '"' '$1 ~ /^version = / { print $2; exit }' "$root_dir/crates/rayslash-core/Cargo.toml")"
spec_version="$(awk '$1 == "Version:" { print $2; exit }' "$fedora_spec")"
pkgbuild_version="$(awk -F= '$1 == "pkgver" { print $2; exit }' "$arch_pkgbuild")"

if [ "$app_version" != "$core_version" ] \
    || [ "$app_version" != "$spec_version" ] \
    || [ "$app_version" != "$pkgbuild_version" ]; then
    echo "package versions do not agree" >&2
    exit 1
fi
require_file_text "$metainfo_file" "<release version=\"$app_version\""

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$desktop_file"
else
    echo "skipping desktop-file-validate: command not found" >&2
fi

if command -v appstreamcli >/dev/null 2>&1; then
    appstreamcli validate --no-net "$metainfo_file"
else
    echo "skipping appstreamcli validate: command not found" >&2
fi
