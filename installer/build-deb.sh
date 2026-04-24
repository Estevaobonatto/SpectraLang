#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-deb.sh — Build a Debian/Ubuntu .deb package for SpectraLang
#
# Usage:
#   ./build-deb.sh <version> <binaries-dir>
#
# Example:
#   ./build-deb.sh 0.1.0 /path/to/bin
#
# The script expects the following files in <binaries-dir>:
#   spectralang
#   spectra-lsp
#   spectra-vscode-extension.vsix   (optional — bundled when present)
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

VERSION="${1:?Usage: $0 <version> <binaries-dir>}"
BIN_DIR="${2:?Usage: $0 <version> <binaries-dir>}"
ARCH="amd64"
PACKAGE="spectralang"
MAINTAINER="SpectraLang <support@spectralang.dev>"
DESCRIPTION="SpectraLang compiler, CLI, and language server"
HOMEPAGE="https://github.com/Estevaobonatto/SpectraLang"

STAGING="$(pwd)/deb-staging-${VERSION}"
DEB_NAME="${PACKAGE}_${VERSION}_${ARCH}.deb"
VSIX_SRC="${BIN_DIR}/spectra-vscode-extension.vsix"

# ── Clean previous staging ────────────────────────────────────────────────────
rm -rf "${STAGING}"

# ── Directory layout ──────────────────────────────────────────────────────────
# /usr/local/bin           — executables
# /usr/share/spectra       — VS Code extension VSIX
# /usr/share/doc/spectra   — doc placeholder
mkdir -p "${STAGING}/DEBIAN"
mkdir -p "${STAGING}/usr/local/bin"
mkdir -p "${STAGING}/usr/share/spectra"
mkdir -p "${STAGING}/usr/share/doc/${PACKAGE}"

# ── Copy binaries ─────────────────────────────────────────────────────────────
cp "${BIN_DIR}/spectralang" "${STAGING}/usr/local/bin/spectralang"
cp "${BIN_DIR}/spectra-lsp" "${STAGING}/usr/local/bin/spectra-lsp"
chmod 755 "${STAGING}/usr/local/bin/spectralang"
chmod 755 "${STAGING}/usr/local/bin/spectra-lsp"

# ── Copy VSIX (if present) ────────────────────────────────────────────────────
if [[ -f "${VSIX_SRC}" ]]; then
  cp "${VSIX_SRC}" "${STAGING}/usr/share/spectra/spectra-vscode-extension.vsix"
  chmod 644 "${STAGING}/usr/share/spectra/spectra-vscode-extension.vsix"
  INCLUDES_VSIX=true
else
  INCLUDES_VSIX=false
  echo "Note: VSIX not found at ${VSIX_SRC}, skipping VS Code extension bundling."
fi

# ── Changelog stub (required by lintian) ─────────────────────────────────────
cat > "${STAGING}/usr/share/doc/${PACKAGE}/changelog.Debian" <<EOF
${PACKAGE} (${VERSION}) unstable; urgency=low

  * Release ${VERSION}.

 -- ${MAINTAINER}  $(date -R)
EOF
gzip -9 -n "${STAGING}/usr/share/doc/${PACKAGE}/changelog.Debian"

# ── copyright ─────────────────────────────────────────────────────────────────
cat > "${STAGING}/usr/share/doc/${PACKAGE}/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: ${PACKAGE}
Source: ${HOMEPAGE}

Files: *
Copyright: $(date +%Y) SpectraLang Contributors
License: MIT
EOF

# ── Installed-Size (in KiB) ───────────────────────────────────────────────────
INSTALLED_SIZE=$(du -sk "${STAGING}/usr" | cut -f1)

# ── DEBIAN/control ────────────────────────────────────────────────────────────
cat > "${STAGING}/DEBIAN/control" <<EOF
Package: ${PACKAGE}
Version: ${VERSION}
Architecture: ${ARCH}
Maintainer: ${MAINTAINER}
Installed-Size: ${INSTALLED_SIZE}
Depends: libc6 (>= 2.17)
Homepage: ${HOMEPAGE}
Description: ${DESCRIPTION}
 SpectraLang is a statically-typed, compiled programming language.
 .
 This package provides:
  - spectralang: the command-line compiler and REPL
  - spectra-lsp: the Language Server Protocol daemon for editor integration
EOF

# ── postinst / prerm hooks (optional) ────────────────────────────────────────
cat > "${STAGING}/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
VSIX="/usr/share/spectra/spectra-vscode-extension.vsix"
if [ -f "${VSIX}" ]; then
    for cmd in code code-insiders; do
        if command -v "${cmd}" >/dev/null 2>&1; then
            echo "Installing SpectraLang VS Code extension via ${cmd}..."
            "${cmd}" --install-extension "${VSIX}" --force || true
            break
        fi
    done
fi
exit 0
EOF
chmod 755 "${STAGING}/DEBIAN/postinst"

cat > "${STAGING}/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -e
exit 0
EOF
chmod 755 "${STAGING}/DEBIAN/prerm"

# ── Build .deb ────────────────────────────────────────────────────────────────
dpkg-deb --root-owner-group --build "${STAGING}" "installer/${DEB_NAME}"

# ── Cleanup staging ───────────────────────────────────────────────────────────
rm -rf "${STAGING}"

echo "Created: installer/${DEB_NAME}"
