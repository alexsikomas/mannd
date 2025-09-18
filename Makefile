build_tui_debug:
	cargo build --package tui
	sudo setcap cap_net_admin,cap_dac_override=ep ./target/debug/tui

run_tui_debug: build_tui_debug
	./target/debug/tui

tui_release:
	cargo build --release --package tui
	sudo setcap cap_net_admin,cap_dac_override=ep ./target/debug/tui

build_gui_debug:
	cargo build --package gui
	sudo setcap cap_net_admin,cap_dac_override=ep ./target/debug/gui

run_gui_debug: build_gui_debug
	./target/debug/gui

gui_release:
	cargo build --release --package gui
	sudo setcap cap_net_admin,cap_dac_override=ep ./target/debug/gui

test_lib:
	$(eval LIB_TEST_BIN := $(shell cargo test -p com --no-run --message-format=json | \
		jq -s -r 'map(select(.profile.test == true and .target.name == "com")) | .[-1].filenames[] | select(endswith(".dSYM") | not)'))

	sudo setcap cap_net_admin,cap_dac_override=ep $(LIB_TEST_BIN)
	"$(LIB_TEST_BIN)" --nocapture

clean:
	cargo clean
