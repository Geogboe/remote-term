#!/bin/sh
set -eu

REPO="Geogboe/remote-term"
BINARY="rterm"
PREFIX="RTERM"

log() {
  if [ "${DEBUG:-}" = "1" ]; then
    echo "debug: $*" >&2
  fi
}

die() {
  echo "error: $*" >&2
  exit 1
}

get_env() {
  eval "printf '%s' \"\${$1:-}\""
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required"
}

curl_fetch() {
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    curl -H "Authorization: Bearer ${GITHUB_TOKEN}" "$@"
  elif [ -n "${GH_TOKEN:-}" ]; then
    curl -H "Authorization: Bearer ${GH_TOKEN}" "$@"
  else
    curl "$@"
  fi
}

has_github_token() {
  [ -n "${GITHUB_TOKEN:-}" ] || [ -n "${GH_TOKEN:-}" ]
}

github_asset_api_url() {
  name="$1"
  curl_fetch -fsSL "https://api.github.com/repos/${REPO}/releases/tags/${TAG}" \
    | awk -v wanted="$name" '
      /"url":/ {
        url = $0
        sub(/^[[:space:]]*"url":[[:space:]]*"/, "", url)
        sub(/",?[[:space:]]*$/, "", url)
      }
      /"name":/ {
        name = $0
        sub(/^[[:space:]]*"name":[[:space:]]*"/, "", name)
        sub(/",?[[:space:]]*$/, "", name)
        if (name == wanted) {
          print url
          exit
        }
      }
    '
}

download_asset() {
  name="$1"
  path="$2"
  if has_github_token; then
    asset_url="$(github_asset_api_url "$name")"
    [ -n "$asset_url" ] || die "release asset not found: $name"
    curl_fetch -H "Accept: application/octet-stream" -fsSL -o "$path" "$asset_url"
  else
    curl_fetch -fsSL -o "$path" "${BASE_URL}/${name}"
  fi
}

DEBUG="$(get_env "${PREFIX}_DEBUG")"
if [ -z "$DEBUG" ]; then
  DEBUG="${INSTALLER_DEBUG:-}"
fi

case "$(uname -s)" in
  Linux) OS="linux" ;;
  Darwin) OS="darwin" ;;
  *) die "unsupported OS: $(uname -s)" ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) ARCH="amd64" ;;
  arm64 | aarch64) ARCH="arm64" ;;
  *) die "unsupported architecture: $(uname -m)" ;;
esac

require_cmd curl
require_cmd tar

TAG="$(get_env "${PREFIX}_VERSION")"
if [ -z "$TAG" ]; then
  TAG="${INSTALLER_VERSION:-}"
fi
if [ -z "$TAG" ]; then
  TAG="$(
    curl_fetch -fsSL "https://api.github.com/repos/${REPO}/releases?per_page=10" \
      | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' \
      | head -n 1
  )"
fi
[ -n "$TAG" ] || die "could not find a GitHub release; set ${PREFIX}_VERSION or INSTALLER_VERSION"

ARCHIVE="${BINARY}_${TAG}_${OS}_${ARCH}.tar.gz"
BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

log "release tag: $TAG"
log "archive: $ARCHIVE"

download_asset "$ARCHIVE" "${TMP}/${ARCHIVE}"
download_asset "checksums.txt" "${TMP}/checksums.txt"

CHECKSUM_LINE="$(grep "[[:space:]]${ARCHIVE}$" "${TMP}/checksums.txt" || true)"
[ -n "$CHECKSUM_LINE" ] || die "checksum not found for ${ARCHIVE}"

if command -v sha256sum >/dev/null 2>&1; then
  printf '%s\n' "$CHECKSUM_LINE" | (cd "$TMP" && sha256sum --check --status)
elif command -v shasum >/dev/null 2>&1; then
  printf '%s\n' "$CHECKSUM_LINE" | (cd "$TMP" && shasum -a 256 --check --status)
else
  die "sha256sum or shasum is required"
fi

tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP" "$BINARY"

INSTALL_DIR="$(get_env "${PREFIX}_INSTALL_DIR")"
if [ -z "$INSTALL_DIR" ]; then
  INSTALL_DIR="${INSTALLER_INSTALL_DIR:-}"
fi
if [ -z "$INSTALL_DIR" ]; then
  if [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="${HOME}/.local/bin"
  fi
fi

FORCE="$(get_env "${PREFIX}_FORCE")"
if [ -z "$FORCE" ]; then
  FORCE="${INSTALLER_FORCE:-}"
fi

DEST="${INSTALL_DIR}/${BINARY}"
if [ -e "$DEST" ] && [ "$FORCE" != "1" ]; then
  echo "${BINARY} is already installed at ${DEST}"
  echo "Set ${PREFIX}_FORCE=1 or INSTALLER_FORCE=1 to reinstall."
else
  mkdir -p "$INSTALL_DIR"
  install -m 0755 "${TMP}/${BINARY}" "$DEST"
fi

[ -x "$DEST" ] || die "installed binary is not executable: $DEST"

case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo
    echo "The binary was installed to ${INSTALL_DIR}, but that directory is not in your PATH yet."
    echo "Run the command below, then open a new shell or source your profile and retry:"
    case "${SHELL:-}" in
      */zsh) echo "echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc" ;;
      */fish) echo "set -U fish_user_paths ${INSTALL_DIR} \$fish_user_paths" ;;
      *) echo "echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc" ;;
    esac
    ;;
esac

"$DEST" --version
echo "installed ${BINARY} to ${DEST}"
