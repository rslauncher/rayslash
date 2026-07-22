#!/usr/bin/env sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: $0 DESTINATION" >&2
    exit 2
fi

host_version=0.1.2
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
        echo "unsupported architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

for command in curl install mktemp sha256sum tar; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "required command is missing: $command" >&2
        exit 1
    fi
done

destination="$1"
archive="rayslash-module-host-v${host_version}-${host_target}.tar.xz"
release_url="https://github.com/rslauncher/rayslash-module-host/releases/download/v${host_version}/$archive"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

curl --fail --location --proto '=https' --tlsv1.2 \
    --output "$temporary_dir/$archive" "$release_url"
printf '%s  %s\n' "$host_sha256" "$temporary_dir/$archive" \
    | sha256sum --check --status
tar --extract --xz --file "$temporary_dir/$archive" --directory "$temporary_dir"
install -Dm0755 \
    "$temporary_dir/rayslash-module-host-v${host_version}-${host_target}/rayslash-module-host" \
    "$destination"
