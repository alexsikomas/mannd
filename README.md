<h1 align="center">
    mannd
</h1>

## Why mannd?
`systemd-networkd` is a powerful and lightweight networking daemon, but managing Wi-Fi or on-the-fly VPN connections often requires manual configurations or switching between various userspace tools like `iwctl` and `wg-quick`. `mannd` aims to condense the process into just one TUI, similar to `nmtui`.

### Features
#### Wi-Fi Management:
- Scan for nearby networks
- Connect to WPA2/WPA3 networks
- Manage and switch between saved networks
- Forget known networks
#### VPN
- Add WireGuard configurations from file or folder(s)
- Activates and deactivates VPN connection
#### Supported daemons
- Wi-Fi: `wpa_supplicant`, `iwd`
- VPN: `WireGuard`

## Prerequisites
Before you can use `mannd`, you must have the following installed on your system:
- `systemd-networkd`
- Rust
- For Wi-Fi: `wpa_supplicant` or `iwd`
- For VPN: `wireguard-tools`

`mannd` expects that a `.network` rule already exists for allowing you to connect to the Internet via Wi-Fi.

### Installation
#### Source
```bash
git clone https://github.com/alexsikomas/mannd
cd mannd

# If you don't have UPX installed remove the opt keyword
run.sh -t r opt
sudo mv ./target/release/tui /usr/local/bin/mannd
```

<hr style="visibility: hidden"/>
