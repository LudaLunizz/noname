#!/bin/sh
set -eu

repo="Silvertree2010/noname"
dir="${NONAME_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Linux) os="unknown-linux-musl" ;;
  Darwin) os="apple-darwin" ;;
  *) echo "unsupported system: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) arch="x86_64" ;;
  aarch64 | arm64) arch="aarch64" ;;
  *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

target="$arch-$os"
url="https://github.com/$repo/releases/latest/download/noname-$target.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "downloading noname for $target"
curl -fsSL "$url" -o "$tmp/noname.tar.gz"
tar -xzf "$tmp/noname.tar.gz" -C "$tmp"
mkdir -p "$dir"
install -m 755 "$tmp/noname" "$dir/noname"
echo "installed to $dir/noname"

case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "add $dir to your PATH to run noname" ;;
esac

command -v claude >/dev/null 2>&1 || echo "noname needs Claude Code: https://claude.com/claude-code"
