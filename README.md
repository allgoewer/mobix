# mobix

Buildroot image for a Raspberry Pi Zero W that runs entirely from RAM:

- the whole rootfs is an LZ4 initramfs; the SD card only holds one FAT partition
- the FAT partition is mounted **read-only** on `/boot` and supplies the configuration
- [LPrint](https://www.msweet.org/lprint) (on PAPPL) drives USB label printers; web UI on port 8000
- Wi-Fi via wpa_supplicant + dhcpcd, auto-connect and reconnect; dual stack (IPv4 DHCP,
  IPv6 SLAAC/DHCPv6 with stable addresses derived from the MAC)
- a Rust app (`mobix-app`) is cross-built by Buildroot's cargo infrastructure and supervised by init
- tuned for **< 10 s from power-on to network**

Buildroot 2026.02.3 (LTS) is a git submodule; everything project specific lives in the
`external/` BR2_EXTERNAL tree.

## Build

```sh
git submodule update --init
make                   # first run applies mobix_pi0w_defconfig
# -> output/images/sdcard.img
```

Other targets are passed through to Buildroot: `make menuconfig`, `make linux-menuconfig`,
`make lprint-rebuild all`, … After `menuconfig`, run `make savedefconfig` to write the change
back to `external/configs/mobix_pi0w_defconfig`. Downloads are cached in `dl/`.

## Rust app

The app comes from its own git repository (the repo must contain `Cargo.lock`, the build uses `--locked`):

```sh
make menuconfig   # External options -> mobix-app: git URL, revision, binary name
make savedefconfig && make
```

Use a commit hash or tag as revision; a branch name is only fetched once and then cached in `dl/`.
The binary is installed as `/usr/bin/mobix-app` and started by `/etc/inittab` (respawned if it exits).
Its config file path is passed in `$MOBIX_CONFIG` (`/boot/mobix.conf`).

For development against a local checkout: `cp local.mk.example local.mk`, adjust the path, then
`make mobix-app-rebuild all`.

## Flash and configure

```sh
sudo dd if=output/images/sdcard.img of=/dev/sdX bs=4M conv=fsync
```

Then edit these files on the FAT partition:

| File | Purpose |
|---|---|
| `wpa_supplicant.conf` | Wi-Fi networks and `country=` (copied to `/etc` at boot) |
| `mobix.conf` | hostname, NTP server; also read by the app |
| `authorized_keys` | optional, root SSH keys for dropbear |
| `lprint.state` | optional, LPrint printer setup (see below) |

LPrint's state lives in RAM. Configure the printer once in the web UI (`http://<ip>:8000`), then
run `mobix-save-state` on the device (e.g. `ssh root@mobix mobix-save-state`). It copies
`/var/lib/lprint.state` to the FAT partition, and it will be restored on every boot.

Serial console: GPIO14/15, 115200 8N1, login `root` (no password — set one or remove the getty
in `external/board/pi0w/rootfs-overlay/etc/inittab` for production).

## Boot time

Budget (power-on → DHCP lease): firmware ~2 s, kernel + initramfs ~1.5 s, userspace ~0.3 s,
Wi-Fi association 1.5–3 s, DHCP 0.3–1 s.

What makes it fast:

- cut-down GPU firmware, `boot_delay=0`, no splash, turbo during boot, Bluetooth off (`config.txt`)
- LZ4 kernel and initramfs; the rootfs is trimmed in `post-build.sh` (CUPS spooler, unused
  firmware and ~all kernel modules removed)
- Wi-Fi driver, cfg80211 and VFAT built into the kernel; mdev only for hotplug, no coldplug on the
  network path
- `S00boot` and `S01network` run first; wpa_supplicant and dhcpcd daemonize immediately
- dhcpcd (DHCP only, no static IP): no start delay, no ARP probe, rapid commit; IPv6 is set up
  in parallel and does not delay the IPv4 lease
- set `scan_freq=` in `wpa_supplicant.conf` to skip the full channel scan

Measure: the console prints `mobix: network up on wlan0 (...) after N s uptime` (from
`/lib/dhcpcd/dhcpcd-hooks/99-boottime`); uptime starts at the kernel, add the firmware stage.
For the full picture from power-on, capture the serial console with timestamps
(`uart_2ndstage=1` is enabled in `config.txt`):

```sh
grabserial -d /dev/ttyUSB0 -b 115200 -t -m 'Raspberry Pi Bootcode'
```

Further knobs: `lpj=` on `cmdline.txt` (value from `dmesg | grep lpj`) skips delay-loop calibration;
`initcall_debug` shows slow kernel initcalls.

## Layout

```
buildroot/                   Buildroot submodule (2026.02.3)
external/                    BR2_EXTERNAL "MOBIX"
  configs/mobix_pi0w_defconfig
  board/pi0w/                config.txt, cmdline.txt, genimage.cfg, kernel/busybox fragments,
                             post-build/post-image scripts, rootfs-overlay/, boot-files/
  package/pappl, lprint      PAPPL 1.4.12, LPrint git 54f1c46 + ESC/POS patches (issue #222)
  package/mobix-app          the Rust app (cargo-package, git)
Makefile                     wrapper (O=output, BR2_EXTERNAL=external, dl/ cache)
```
