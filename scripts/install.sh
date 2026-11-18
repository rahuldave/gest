#!/bin/sh
# Install a release of gest from GitHub, with no Rust.
#
#   sh install.sh [--tag vX.Y.Z] [--dir DIR]
#
# The script maps uname -s and uname -m to a target, downloads gest-<target>.tar.gz and its .sha256 from the release,
# checks the hash, unpacks the archive into DIR (default ~/.local/bin) and prints the version. With no --tag, it
# installs the latest release. It needs only curl, tar, and shasum or sha256sum. Any failure exits non-zero.
set -eu

repo="rahuldave/gest"
bin="gest"
tag=""
dir="${HOME}/.local/bin"

die() {
    echo "install.sh: $*" >&2
    exit 1
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --tag)
            [ "$#" -ge 2 ] || die "--tag needs a value"
            tag="$2"
            shift 2
            ;;
        --dir)
            [ "$#" -ge 2 ] || die "--dir needs a value"
            dir="$2"
            shift 2
            ;;
        -h | --help)
            echo "usage: sh install.sh [--tag vX.Y.Z] [--dir DIR]"
            exit 0
            ;;
        *) die "unknown argument: $1" ;;
    esac
done

case "$(uname -s)" in
    Darwin) os="apple-darwin" ;;
    Linux) os="unknown-linux-musl" ;;
    *) die "no release for the system $(uname -s)" ;;
esac
case "$(uname -m)" in
    arm64 | aarch64) arch="aarch64" ;;
    x86_64 | amd64) arch="x86_64" ;;
    *) die "no release for the machine $(uname -m)" ;;
esac
target="${arch}-${os}"

if [ -z "$tag" ]; then
    latest=$(curl -fsSL "https://api.github.com/repos/${repo}/releases/latest") || die "cannot read the latest release"
    tag=$(printf '%s\n' "$latest" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
    [ -n "$tag" ] || die "the latest release has no tag"
fi

if command -v shasum >/dev/null 2>&1; then
    sha256() { shasum -a 256 "$1"; }
elif command -v sha256sum >/dev/null 2>&1; then
    sha256() { sha256sum "$1"; }
else
    die "found neither shasum nor sha256sum"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

asset="${bin}-${target}.tar.gz"
url="https://github.com/${repo}/releases/download/${tag}/${asset}"
curl -fsSL -o "${tmp}/${asset}" "$url" || die "cannot download $url"
curl -fsSL -o "${tmp}/${asset}.sha256" "${url}.sha256" || die "cannot download ${url}.sha256"

want=$(cut -d ' ' -f 1 "${tmp}/${asset}.sha256")
got=$(sha256 "${tmp}/${asset}" | cut -d ' ' -f 1)
[ -n "$want" ] && [ "$want" = "$got" ] || die "the hash of $asset does not match its .sha256"

mkdir "${tmp}/unpack"
tar -xzf "${tmp}/${asset}" -C "${tmp}/unpack" || die "cannot unpack $asset"
[ -f "${tmp}/unpack/${bin}" ] || die "$asset has no $bin at its top level"
mkdir -p "$dir"
# Put the new binary next to the old one, then rename it over the old one in one step. A copy over a running binary
# fails on Linux with "Text file busy", and an Apple silicon Mac can kill a signed binary that was changed in place.
cp "${tmp}/unpack/${bin}" "${dir}/.${bin}.new"
chmod +x "${dir}/.${bin}.new"
mv -f "${dir}/.${bin}.new" "${dir}/${bin}"
rm "${tmp}/unpack/${bin}"
cp -R "${tmp}/unpack/." "$dir/"
"${dir}/${bin}" --version
