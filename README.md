# mobix

Buildroot image for a Raspberry Pi Zero W that runs entirely from RAM:

- the whole rootfs is an LZ4 initramfs; the SD card only holds one FAT partition
- the FAT partition is mounted **read-only** on `/boot` and supplies the configuration
- signed updates from GitHub releases (`mobix update`), installed into a second boot slot
- [LPrint](https://www.msweet.org/lprint) (on PAPPL) drives USB label printers; web UI on port 8000
- Wi-Fi via wpa_supplicant + dhcpcd, auto-connect and reconnect; dual stack (IPv4 DHCP,
  IPv6 SLAAC/DHCPv6 with stable addresses derived from the MAC)
- a Rust app (`mobix-app`) is cross-built by Buildroot's cargo infrastructure and supervised by init
- [Typst](https://typst.app) command line compiler (`typst`) for rendering labels
- tuned for **< 10 s from power-on to network**

Buildroot 2026.08 is a git submodule (Typst needs its newer Rust); everything project specific lives in the
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

## Releases

Releases are built and signed locally with [minisign](https://jedisct1.github.io/minisign/) and
uploaded to GitHub; devices only install updates that carry a valid signature. The secret key must
never be in the repository or in CI: whoever has it can install software on every device.

Once, create the key pair (pick a passphrase, back up the secret key outside this machine) and
commit the public key, which is baked into every image:

```sh
make                   # builds output/host/bin/minisign
output/host/bin/minisign -G -s ~/.minisign/mobix.key \
    -p external/board/pi0w/rootfs-overlay/etc/mobix/update.pub
git add external/board/pi0w/rootfs-overlay/etc/mobix/update.pub
```

If the secret key is lost, deployed devices cannot be updated any more and need a reflash.

For each release, from a clean, tagged commit:

```sh
git tag v1.0.0
make release           # MINISIGN_KEY=<file> if the key is elsewhere
git push origin v1.0.0
gh release create v1.0.0 --verify-tag --generate-notes output/release/*
```

`make release` builds, signs and collects in `output/release/`:

| File | Purpose |
|---|---|
| `mobix-v1.0.img.xz` (+ `.minisig`) | full image for flashing a card |
| `mobix-update.tar` (+ `.minisig`) | what `mobix update` downloads; the name is fixed so the latest release is always found |

Flash a release image with:

```sh
xz -dc mobix-v1.0.img.xz | sudo dd of=/dev/sdX bs=4M conv=fsync
```

GitHub Actions (`.github/workflows/build.yml`) only checks that a tag builds from a clean
checkout; it publishes nothing.

## Updates

On the device, `mobix update` installs the latest release (`mobix update --check` only looks):

1. it fetches the signature of the latest release and stops if the release is not newer than the
   running image (this also prevents downgrades);
2. it downloads `mobix-update.tar` to RAM and verifies it against `/etc/mobix/update.pub`;
3. it writes kernel, initramfs, device tree, overlays and `cmdline.txt` into the boot slot that is
   not running, reads them back, and only then switches `config.txt` to that slot.

The running system is untouched; the new version starts after the next power cycle. The config
files on the card (`wpa_supplicant.conf`, `mobix.conf`, `authorized_keys`, `lprint.state`) are kept.

The card holds two slots, `a` and `b` (`zImage-a`, `rootfs-a.cpio.lz4`, `mobix-a.dtb`,
`overlays-a/`, `cmdline-a.txt`, and the same with `b`). A power loss during an update leaves the
old slot booting. To go back to the previous version, change the slot letter in the five slot
lines of `config.txt` on a PC. There is no automatic fallback if a new version boots but does not
work, and the GPU firmware files (`bootcode.bin`, `start_cd.elf`, `fixup_cd.dat`) are not updated;
changing those needs a reflash.

`mobix version` shows the running version and slot. Releases are fetched from
`https://github.com/allgoewer/mobix/releases/latest/download`; set `UPDATE_URL=` in `mobix.conf`
to use another location.

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
| `lprint.state` | optional, LPrint printer setup (written by `mobix save`) |
| `dropbear/` | optional, SSH host keys (written by `mobix save`) |

LPrint's state and the SSH host keys live in RAM. Configure the printer once in the web UI
(`http://<ip>:8000`), then run `mobix save` on the device (e.g. `ssh root@mobix mobix save`). It
copies `/var/lib/lprint.state` and the host keys to the FAT partition, from where they are
restored on every boot. Without saved host keys the device generates new ones at each boot and
SSH clients warn about a changed host key. Like the other files on the card, the keys are stored
unencrypted.

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
buildroot/                   Buildroot submodule (2026.08)
external/                    BR2_EXTERNAL "MOBIX"
  configs/mobix_pi0w_defconfig
  board/pi0w/                config.txt, cmdline.txt, genimage.cfg, kernel/busybox fragments,
                             post-build/post-image scripts, release.sh, rootfs-overlay/
                             (incl. the `mobix` command), boot-files/
  package/pappl, lprint      PAPPL 1.4.12, LPrint git 54f1c46 + ESC/POS patches (issue #222)
  package/mobix-app          the Rust app (cargo-package, git)
  package/typst              Typst 0.15.1 (cargo-package, needs Rust >= 1.92)
  patches/wpa_supplicant     fix for WPA2-PSK on brcmfmac (handshake done in firmware)
Makefile                     wrapper (O=output, BR2_EXTERNAL=external, dl/ cache)
```
