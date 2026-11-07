#!/usr/bin/env bash
set -euo pipefail

ver="$1"
sums="$2"
out="$3"
here="$(cd "$(dirname "$0")" && pwd)"
repo="https://github.com/Silvertree2010/noname"

rm -rf "$out"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
cp -r "$here/aur" "$here/homebrew" "$here/scoop" "$out/"

src="SKIP"
if curl -fsSL "$repo/archive/refs/tags/v$ver.tar.gz" -o /tmp/noname-src.tar.gz 2>/dev/null; then
  src="$(sha256sum /tmp/noname-src.tar.gz | cut -d' ' -f1)"
fi

args=(-e "s|@VERSION@|$ver|g" -e "s|@SHA_SRC@|$src|g")
while read -r hash file; do
  t="${file#noname-}"
  t="${t%.tar.gz}"
  t="${t%.zip}"
  args+=(-e "s|@SHA_$t@|$hash|g")
done < <(tr -d '\r' < "$sums" | grep -E 'noname-.*\.(tar\.gz|zip)$')

find "$out" -type f -exec sed -i "${args[@]}" {} +

if left="$(grep -rl '@SHA_\|@VERSION@' "$out")"; then
  echo "unfilled placeholders in: $left" >&2
fi

if command -v makepkg >/dev/null 2>&1; then
  for d in "$out"/aur/*/; do
    (cd "$d" && makepkg --printsrcinfo > .SRCINFO)
  done
elif command -v docker >/dev/null 2>&1; then
  docker run --rm -v "$out/aur:/w" archlinux:latest bash -c '
    pacman -Sy --noconfirm --needed base-devel git >/dev/null 2>&1
    useradd -m b && chown -R b /w
    for d in /w/*/; do su b -c "cd $d && makepkg --printsrcinfo > .SRCINFO"; done
    chown -R '"$(id -u):$(id -g)"' /w'
fi
