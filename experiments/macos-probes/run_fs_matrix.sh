#!/bin/zsh
# Runs fs_semantics on the system volume and on throwaway disk images of other formats.
# usage: run_fs_matrix.sh <fs_semantics binary> <work dir> <output dir>
set -u
BIN=$1; WORK=$2; OUT=$3
mkdir -p "$WORK" "$OUT"
mkdir -p "$WORK/apfs-native"
"$BIN" "$WORK/apfs-native" > "$OUT/apfs-native.txt" 2>&1
echo "apfs-native exit=$?"
typeset -A FORMATS
FORMATS=(apfs-cs "Case-sensitive APFS" hfsplus "HFS+" exfat "ExFAT" fat32 "MS-DOS FAT32")
for key format in ${(kv)FORMATS}; do
  image="$WORK/$key.dmg"; mount="$WORK/mnt-$key"
  hdiutil create -quiet -size 128m -fs "$format" -volname "P$key" "$image" || { echo "$key create failed"; continue; }
  mkdir -p "$mount"
  hdiutil attach -quiet -nobrowse -mountpoint "$mount" "$image" || { echo "$key attach failed"; continue; }
  mkdir "$mount/probe"
  "$BIN" "$mount/probe" > "$OUT/$key.txt" 2>&1
  echo "$key exit=$?"
  hdiutil detach -quiet "$mount" || hdiutil detach -force "$mount"
  rm -f "$image"
done
