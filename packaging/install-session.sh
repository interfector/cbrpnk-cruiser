#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
prefix=${PREFIX:-/usr/local}
destdir=${DESTDIR:-}
bin_dir=$destdir$prefix/bin
session_dir=$destdir/usr/share/xsessions

if [ "${SKIP_BUILD:-0}" != "1" ]; then
    (cd "$repo_dir" && cargo build --release)
fi

binary=$repo_dir/target/release/cbrpnk-cruiser
if [ ! -x "$binary" ]; then
    echo "cbrpnk-cruiser: missing $binary; run cargo build --release" >&2
    exit 1
fi

install -d "$bin_dir" "$session_dir"
install -m 0755 "$binary" "$bin_dir/cbrpnk-cruiser"
install -m 0755 "$repo_dir/packaging/cbrpnk-cruiser-session" "$bin_dir/cbrpnk-cruiser-session"
sed "s|@BINDIR@|$prefix/bin|g" \
    "$repo_dir/packaging/cbrpnk-cruiser.desktop.in" \
    > "$session_dir/cbrpnk-cruiser.desktop"
chmod 0644 "$session_dir/cbrpnk-cruiser.desktop"

echo "Installed:"
echo "  $bin_dir/cbrpnk-cruiser"
echo "  $bin_dir/cbrpnk-cruiser-session"
echo "  $session_dir/cbrpnk-cruiser.desktop"
echo "Log out and select CBRPNK//CRUISER in the display manager session menu."
