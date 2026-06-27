#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
name="trpg-platform-source"
out_dir="$root/dist"
archive="$out_dir/$name.tar.gz"
max_bytes=$((5 * 1024 * 1024))

mkdir -p "$out_dir"
rm -f "$archive"

files="$(mktemp)"
trap 'rm -f "$files"' EXIT

(
  cd "$root"
  git ls-files -z --cached --others --exclude-standard > "$files"
)

if tr '\0' '\n' < "$files" | grep -E '(^|/)\.env($|\.)' | grep -vE '(^|/)\.env\.example$' >/dev/null; then
  echo "refusing to package .env files" >&2
  exit 1
fi

tar -C "$root" \
  --exclude='*.tsbuildinfo' \
  --null -T "$files" \
  --transform "s,^,$name/," \
  -czf "$archive"

bytes="$(wc -c < "$archive" | tr -d '[:space:]')"
echo "created $archive ($bytes bytes)"

if [ "$bytes" -gt "$max_bytes" ]; then
  echo "source package exceeds 5 MB" >&2
  exit 1
fi
