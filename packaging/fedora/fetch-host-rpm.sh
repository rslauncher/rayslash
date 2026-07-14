#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <x86_64|aarch64> <output-directory>" >&2
  exit 2
fi

architecture=$1
output_directory=$(realpath -m "$2")
repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
checksums="$repository_root/packaging/fedora/host-release-assets.sha256"
version=0.1.2

case "$architecture" in
  x86_64|aarch64) ;;
  *) echo "unsupported architecture: $architecture" >&2; exit 2 ;;
esac

rpm_name="rayslash-module-host-${version}-1.fc44.${architecture}.rpm"
sidecar="${rpm_name}.sha256"
release_base="https://github.com/rslauncher/rayslash-module-host/releases/download/v${version}"
mkdir -p "$output_directory"

curl --fail --location --retry 3 --silent --show-error \
  --output "$output_directory/$rpm_name" "$release_base/$rpm_name"
curl --fail --location --retry 3 --silent --show-error \
  --output "$output_directory/$sidecar" "$release_base/$sidecar"

for asset in "$rpm_name" "$sidecar"; do
  expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$checksums")
  if [[ -z "$expected" ]]; then
    echo "missing pinned checksum for $asset" >&2
    exit 1
  fi
  printf '%s  %s\n' "$expected" "$asset" | \
    (cd "$output_directory" && sha256sum --check --strict -)
done
(cd "$output_directory" && sha256sum --check --strict "$sidecar")
rpm -Kv "$output_directory/$rpm_name"
