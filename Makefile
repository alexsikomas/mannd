build_tui_debug:
	cargo build --package tui
	sudo setcap cap_net_admin+ep ./target/debug/tui

run_tui_debug: build_tui_debug
	./target/debug/tui

tui_release:
	cargo build --release --package tui
	sudo setcap cap_net_admin+ep ./target/debug/tui

build_gui_debug:
	cargo build --package gui
	sudo setcap cap_net_admin+ep ./target/debug/gui

run_gui_debug: build_gui_debug
	./target/debug/gui

gui_release:
	cargo build --release --package gui
	sudo setcap cap_net_admin+ep ./target/debug/gui

test_lib:
	cargo test -p nd_common -- --nocapture

clean:
	cargo clean
