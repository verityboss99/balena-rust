#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
STAGE_DIR="$ROOT_DIR/target/packaging-stage"
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n1)"
VERSION="${VERSION:-2.1.9}"

ARCH_BIN_DIR="$ROOT_DIR/target/release"
PORTABLE_BIN_DIR="$ROOT_DIR/target/x86_64-unknown-linux-gnu/release"
WIN64_BIN_DIR="$ROOT_DIR/target/x86_64-pc-windows-gnullvm/release"
WIN32_BIN_DIR="$ROOT_DIR/target/i686-pc-windows-gnullvm/release"

APPIMAGETOOL="${APPIMAGETOOL:-/home/xp/.local/bin/appimagetool}"

rm -rf "$STAGE_DIR"
mkdir -p "$DIST_DIR" "$STAGE_DIR"

echo "=== 1. Packaging Debian (.deb) ==="
DEB_ROOT="$STAGE_DIR/deb"
DEB_PKG="balena-etcher_${VERSION}_amd64.deb"
mkdir -p \
    "$DEB_ROOT/DEBIAN" \
    "$DEB_ROOT/usr/lib/balena-etcher" \
    "$DEB_ROOT/usr/bin" \
    "$DEB_ROOT/usr/share/applications" \
    "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps" \
    "$DEB_ROOT/usr/share/polkit-1/actions" \
    "$DEB_ROOT/usr/share/doc/balena-etcher"

install -m755 "$PORTABLE_BIN_DIR/balena-etcher" "$DEB_ROOT/usr/lib/balena-etcher/balena-etcher"
install -m755 "$PORTABLE_BIN_DIR/etcher-util" "$DEB_ROOT/usr/lib/balena-etcher/etcher-util"
ln -s "../lib/balena-etcher/balena-etcher" "$DEB_ROOT/usr/bin/balena-etcher"
install -m644 "$ROOT_DIR/packaging/balena-etcher.desktop" "$DEB_ROOT/usr/share/applications/balena-etcher.desktop"
install -m644 "$ROOT_DIR/assets/icon.png" "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps/balena-etcher.png"
install -m644 "$ROOT_DIR/packaging/io.balena.etcher.policy" "$DEB_ROOT/usr/share/polkit-1/actions/io.balena.etcher.policy"
install -m644 "$ROOT_DIR/LICENSE" "$DEB_ROOT/usr/share/doc/balena-etcher/copyright"

INSTALLED_SIZE_KB="$(du -sk "$DEB_ROOT/usr" | awk '{print $1}')"
cat > "$DEB_ROOT/DEBIAN/control" <<EOF
Package: balena-etcher
Version: ${VERSION}
Architecture: amd64
Section: utils
Priority: optional
Installed-Size: ${INSTALLED_SIZE_KB}
Depends: libc6 (>= 2.31), policykit-1 | pkexec, libnotify-bin | notification-daemon
Maintainer: Balena Ltd. <hello@balena.io>
Homepage: https://etcher.balena.io/
Description: Flash OS images to SD cards and USB drives, safely and easily.
 Pure Rust rewrite of balenaEtcher with hardware drive validation,
 compressed image streaming, and multi-target verification.
EOF

(
    cd "$DEB_ROOT"
    find usr -type f -print0 | xargs -0 md5sum > DEBIAN/md5sums
)
chmod 0644 "$DEB_ROOT/DEBIAN/control" "$DEB_ROOT/DEBIAN/md5sums"

printf '2.0\n' > "$STAGE_DIR/debian-binary"
tar --owner=0 --group=0 --numeric-owner -C "$DEB_ROOT/DEBIAN" -cJf "$STAGE_DIR/control.tar.xz" .
tar --owner=0 --group=0 --numeric-owner -C "$DEB_ROOT" --exclude='./DEBIAN' -cJf "$STAGE_DIR/data.tar.xz" .
rm -f "$DIST_DIR/$DEB_PKG"
ar rcs "$DIST_DIR/$DEB_PKG" "$STAGE_DIR/debian-binary" "$STAGE_DIR/control.tar.xz" "$STAGE_DIR/data.tar.xz"

