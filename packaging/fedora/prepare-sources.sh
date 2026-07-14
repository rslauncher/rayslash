#!/usr/bin/env sh
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "usage: $0 OUTPUT_DIRECTORY [GIT_REF]" >&2
    exit 2
fi

for command in cargo git tar xz; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "required command is missing: $command" >&2
        exit 1
    fi
done

root_dir="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
output_dir="$1"
git_ref="${2:-HEAD}"
name=rayslash
version="$(awk '$1 == "Version:" { print $2; exit }' "$root_dir/packaging/fedora/rayslash.spec")"

if [ -z "$version" ]; then
    echo "could not read Version from the Fedora spec" >&2
    exit 1
fi

commit="$(git -C "$root_dir" rev-parse --verify "${git_ref}^{commit}")"
source_date_epoch="$(git -C "$root_dir" show -s --format=%ct "$commit")"
mkdir -p "$output_dir"
output_dir="$(CDPATH= cd -- "$output_dir" && pwd)"
source_archive="$output_dir/$name-$version.tar.gz"
vendor_archive="$output_dir/$name-$version-vendor.tar.xz"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

git -C "$root_dir" archive \
    --format=tar.gz \
    --prefix="$name-$version/" \
    --output="$source_archive" \
    "$commit"

mkdir -p "$temporary_dir/source"
git -C "$root_dir" archive --format=tar "$commit" \
    | tar -xf - -C "$temporary_dir/source"

cargo vendor \
    --quiet \
    --locked \
    --versioned-dirs \
    --manifest-path "$temporary_dir/source/Cargo.toml" \
    "$temporary_dir/vendor" \
    >/dev/null

LC_ALL=C tar \
    --create \
    --xz \
    --file "$vendor_archive" \
    --directory "$temporary_dir" \
    --sort=name \
    --mtime="@$source_date_epoch" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    --format=posix \
    --pax-option=delete=atime,delete=ctime \
    vendor

sha256sum "$source_archive" "$vendor_archive"
