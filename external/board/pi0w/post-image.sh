#!/bin/bash
#
# Build sdcard.img: a single FAT partition holding firmware, kernel,
# DTB and the initramfs. There is no root partition.
#

set -euo pipefail

BOARD_DIR="$(dirname "$0")"
GENIMAGE_TMP="${BUILD_DIR}/genimage.tmp"

ROOTPATH_TMP="$(mktemp -d)"
trap 'rm -rf "${ROOTPATH_TMP}"' EXIT

rm -rf "${GENIMAGE_TMP}"

# Example config files for the boot partition (edited there by the user)
install -d "${BINARIES_DIR}/boot-files"
install -m 0644 "${BOARD_DIR}"/boot-files/* "${BINARIES_DIR}/boot-files/"

genimage \
	--rootpath "${ROOTPATH_TMP}" \
	--tmppath "${GENIMAGE_TMP}" \
	--inputpath "${BINARIES_DIR}" \
	--outputpath "${BINARIES_DIR}" \
	--config "${BOARD_DIR}/genimage.cfg"

echo
echo "Image sizes:"
ls -lh "${BINARIES_DIR}"/zImage "${BINARIES_DIR}"/rootfs.cpio.lz4 "${BINARIES_DIR}"/sdcard.img
