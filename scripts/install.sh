#!/usr/bin/env sh
set -eu

REPO="${MAD_REPO:-ngoclinh93qt/MarkApiDown}"
BIN_NAME="mad"
INSTALL_DIR="${MAD_INSTALL_DIR:-}"
VERSION="latest"

usage() {
  cat <<'EOF'
Install MarkApiDown.

Usage:
  install.sh [--version=<tag>]

Environment:
  MAD_REPO         GitHub repo, default ngoclinh93qt/MarkApiDown
  MAD_INSTALL_DIR  Install directory override
EOF
}

for arg in "$@"; do
  case "$arg" in
    --version=*) VERSION="${arg#--version=}" ;;
    --help|-h) usage; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "required command not found: $1" >&2
    exit 1
  }
}

need uname
need curl
need tar

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux) os_slug="unknown-linux-musl" ;;
  Darwin) os_slug="apple-darwin" ;;
  *) echo "unsupported OS: $os" >&2; exit 1 ;;
esac

case "$arch" in
  x86_64|amd64) arch_slug="x86_64" ;;
  arm64|aarch64) arch_slug="aarch64" ;;
  *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac

target="${arch_slug}-${os_slug}"
archive="mad-${target}.tar.xz"

if [ "$VERSION" = "latest" ]; then
  base_url="https://github.com/${REPO}/releases/latest/download"
else
  base_url="https://github.com/${REPO}/releases/download/${VERSION}"
fi

if [ -z "$INSTALL_DIR" ]; then
  if [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="${HOME}/.local/bin"
  fi
fi

tmp="${TMPDIR:-/tmp}/mad-install.$$"
mkdir -p "$tmp"
trap 'rm -rf "$tmp"' EXIT INT TERM

echo "Downloading ${archive}..."
curl -fsSL "${base_url}/${archive}" -o "${tmp}/${archive}"

if curl -fsSL "${base_url}/${archive}.sha256" -o "${tmp}/${archive}.sha256"; then
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$tmp" && sha256sum -c "${archive}.sha256")
  elif command -v shasum >/dev/null 2>&1; then
    expected="$(awk '{print $1}' "${tmp}/${archive}.sha256")"
    actual="$(shasum -a 256 "${tmp}/${archive}" | awk '{print $1}')"
    [ "$expected" = "$actual" ] || {
      echo "checksum mismatch" >&2
      exit 1
    }
  else
    echo "warning: no SHA256 tool found; skipping checksum verification" >&2
  fi
else
  echo "warning: checksum file not found; skipping checksum verification" >&2
fi

tar -xJf "${tmp}/${archive}" -C "$tmp"
bin_path="$(find "$tmp" -type f -name "$BIN_NAME" | head -n 1)"
[ -n "$bin_path" ] || {
  echo "archive did not contain ${BIN_NAME}" >&2
  exit 1
}

mkdir -p "$INSTALL_DIR"
install -m 0755 "$bin_path" "${INSTALL_DIR}/${BIN_NAME}"

if ! command -v "$BIN_NAME" >/dev/null 2>&1 && ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
  echo "Installed to ${INSTALL_DIR}/${BIN_NAME}"
  echo "Add this to your PATH: export PATH=\"${INSTALL_DIR}:\$PATH\""
else
  "${INSTALL_DIR}/${BIN_NAME}" version
fi
