#!/usr/bin/env sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: $0 DESTINATION" >&2
    exit 2
fi

case "$(uname -m)" in
    x86_64) architecture=x86_64 ;;
    aarch64) architecture=aarch64 ;;
    *)
        echo "unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

for command in install mktemp tar; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "required command is missing: $command" >&2
        exit 1
    fi
done

destination="$1"
script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

archive_path="$("$script_dir/fetch-host-archive.sh" "$architecture" "$temporary_dir")"
archive="$(basename -- "$archive_path")"
host_directory="${archive%.tar.xz}"
tar --extract --xz --file "$archive_path" --directory "$temporary_dir"
install -Dm0755 \
    "$temporary_dir/$host_directory/rayslash-module-host" \
    "$destination"
