#!/usr/bin/env sh
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <x86_64|aarch64> OUTPUT_DIRECTORY" >&2
    exit 2
fi

host_version=0.1.4
architecture="$1"
output_directory="$2"

case "$architecture" in
    x86_64)
        host_target=x86_64-unknown-linux-gnu
        host_sha256=ad42b2bc7ab526d83b98784a5508d78dc024c04588b9924ed9f1b68eec245311
        ;;
    aarch64)
        host_target=aarch64-unknown-linux-gnu
        host_sha256=bc39524d3d066f59ec9fbc23b0c324bf79b325606a1e9bcdc5eb6e9900953329
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
    curl --fail --location --proto '=https' --tlsv1.2 \
        --retry 3 --retry-all-errors --retry-delay 1 \
        --silent --show-error \
        --output "$output_directory/$archive" "$release_url"
fi

printf '%s  %s\n' "$host_sha256" "$output_directory/$archive" \
    | sha256sum --check --status
printf '%s\n' "$output_directory/$archive"
