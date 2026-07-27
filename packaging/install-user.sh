#!/usr/bin/env sh
set -eu

case "$(uname -m)" in
    x86_64) architecture=x86_64 ;;
    aarch64) architecture=aarch64 ;;
    *)
        echo "Unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

for command in cargo tar install; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "Required command is missing: $command" >&2
        exit 1
    fi
done

root_dir="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

archive_path="$("$root_dir/packaging/release/fetch-host-archive.sh" "$architecture" "$temporary_dir")"
archive="$(basename -- "$archive_path")"
host_directory="${archive%.tar.xz}"
tar --extract --xz --file "$archive_path" --directory "$temporary_dir"

cargo install --locked --path "$root_dir/crates/rayslash-ui"
install -Dm0755 \
    "$temporary_dir/$host_directory/rayslash-module-host" \
    "$HOME/.local/libexec/rayslash/rayslash-module-host"
install -Dm0755 \
    "$temporary_dir/$host_directory/rayslash-module-compiler" \
    "$HOME/.local/libexec/rayslash/rayslash-module-compiler"

echo "Installed rayslash and its module runtime. No optional modules were installed."
