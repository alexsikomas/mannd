#!/bin/bash
set -euo pipefail

BIN_DIR="/usr/local/bin"
SOCK_DIR="/usr/libexec"

display_help() {
    cat <<-EOF
    Usage: $(basename "$0") [OPTIONS] <argument>

    This is a utility to run & install mannd.

    Options:
        -h, --help      Displays this help message and exits.
        -t, --tui [d|r|debug|release]
                        Compiles and runs the TUI debug or release build.
                        'd' or 'debug' is the default build.
                        'r' or 'release' creates the release build.
        -m  --mannd     Compiles and tests the core mannd networking package in debug mode.
        -i  --install   Installs mannd.
        -u  --uninstall Uninstalls mannd.
        -s  --startup   Enable the mannd startup service

    Examples:
        Run the TUI in default (debug) mode:
        $(basename "$0") -t

        Run the TUI in release mode:
        $(basename "$0") -t release

        Compile and test the mannd package in debug mode:
        $(basename "$0") -m

        Install mannd:
        $(basename "$0") -i

        Uninstall mannd:
        $(basename "$0") -u
EOF
}

build() {
    echo "Building TUI..."
    if [[ "${1:-}" == "r" ]] || [[ "${1:-}" == "release" ]]; then
        cargo build --release || { echo "Error: Could not build in release mode." >&2; return 1; }
    else
        cargo build || { echo "Error: Could not build in debug mode." >&2; return 1; }
    fi

}

tui() {
    make_config
    # Release
    if [[ "$1" == "r" ]] || [[ "$1" == "release" ]]; then
        build "r"
        ./target/release/tui
    else
        build
       ./target/debug/tui
    fi
}

make_config() {
    local config_dir="${XDG_CONFIG_HOME:-${HOME:-}/.config}/mannd"

    if [ "${config_dir}" = "/mannd" ] || [ -z "${HOME:-}" ]; then
        echo "Error: Unable to determine config directory." >&2
        return 2
    fi

    mkdir -p "${config_dir}" || { echo "Error: Failed to create config dir" >&2; return 1; }
    if [ ! -f "${config_dir}/settings.conf" ]; then
        cp ./etc/settings.conf "${config_dir}/settings.conf" || { echo "Error: Failed to copy config to directory" >&2; return 1; }
        echo "Config created at ${config_dir}/settings.conf"
    fi
}

mannd() {
    command -v jq >/dev/null 2>&1 || { echo >&2 "Error: jq is required but it's not installed. Aborting."; exit 1; }
    LIB_TEST_BIN=$(cargo test -p mannd --no-run --message-format=json | \
        jq -s -r 'map(select(.profile.test == true and .target.name == "mannd")) | .[-1].filenames[] | select(endswith(".dSYM") | not)') || true

    if [[ -z "$LIB_TEST_BIN" ]] || [[ ! -f "$LIB_TEST_BIN" ]]; then
        echo "Error: Could not find the test binary for the core mannd package."
        return 1
    fi

    echo "Setting capabilities..."

    if ! sudo setcap cap_net_admin,cap_dac_override=ep "$LIB_TEST_BIN"; then
        echo "Error: Failed to set capabilities. Make sure you have sudo privileges."
        return 1
    fi

    echo "Running tests..."
    "$LIB_TEST_BIN" --nocapture
}

startup() {
    echo "startup"
}

install() {
    build "r"
    sudo cp ./target/release/tui "${BIN_DIR}/mannd" || { echo "Error: Failed to copy mannd to directory" >&2; return 1; }
    sudo cp ./target/release/socket "${SOCK_DIR}/mannd-socket" || { echo "Error: Failed to copy mannd socket to directory" >&2; return 1; }
    echo "Successfully installed mannd"
}

uninstall() {
    if [[ -f "${BIN_DIR}/mannd" ]]; then
        sudo rm "${BIN_DIR}/mannd"
    else
        echo "The mannd executable could not be found at ${BIN_DIR}/mannd, was it moved?" >&2
    fi

    if [[ -f "${SOCK_DIR}/mannd-socket" ]]; then
        sudo rm "${SOCK_DIR}/mannd-socket"
    else
        echo "The mannd-socket executable could not be found at ${SOCK_DIR}/mannd-socket, was it moved?" >&2
    fi

    echo "Uninstalled... :("
}

if [[ $# -eq 0 ]]; then
    echo "Error: No argument provided, use -h or --help for usage."
    exit 1
fi

RUN_TUI=false
TUI_MODE="d"
RUN_MANND_CORE=false
RUN_INSTALL=false
RUN_UNINSTALL=false
RUN_STARTUP=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            display_help 
            exit 0
            ;;
        -t|--tui)
            RUN_TUI=true
            if [[ $# -gt 1 && -n "$2" && "$2" != -* ]]; then
                TUI_MODE="$2"
                shift 2
            else
                shift 1
            fi
            ;;
        -m|--mannd)
            RUN_MANND_CORE=true
            shift
            ;;
        -i|--install)
            RUN_INSTALL=true 
            shift
            ;;
        -u|--uninstall)
            RUN_UNINSTALL=true
            shift
            ;;
        -s|--startup)
            RUN_STARTUP=true
            shift
            ;;
        -[a-zA-Z0-9]?*)
            split_args=()
            for ((i=1; i<${#1}; i++)); do
                split_args+=("-${1:$i:1}")
            done
            shift
            set -- "${split_args[@]}" "$@"
            ;;
        *)
            echo "Error: Unkown option $1"
            display_help 
            exit 1
            ;;
    esac
done

if [[ "$RUN_UNINSTALL" == true ]]; then
    uninstall
fi

if [[ "$RUN_INSTALL" == true ]]; then
    install
fi

if [[ "$RUN_STARTUP" == true ]]; then
    startup
fi

if [[ "$RUN_MANND_CORE" == true ]]; then
    mannd
fi

if [[ "$RUN_TUI" == true ]]; then
    tui "$TUI_MODE"
fi
