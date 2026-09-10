#!/bin/sh
#
# Installs the latest release of whisker.
#
#   curl -LsSf https://aonyx-ai.github.io/whisker/install.sh | sh
#
# The line above runs this in whichever /bin/sh the machine has, and on
# Debian that is dash, so the script is POSIX shell rather than bash.

set -eu

REPOSITORY="aonyx-ai/whisker"

# Every transfer goes through here. `--proto =https` refuses plain http,
# also after a redirect, so no hop can move a download onto a channel that
# someone else can rewrite. The digest needs the same guard as the archive
# it checks, because a downgrade of both would pass the check.
download() {
    curl --proto '=https' --tlsv1.2 --retry 3 \
        --fail --location --silent --show-error "$@"
}

main() {
    os="$(uname -s)"
    architecture="$(uname -m)"

    # Under Rosetta, uname reports x86_64 on arm64 hardware that runs the
    # arm64 binary. The kernel answers for the hardware.
    if [ "${os}-${architecture}" = "Darwin-x86_64" ] &&
        (sysctl hw.optional.arm64 2> /dev/null || true) | grep -q ': 1'; then
        architecture="arm64"
    fi

    case "${os}-${architecture}" in
        Linux-x86_64) target="x86_64-unknown-linux-gnu" ;;
        Linux-aarch64) target="aarch64-unknown-linux-gnu" ;;
        Darwin-arm64) target="aarch64-apple-darwin" ;;
        *)
            echo "whisker publishes no binary for ${os} on ${architecture}" >&2
            exit 1
            ;;
    esac

    # The Linux binary links against glibc. On musl it installs, then fails
    # with "not found", because the interpreter it names is missing.
    if [ "${os}" = "Linux" ] && ldd --version 2>&1 | grep -qi musl; then
        echo "whisker publishes no binary for musl systems such as Alpine" >&2
        exit 1
    fi

    # `releases/latest` names the newest release that is not a prerelease,
    # and answers 404 while a repository has only prereleases. `--fail`
    # stops here in that case, not at the download of an archive for a
    # missing version.
    #
    # The parse is a separate statement because a pipeline reports the
    # status of its last command. A failed `curl` inside one would let `sed`
    # report success over an empty version.
    release="$(download "https://api.github.com/repos/${REPOSITORY}/releases/latest")"
    version="$(printf '%s' "${release}" |
        sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"

    # A network that answers for api.github.com, such as a captive portal or
    # an inspecting proxy, returns 200 and a page of its own. The parse finds
    # no name in it, and the download would then ask for version "".
    if [ -z "${version}" ]; then
        echo "cannot read a release name from GitHub's answer" >&2
        exit 1
    fi

    # Archive names carry the version without its leading `v`, as
    # `Cargo.toml` holds it. The same name is the directory the archive
    # unpacks to.
    stem="whisker-${version#v}-${target}"
    name="${stem}.tar.gz"
    base="https://github.com/${REPOSITORY}/releases/download/${version}"

    # The staged copy sits in the destination directory, not beside the
    # download, so the rename below stays within one filesystem.
    directory="${WHISKER_INSTALL_DIR:-${HOME}/.local/bin}"
    mkdir -p "${directory}"
    work="$(mktemp -d "${TMPDIR:-/tmp}/whisker-install.XXXXXX")"
    staged="${directory}/whisker.$$.tmp"
    trap 'rm -rf "${work}" "${staged}"' EXIT

    echo "Downloading whisker ${version} for ${target}"
    download -o "${work}/${name}" "${base}/${name}"
    download -o "${work}/${name}.sha256" "${base}/${name}.sha256"

    # The digest names the archive without a path, so the check runs in the
    # directory that holds both. Linux ships `sha256sum` and macOS `shasum`.
    if command -v sha256sum > /dev/null 2>&1; then
        (cd "${work}" && sha256sum -c "${name}.sha256") > /dev/null
    else
        (cd "${work}" && shasum -a 256 -c "${name}.sha256") > /dev/null
    fi

    tar -xzf "${work}/${name}" -C "${work}"

    # A rename replaces a running whisker. Linux refuses a copy onto one
    # with ETXTBSY.
    cp "${work}/${stem}/whisker" "${staged}"
    chmod 755 "${staged}"
    mv "${staged}" "${directory}/whisker"

    echo "Installed ${directory}/whisker"
}

# The body is one function that the last line calls, so a half-downloaded
# script runs nothing at all.
main
