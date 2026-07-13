#!/usr/bin/env sh
set -eu

root_dir="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
inventory="$root_dir/packaging/linux/inventory.toml"
desktop_file="$root_dir/packaging/linux/dev.rayan6ms.rayslash.desktop"
metainfo_file="$root_dir/packaging/linux/dev.rayan6ms.rayslash.metainfo.xml"
fedora_spec="$root_dir/packaging/fedora/rayslash.spec"
arch_pkgbuild="$root_dir/packaging/arch/PKGBUILD"
flatpak_manifest="$root_dir/packaging/flatpak/dev.rayan6ms.rayslash.yml"

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
    if ! grep -Fq "$text" "$file"; then
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
require_file_text "$fedora_spec" "Requires:       rayslash-module-host >= 0.1.2"
require_file_text "$fedora_spec" "Release:        2%{?dist}"
require_file_text "$fedora_spec" "Source1:        %{name}-%{version}-vendor.tar.xz"
require_file_text "$fedora_spec" "cargo build --release --frozen --jobs 2 -p rayslash"
require_file_text "$fedora_spec" "cargo test --frozen --jobs 2 --workspace"
require_file_text "$arch_pkgbuild" "depends=('fontconfig' 'rayslash-module-host>=0.1.2')"
require_file_text "$arch_pkgbuild" "pkgrel=2"
require_file_text "$flatpak_manifest" "install -Dm0755 rayslash-module-host /app/libexec/rayslash/rayslash-module-host"

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
