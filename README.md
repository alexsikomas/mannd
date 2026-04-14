<div align="center">
    <h1 style="border-bottom: none; margin-bottom: 1px;">
        mannd
    </h1>
<i>Human-written software</i>
</div>

<div align="center">
  <video src="https://sikomas.com/public/mannd-git-demo.mp4" 
         autoplay 
         loop 
         muted 
         playsinline 
         style="max-width: 100%; border-radius: 10px; box-shadow: 0 10px 30px rgba(0,0,0,0.5);">
  </video>
</div>

## Why mannd?
`mannd` is an unopinionated manager for various networking daemons. This means if all you want is a cleaner interface for connecting to Wi-Fi networks with either `iwd` or `wpa_supplicant` you can do that.

If instead you just want to connect to your VPN through `WireGuard` you can just do that.

If you want to do any assortment of these things, you are free to do so.

### Features
#### Wi-Fi Management:
- Scan for nearby networks
- Connect to WPA2/WPA3 networks (via Wi-Fi daemons)
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
- Rust
- For Wi-Fi: `wpa_supplicant` or `iwd`
- For VPN: `wireguard-tools`
- For `networkd`: ...`networkd` as an active service
- Font Awesome Free (Optional)

*Note: Your Wi-Fi daemons only allow you to connect to a network but your PC internally needs an IP address assigned to your Wi-Fi interface. This means if you're using `networkd` without any `.network` rules you will need to create them either manually or through the TUI*

### Installation
```bash
git clone https://github.com/alexsikomas/mannd
cd mannd

# try out
./run.sh -t d

# install
./run.sh -i

# uninstall
./run.sh -u
```

## Roadmap

`mannd` is not yet feature complete, here is what's missing as of now:
- Connecting to EAP networks
- Config management for the daemons
- `systemd-networkd` management
- Consistent design
- Missing popups
- Popups reworked as non-intrusive notifications
    - Possibly animated

<hr style="visibility: hidden"/>
