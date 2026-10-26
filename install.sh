#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf 'Usage: bash install.sh <repository-url>\n'
    printf 'Installs opencode-profile from the supplied Git repository using Cargo.\n'
}

if [[ $# -eq 1 && ( $1 == --help || $1 == -h ) ]]; then
    usage
    exit 0
fi

if [[ $# -ne 1 || -z ${1-} || ${1-} == -* ]]; then
    usage >&2
    exit 1
fi
repo=$1

for tool in cargo git; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Error: %s is required; install it before running this script.\n' "$tool" >&2
        exit 1
    fi
done

if [[ -n ${CARGO_INSTALL_ROOT:-} ]]; then
    install_root=$CARGO_INSTALL_ROOT
elif [[ -n ${CARGO_HOME:-} ]]; then
    install_root=$CARGO_HOME
elif [[ -n ${HOME:-} ]]; then
    install_root=$HOME/.cargo
else
    printf 'Error: set CARGO_INSTALL_ROOT, CARGO_HOME, or HOME to locate the install directory.\n' >&2
    exit 1
fi

printf 'Expected binary directory: %s/bin\n' "$install_root"
cargo install --locked --git "$repo" opencode-profile
printf 'Installation completed. Ensure %s/bin is on your PATH.\n' "$install_root"
