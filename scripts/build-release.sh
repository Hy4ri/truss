#!/usr/bin/env bash
# Build a native GNU/Linux release bundle. Requires Rust, Python 3.11+, GNU tar
# and coreutils. Usage: scripts/build-release.sh [output-directory]
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo_root"

if [[ $# -gt 1 ]]; then
    printf 'Usage: %s [output-directory]\n' "$0" >&2
    exit 2
fi
if [[ $(uname -s) != Linux ]]; then
    printf 'Release bundles must be built natively on Linux.\n' >&2
    exit 1
fi
arch=$(uname -m)
case "$arch" in
    x86_64|aarch64) ;;
    *) printf 'Unsupported release architecture: %s\n' "$arch" >&2; exit 1 ;;
esac
target="${arch}-unknown-linux-gnu"
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')

if [[ -n ${RELEASE_TAG:-} && $RELEASE_TAG != "v$version" ]]; then
    printf 'Release tag mismatch: expected v%s, got %s\n' "$version" "$RELEASE_TAG" >&2
    exit 1
fi

output_dir=${1:-dist}
mkdir -p -- "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd)
archive="truss-${version}-${arch}-linux.tar.gz"
target_dir=${CARGO_TARGET_DIR:-target}

# Explicit native target prevents a configured cross target from being mislabeled.
cargo build --locked --release --bin truss --target "$target" --target-dir "$target_dir"
binary="$target_dir/$target/release/truss"
actual_version=$("$binary" --version)
if [[ $actual_version != "truss $version" ]]; then
    printf 'Binary version mismatch: expected truss %s, got %s\n' "$version" "$actual_version" >&2
    exit 1
fi
printf '%s\n' "$actual_version"

stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
install -Dm755 "$binary" "$stage/bin/truss"
install -Dm644 resources/config.default.lua "$stage/share/truss/config.default.lua"
install -Dm755 resources/truss-session "$stage/bin/truss-session"
install -Dm644 resources/truss.desktop "$stage/share/wayland-sessions/truss.desktop"
install -Dm644 resources/truss-session.target "$stage/lib/systemd/user/truss-session.target"
mkdir -p -- "$stage/examples"
cp -a -- examples/waybar "$stage/examples/waybar"
# Root-level paths are intentional: Nix and conventional prefix installs can
# consume this bundle directly.
tar --sort=name --owner=0 --group=0 --numeric-owner \
    -C "$stage" -czf "$output_dir/$archive" bin share lib examples
(
    cd -- "$output_dir"
    sha256sum "$archive" > "$archive.sha256"
    sha256sum --check "$archive.sha256"
)
printf 'Release archive: %s/%s\n' "$output_dir" "$archive"
