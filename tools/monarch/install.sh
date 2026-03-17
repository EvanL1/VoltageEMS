#!/usr/bin/env bash
# Monarch CLI installer — auto-detects platform, downloads binary, adds to PATH
# Usage: curl -fsSL https://raw.githubusercontent.com/EvanL1/VoltageEMS/develop/tools/monarch/install.sh | bash
set -euo pipefail

REPO="EvanL1/VoltageEMS"
INSTALL_DIR="${MONARCH_INSTALL_DIR:-$HOME/.local/bin}"

# ─── Detect platform ────────────────────────────────────────────────────────

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)  os="linux" ;;
        Darwin) os="darwin" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *) echo "Error: unsupported OS: $os" >&2; exit 1 ;;
    esac

    case "$arch" in
        x86_64|amd64)  arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) echo "Error: unsupported architecture: $arch" >&2; exit 1 ;;
    esac

    echo "${os}-${arch}"
}

# ─── Find latest release ────────────────────────────────────────────────────

latest_tag() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases" \
        | grep -o '"tag_name": *"[^"]*-monarch"' \
        | head -1 \
        | sed 's/.*"tag_name": *"//;s/"//'
}

# ─── Main ────────────────────────────────────────────────────────────────────

main() {
    local platform tag archive url

    echo "Detecting platform..."
    platform="$(detect_platform)"
    echo "  Platform: ${platform}"

    echo "Finding latest release..."
    tag="$(latest_tag)"
    if [ -z "$tag" ]; then
        echo "Error: no monarch release found" >&2
        exit 1
    fi
    echo "  Version: ${tag}"

    archive="monarch-${platform}"
    if [ "${platform%%-*}" = "windows" ]; then
        url="https://github.com/${REPO}/releases/download/${tag}/${archive}.zip"
    else
        url="https://github.com/${REPO}/releases/download/${tag}/${archive}.tar.gz"
    fi

    echo "Downloading ${url}..."
    mkdir -p "$INSTALL_DIR"

    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    if [ "${platform%%-*}" = "windows" ]; then
        curl -fsSL "$url" -o "$tmpdir/monarch.zip"
        unzip -qo "$tmpdir/monarch.zip" -d "$tmpdir"
        cp "$tmpdir/monarch.exe" "$INSTALL_DIR/"
    else
        curl -fsSL "$url" | tar xz -C "$tmpdir"
        chmod +x "$tmpdir/monarch"
        cp "$tmpdir/monarch" "$INSTALL_DIR/"
    fi

    echo ""
    echo "Installed: ${INSTALL_DIR}/monarch"

    # Check if INSTALL_DIR is in PATH
    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        echo ""
        echo "Add to PATH (add to your shell profile):"
        echo ""
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo ""

        # Detect shell and suggest the right file
        case "${SHELL:-}" in
            */zsh)  echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc" ;;
            */bash) echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc" ;;
            *)      echo "  # Add the export line to your shell config" ;;
        esac
    fi

    echo ""
    echo "Verify: monarch --version"
}

main "$@"
