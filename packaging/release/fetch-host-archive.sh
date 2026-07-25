#!/usr/bin/env sh
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <x86_64|aarch64> OUTPUT_DIRECTORY" >&2
    exit 2
fi

host_version=0.1.2
architecture="$1"
output_directory="$2"

case "$architecture" in
    x86_64)
        host_target=x86_64-unknown-linux-gnu
        host_sha256=9bb6a9ad524a302032e44ead67ef2541cd36d786d9322a03a4ac2ba84a9994f8
        ;;
    aarch64)
        host_target=aarch64-unknown-linux-gnu
        host_sha256=e86423363a104077f5c101fc29cd1476fd3c1ed626fe271a4ed0bf293912e155
        ;;
    *)
        echo "unsupported architecture: $architecture" >&2
        exit 2
        ;;
esac

for command in curl mkdir sha256sum; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "required command is missing: $command" >&2
        exit 1
    fi
done

archive="rayslash-module-host-v${host_version}-${host_target}.tar.xz"
release_url="https://github.com/rslauncher/rayslash-module-host/releases/download/v${host_version}/$archive"
mkdir -p "$output_directory"

if ! printf '%s  %s\n' "$host_sha256" "$output_directory/$archive" \
    | sha256sum --check --status 2>/dev/null
then
    curl --fail --location --proto '=https' --tlsv1.2 --retry 3 \
        --silent --show-error \
        --output "$output_directory/$archive" "$release_url"
fi

printf '%s  %s\n' "$host_sha256" "$output_directory/$archive" \
    | sha256sum --check --status
printf '%s\n' "$output_directory/$archive"
