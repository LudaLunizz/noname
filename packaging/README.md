# Packaging

Every release run renders the files in this folder with the real version and checksums and attaches them as `packaging.tar.gz`.

| target | where it goes |
|---|---|
| `aur/noname`, `aur/noname-bin`, `aur/noname-git` | one AUR repo each: `ssh://aur@aur.archlinux.org/<name>.git`, commit `PKGBUILD` and `.SRCINFO` |
| `homebrew/noname.rb` | `Formula/noname.rb` in the tap repo `Silvertree2010/homebrew-noname` |
| `scoop/noname.json` | `bucket/noname.json` in `Silvertree2010/scoop-noname`; `checkver` and `autoupdate` let Scoop pick up later releases |

Render by hand: `packaging/render.sh <version> SHA256SUMS out/`.

Packaged installs never update themselves; noname detects `/usr`, Homebrew and Scoop paths and builds with `NONAME_CHANNEL=package`.