echo "=== 2. Packaging AppImage ==="
APPDIR="$STAGE_DIR/AppDir"
APPIMAGE="balenaEtcher-${VERSION}-x86_64.AppImage"
mkdir -p \
    "$APPDIR/usr/bin" \
    "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps"

install -m755 "$PORTABLE_BIN_DIR/balena-etcher" "$APPDIR/usr/bin/balena-etcher"
install -m755 "$PORTABLE_BIN_DIR/etcher-util" "$APPDIR/usr/bin/etcher-util"
install -m644 "$ROOT_DIR/packaging/balena-etcher.desktop" "$APPDIR/balena-etcher.desktop"
install -m644 "$ROOT_DIR/packaging/balena-etcher.desktop" "$APPDIR/usr/share/applications/balena-etcher.desktop"
install -m644 "$ROOT_DIR/assets/icon.png" "$APPDIR/balena-etcher.png"
install -m644 "$ROOT_DIR/assets/icon.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/balena-etcher.png"

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/balena-etcher" "$@"
EOF
chmod 0755 "$APPDIR/AppRun"

rm -f "$DIST_DIR/$APPIMAGE"
ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" "$DIST_DIR/$APPIMAGE"

echo "=== 3. Packaging Arch Linux (.pkg.tar.zst) ==="
ARCH_STAGE="$STAGE_DIR/arch"
mkdir -p "$ARCH_STAGE/src"
cp "$ROOT_DIR/packaging/arch/PKGBUILD" "$ARCH_STAGE/PKGBUILD"
install -m755 "$ARCH_BIN_DIR/balena-etcher" "$ARCH_STAGE/src/balena-etcher"
install -m755 "$ARCH_BIN_DIR/etcher-util" "$ARCH_STAGE/src/etcher-util"
install -m644 "$ROOT_DIR/packaging/balena-etcher.desktop" "$ARCH_STAGE/src/balena-etcher.desktop"
install -m644 "$ROOT_DIR/assets/icon.png" "$ARCH_STAGE/src/icon.png"
install -m644 "$ROOT_DIR/packaging/io.balena.etcher.policy" "$ARCH_STAGE/src/io.balena.etcher.policy"
install -m644 "$ROOT_DIR/LICENSE" "$ARCH_STAGE/src/LICENSE"

(
    cd "$ARCH_STAGE"
    PKGDEST="$DIST_DIR" makepkg -f --noconfirm --nodeps
)

echo "=== 4. Packaging Windows 10+ 64-bit & 32-bit ZIPs ==="
python3 - "$ROOT_DIR" "$DIST_DIR" "$VERSION" "$WIN64_BIN_DIR" "$WIN32_BIN_DIR" <<'PY'
import os
import sys
import zipfile

root_dir, dist_dir, version, win64_dir, win32_dir = sys.argv[1:6]
license_path = os.path.join(root_dir, "LICENSE")

bundles = [
    (f"balenaEtcher-{version}-win10-x64", win64_dir),
    (f"balenaEtcher-{version}-win10-x86", win32_dir),
]

for folder, bin_dir in bundles:
    zip_path = os.path.join(dist_dir, f"{folder}.zip")
    with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for fname in ("balena-etcher.exe", "etcher-util.exe"):
            src = os.path.join(bin_dir, fname)
            if not os.path.isfile(src):
                raise FileNotFoundError(f"Missing Windows binary: {src}")
            zf.write(src, arcname=f"{folder}/{fname}")
        zf.write(license_path, arcname=f"{folder}/LICENSE")
PY

echo "=== 5. Writing SHA256SUMS.txt ==="
(
    cd "$DIST_DIR"
    rm -f SHA256SUMS.txt
    sha256sum \
        "$DEB_PKG" \
        "$APPIMAGE" \
        "balena-etcher-${VERSION}-1-x86_64.pkg.tar.zst" \
        "balenaEtcher-${VERSION}-win10-x64.zip" \
        "balenaEtcher-${VERSION}-win10-x86.zip" > SHA256SUMS.txt
)

ls -lh "$DIST_DIR"
