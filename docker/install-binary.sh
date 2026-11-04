#!/usr/bin/env bash
set -euo pipefail

package=${1:?Expected a Linux amd64 .deb or ELF executable}
destination=/opt/lettuceai/bin/lettuceai
install -d "$(dirname "$destination")"
if dpkg-deb --info "$package" >/dev/null 2>&1; then
  architecture=$(dpkg-deb --field "$package" Architecture)
  dpkg-deb --field "$package" Version > /opt/lettuceai/package-version
  [[ "$architecture" == "$(dpkg --print-architecture)" ]] || {
    echo "Package architecture $architecture does not match this image." >&2
    exit 1
  }
  mv "$package" "$package.deb"
  apt-get update
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "$package.deb"
  mv "$package.deb" "$package"
  rm -rf /var/lib/apt/lists/*
  for binary in /usr/bin/lettuceai /usr/bin/lettuce-tauri; do
    if [[ -x "$binary" ]]; then
      ln -s "$binary" "$destination"
      break
    fi
  done
  [[ -x "$destination" ]] || { echo "The package contains no LettuceAI executable." >&2; exit 1; }
  for desktop in /usr/share/applications/*.desktop; do
    [[ -f "$desktop" ]] || continue
    sed -i -E 's@^(Exec=)(/usr/bin/)?(lettuceai|lettuce-tauri)([[:space:]]|$)@\1/usr/local/bin/lettuceai\4@' "$desktop"
  done
else
  magic=$(od -An -tx1 -N4 "$package" | tr -d ' \n')
  [[ "$magic" == 7f454c46 ]] || { echo "Expected a .deb or ELF executable, not an archive or installer." >&2; exit 1; }
  class=$(od -An -tx1 -j4 -N2 "$package" | tr -d ' \n')
  machine=$(od -An -tx1 -j18 -N2 "$package" | tr -d ' \n')
  [[ "$class" == 0201 && "$machine" == 3e00 ]] || { echo "Expected a Linux amd64 ELF executable." >&2; exit 1; }
  appimage=$(od -An -tx1 -j8 -N3 "$package" | tr -d ' \n')
  [[ "$appimage" != 414901 && "$appimage" != 414902 ]] || { echo "Extract the AppImage first or use its .deb package." >&2; exit 1; }
  install -m755 "$package" "$destination"
fi
ldconfig
