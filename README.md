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

### wpa_supplicant
`mannd` communicates with `wpa_supplicant` through the D-Bus control interface, this has to be explicitly enabled on your main Wi-Fi adapter. This is not always the default.

Find your main interface name with: `ip a`

Check if `wpa_supplicant` is running with the correct flags already by using:
```bash
ps aux | grep wpa_supplicant
```

Note: below `interface` is used as a placeholder you should expect your main interface name there.

Correct output:
```bash
root    210470  0.0  0.0  17168 11192 ? Ss 16:50  0:00 /usr/bin/wpa_supplicant -c/etc/wpa_supplicant/wpa_supplicant-interface.conf -iinterface -u
```

Bad output (missing `-u` flag):
```bash
root    682  0.0  0.0  16260  5504 ? Ss Nov08 0:00 /usr/bin/wpa_supplicant -u -s -O /run/wpa_supplicant
root    715  0.0  0.0  17164 10972 ? Ss Nov08 0:01 /usr/bin/wpa_supplicant -c/etc/wpa_supplicant/wpa_supplicant-interface.conf -iinterface
```
While there is a `-u` flag present it is for a general `wpa_supplicant` service not for our main Wi-Fi interface. The main interface is run on the second line, in-fact if this had a `-u` flag we would run into an error as only one can exist at a time.

Remove the `-u` flags from any `wpa_supplicant` service that isn't you main one. You will find the flag on the `ExecStart` section.

Edit your `.service` file with:
```bash
 sudo systemctl edit wpa_supplicant@interface.service
```
or
```bash
sudo [your favourite editor] /etc/systemd/system/multi-user.target.wants/wpa_supplicant@interface.service
```


Finally run:
```bash
sudo systemctl daemon-reload
sudo systemctl restart wpa_supplicant@interface.service
```

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
