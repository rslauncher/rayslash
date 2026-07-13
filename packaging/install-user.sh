#!/usr/bin/env sh
set -eu

host_version=0.1.2
host_base_url="https://github.com/rslauncher/rayslash-module-host/releases/download/v${host_version}"

case "$(uname -m)" in
    x86_64)
        host_target=x86_64-unknown-linux-gnu
        host_sha256=9bb6a9ad524a302032e44ead67ef2541cd36d786d9322a03a4ac2ba84a9994f8
        ;;
    aarch64)
        host_target=aarch64-unknown-linux-gnu
        host_sha256=e86423363a104077f5c101fc29cd1476fd3c1ed626fe271a4ed0bf293912e155
        ;;
    *)
        echo "Unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

for command in cargo curl sha256sum tar install; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "Required command is missing: $command" >&2
        exit 1
    fi
done

root_dir="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
archive="rayslash-module-host-v${host_version}-${host_target}.tar.xz"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

curl --fail --location --proto '=https' --tlsv1.2 \
    --output "$temporary_dir/$archive" "$host_base_url/$archive"
printf '%s  %s\n' "$host_sha256" "$temporary_dir/$archive" | sha256sum --check --status
tar --extract --xz --file "$temporary_dir/$archive" --directory "$temporary_dir"

cargo install --locked --path "$root_dir/crates/rayslash-ui"
install -Dm0755 \
    "$temporary_dir/rayslash-module-host-v${host_version}-${host_target}/rayslash-module-host" \
    "$HOME/.local/libexec/rayslash/rayslash-module-host"

echo "Installed rayslash and rayslash-module-host. No optional modules were installed."
