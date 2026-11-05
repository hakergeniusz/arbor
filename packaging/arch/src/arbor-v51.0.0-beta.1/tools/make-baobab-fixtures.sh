#!/bin/bash
# make-baobab-fixtures.sh — disposable deterministic test fixtures for
# black-box observation of GNOME Baobab. Everything lives under
# /tmp/ai-agent/baobab-tests and only contains data created here.
set -euo pipefail

ROOT=/tmp/ai-agent/baobab-tests

# Rebuild from scratch (our own /tmp scratch only)
if [ -d "$ROOT" ]; then
  chmod -R u+rwx "$ROOT" 2>/dev/null || true
  trash "$ROOT"
fi
mkdir -p "$ROOT"

# ---------------------------------------------------------------- basic
B="$ROOT/fixture-basic"
mkdir -p "$B/docs" "$B/nested/l1/l2/l3" "$B/large" "$B/.hidden-dir" "$B/empty-dir"

head -c 1000        /dev/zero > "$B/docs/report-1000B.txt"
head -c 10000       /dev/zero > "$B/docs/report-10000B.txt"
head -c 100000      /dev/zero > "$B/docs/data-100000B.bin"
head -c 1000000     /dev/zero > "$B/docs/data-1000000B.bin"

head -c 10000000    /dev/zero > "$B/large/big-10MB.bin"
head -c 33554432    /dev/zero > "$B/large/big-32MiB.bin"
head -c 67108864    /dev/zero > "$B/large/big-64MiB.bin"

head -c 2048        /dev/zero > "$B/nested/level0-2048B.txt"
head -c 4096        /dev/zero > "$B/nested/l1/level1-4096B.txt"
head -c 8192        /dev/zero > "$B/nested/l1/l2/level2-8192B.txt"
head -c 16384       /dev/zero > "$B/nested/l1/l2/l3/level3-16384B.txt"

head -c 512         /dev/zero > "$B/.hidden-file"
head -c 5120        /dev/zero > "$B/.hidden-dir/inside-hidden-5120B.txt"

head -c 256         /dev/zero > "$B/file with spaces.txt"
head -c 256         /dev/zero > "$B/unicode-łóđž-文件-日本語.txt"
head -c 256         /dev/zero > "$B/emoji-📁-name.txt"
head -c 256         /dev/zero > "$B/apostrophe's-file.txt"
LONGNAME=$(printf 'L%.0s' $(seq 1 200))
head -c 256         /dev/zero > "$B/$LONGNAME.txt"

# Sparse: apparent size 10 MiB, allocated ~0
truncate -s 10485760 "$B/sparse-10MiB.dat"

ln -s docs/report-1000B.txt "$B/link-to-file"
ln -s docs                  "$B/link-to-dir"
ln -s does-not-exist-xyz    "$B/broken-link"

# ---------------------------------------------------------------- loop
L="$ROOT/fixture-loop"
mkdir -p "$L/inner"
head -c 4096 /dev/zero > "$L/inner/data-4096B.txt"
ln -s .. "$L/inner/loop-up"    # directory cycle through symlink

# ---------------------------------------------------------------- perm
P="$ROOT/fixture-perm"
mkdir -p "$P/locked"
head -c 4096 /dev/zero > "$P/locked/secret-4096B.txt"
chmod 000 "$P/locked"          # unreadable dir → permission error case

# ---------------------------------------------------------------- trash
T="$ROOT/fixture-trash"
mkdir -p "$T/trash-target"
head -c 12345 /dev/zero > "$T/trash-target/a-12345B.txt"
head -c 54321 /dev/zero > "$T/trash-target/b-54321B.txt"
head -c 999   /dev/zero > "$T/top-level-note-999B.txt"

# ---------------------------------------------------------------- simple
S="$ROOT/scan-target-simple"
mkdir -p "$S/alpha" "$S/beta"
head -c 500000  /dev/zero > "$S/alpha/half-meg.bin"
head -c 250000  /dev/zero > "$S/beta/quarter-meg.bin"
head -c 125000  /dev/zero > "$S/root-125KB.bin"

echo "fixtures created under $ROOT"
