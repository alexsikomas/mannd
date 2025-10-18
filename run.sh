#!/bin/bash

dbus_dir="/usr/share/dbus-1/system.d/"

display_help() {
    cat <<-EOF
    Usage: $(basename "$0") [OPTIONS] <argument>

    This is a utility to run & install mannd.

    Options:
        -h, --help      Displays this help message and exits.
        -t, --tui [d|r|debug|release] [opt]
                        Compiles and runs the TUI debug or release build.
                        'd' or 'debug' is the default build.
                        'r' or 'release' creates the release build.
                        'opt' can be added to optimise the release.

        -c  --com       Compiles and tests the com package in debug mode.
        -i  --install   Installs the TUI.

    Examples:
        Run the TUI in default (debug) mode:
        $(basename "$0") -t

        Run the TUI in release mode:
        $(basename "$0") -t release

        Run the TUI in release mode with optimisation:
        $(basename "$0") -t r opt

        Compile and test the com package in debug mode:
        $(basename "$0") -c

        Install the TUI:
        $(basename "$0") -i
EOF
}

tui() {
    echo "Building TUI..."
    # Release
    if [[ "$1" == "r" ]] || [[ "$1" == "release" ]]; then
        cargo build --release --package tui
        sudo setcap cap_net_admin,cap_dac_override=ep ./target/release/tui
        if [[ "$2" == "opt" ]] || [[ "$2" == "optimise" ]] then
            upx --best --lzma ./target/release/tui
        fi
        ./target/release/tui
        exit 0
    fi

   # Debug 
   cargo build --package tui
   sudo setcap cap_net_admin,cap_dac_override=ep ./target/debug/tui
   ./target/debug/tui
}

com() {
    LIB_TEST_BIN=$(cargo test -p com --no-run --message-format=json | \
        jq -s -r 'map(select(.profile.test == true and .target.name == "com")) | .[-1].filenames[] | select(endswith(".dSYM") | not)')

    if [[ -z "$LIB_TEST_BIN" ]] || [[ ! -f "$LIB_TEST_BIN" ]]; then
        echo "Error: Could not find the test binary for the com package."
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

install() {
    echo "install"
}

if [[ $# -eq 0 ]]; then
    echo "Error: No argument provided, use -h or --help for usage."
    exit 1
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            display_help 
            exit 0
            ;;
        -t|--tui)
            shift
            tui "$@"
            exit 0
            ;;
        -c|--com)
            shift
            com  "$0"
            exit 0
            ;;
        -i|--install)
            install 
            exit 0
            ;;
    esac
done
