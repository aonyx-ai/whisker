#!/bin/sh
#
# Installs the latest release of whisker.
#
#   curl -LsSf https://aonyx-ai.github.io/whisker/install.sh | sh
#
# This is POSIX shell rather than bash, because the line above pipes it into
# whichever /bin/sh the machine has, and on Debian that is dash.

set -eu

REPOSITORY="aonyx-ai/whisker"

main() {
    os="$(uname -s)"
    architecture="$(uname -m)"

    case "${os}-${architecture}" in
        Linux-x86_64) target="x86_64-unknown-linux-gnu" ;;
        Linux-aarch64) target="aarch64-unknown-linux-gnu" ;;
        Darwin-arm64) target="aarch64-apple-darwin" ;;
        *)
            echo "whisker publishes no binary for ${os} on ${architecture}" >&2
            exit 1
            ;;
    esac

    # GitHub reserves `releases/latest` for the newest release that is not a
    # prerelease, and answers 404 while a repository has published only
    # prereleases. The `--fail` stops here in that case, rather than at the
    # download of an archive named after a version that does not exist.
    #
    # The parse is a second statement because a shell reports the status of
    # the last command in a pipeline. A `curl` that failed inside one would
    # leave `sed` to report success over an empty version.
    release="$(curl -fLsS \
        "https://api.github.com/repos/${REPOSITORY}/releases/latest")"
    version="$(printf '%s' "${release}" |
        sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"

    # The archive names carry the version without its leading `v`, because
    # that is what `Cargo.toml` holds. The same name spells the directory
    # the archive unpacks to.
    stem="whisker-${version#v}-${target}"
    name="${stem}.tar.gz"
    base="https://github.com/${REPOSITORY}/releases/download/${version}"

    # The staged copy sits in the destination directory and not beside the
    # download, so that the rename below stays within one filesystem.
    directory="${WHISKER_INSTALL_DIR:-${HOME}/.local/bin}"
    mkdir -p "${directory}"
    work="$(mktemp -d "${TMPDIR:-/tmp}/whisker-install.XXXXXX")"
    staged="${directory}/whisker.$$.tmp"
    trap 'rm -rf "${work}" "${staged}"' EXIT

    echo "Downloading whisker ${version} for ${target}"
    curl -fLsS -o "${work}/${name}" "${base}/${name}"
    curl -fLsS -o "${work}/${name}.sha256" "${base}/${name}.sha256"

    # The digest names the archive without a path, so the check runs from the
    # directory holding both. Linux carries `sha256sum` and macOS `shasum`.
    if command -v sha256sum > /dev/null 2>&1; then
        (cd "${work}" && sha256sum -c "${name}.sha256") > /dev/null
    else
        (cd "${work}" && shasum -a 256 -c "${name}.sha256") > /dev/null
    fi

    tar -xzf "${work}/${name}" -C "${work}"

    # A rename replaces a whisker that is running. Copying onto one instead
    # is what Linux refuses with ETXTBSY.
    cp "${work}/${stem}/whisker" "${staged}"
    chmod 755 "${staged}"
    mv "${staged}" "${directory}/whisker"

    echo "Installed ${directory}/whisker"
}

# The body is one function that the last line calls, so that a download this
# shell has read only half of runs nothing at all.
main
