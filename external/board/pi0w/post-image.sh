#!/bin/bash
#
# Build sdcard.img: a single FAT partition holding firmware, kernel,
# DTB and the initramfs. There is no root partition.
#
# Also builds mobix-update.tar, the payload "mobix update" installs on a
# running device (signed by "make release").
#

set -euo pipefail

BOARD_DIR="$(dirname "$0")"
GENIMAGE_TMP="${BUILD_DIR}/genimage.tmp"
FW_DIR="${BINARIES_DIR}/rpi-firmware"
UPDATE_DIR="${BINARIES_DIR}/update"
BOOT_DIR="${BINARIES_DIR}/boot"

ROOTPATH_TMP="$(mktemp -d)"
trap 'rm -rf "${ROOTPATH_TMP}"' EXIT

rm -rf "${GENIMAGE_TMP}" "${UPDATE_DIR}" "${BOOT_DIR}"

# Everything that lives in a boot slot, under slot-independent names.
# config.txt and cmdline.txt still contain the @SLOT@ placeholder.
install -d "${UPDATE_DIR}"
cp "${BINARIES_DIR}/zImage" "${BINARIES_DIR}/rootfs.cpio.lz4" "${UPDATE_DIR}/"
cp "${BINARIES_DIR}/bcm2708-rpi-zero-w.dtb" "${UPDATE_DIR}/mobix.dtb"
cp "${FW_DIR}/config.txt" "${FW_DIR}/cmdline.txt" "${UPDATE_DIR}/"
cp -r "${FW_DIR}/overlays" "${UPDATE_DIR}/overlays"
cp "${TARGET_DIR}/etc/mobix-release" "${UPDATE_DIR}/release"
tar -C "${UPDATE_DIR}" --sort=name --owner=0 --group=0 --numeric-owner \
	-cf "${BINARIES_DIR}/mobix-update.tar" .

# Boot partition of a fresh image: slot a is populated, slot b is empty.
# "mobix update" uses the same file names for the other slot.
install -d "${BOOT_DIR}"
cp "${FW_DIR}/bootcode.bin" "${FW_DIR}/start_cd.elf" "${FW_DIR}/fixup_cd.dat" "${BOOT_DIR}/"
sed 's/@SLOT@/a/g' "${UPDATE_DIR}/config.txt" > "${BOOT_DIR}/config.txt"
sed 's/@SLOT@/a/g' "${UPDATE_DIR}/cmdline.txt" > "${BOOT_DIR}/cmdline-a.txt"
cp "${UPDATE_DIR}/zImage" "${BOOT_DIR}/zImage-a"
cp "${UPDATE_DIR}/rootfs.cpio.lz4" "${BOOT_DIR}/rootfs-a.cpio.lz4"
cp "${UPDATE_DIR}/mobix.dtb" "${BOOT_DIR}/mobix-a.dtb"
cp -r "${UPDATE_DIR}/overlays" "${BOOT_DIR}/overlays-a"
# Example config files for the boot partition (edited there by the user)
install -m 0644 "${BOARD_DIR}"/boot-files/* "${BOOT_DIR}/"

genimage \
	--rootpath "${ROOTPATH_TMP}" \
	--tmppath "${GENIMAGE_TMP}" \
	--inputpath "${BINARIES_DIR}" \
	--outputpath "${BINARIES_DIR}" \
	--config "${BOARD_DIR}/genimage.cfg"

echo
echo "Image sizes:"
ls -lh "${BINARIES_DIR}"/zImage "${BINARIES_DIR}"/rootfs.cpio.lz4 \
	"${BINARIES_DIR}"/mobix-update.tar "${BINARIES_DIR}"/sdcard.img
