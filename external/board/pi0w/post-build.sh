#!/bin/sh
#
# Trim the rootfs: everything ends up in the initramfs, which the GPU
# firmware reads from SD and the kernel unpacks on every boot.
#

set -eu

# Init scripts replaced by our own S00boot/S01network or not wanted
rm -f "${TARGET_DIR}/etc/init.d/S40network" \
      "${TARGET_DIR}/etc/init.d/S41dhcpcd" \
      "${TARGET_DIR}/etc/init.d/S50crond" \
      "${TARGET_DIR}/etc/init.d/S81cupsd"

# Only libcups is needed (by PAPPL/LPrint), not the CUPS spooler
rm -rf "${TARGET_DIR}/etc/cups" \
       "${TARGET_DIR}/usr/lib/cups" \
       "${TARGET_DIR}/usr/share/cups" \
       "${TARGET_DIR}/usr/share/doc"
for f in cupsd cupsfilter cupsctl cupsaccept cupsreject cupsenable \
         cupsdisable lpadmin lpc lpinfo lpmove; do
	rm -f "${TARGET_DIR}/usr/sbin/$f"
done
for f in lp lpq lpr lprm lpstat lpoptions cancel cupstestppd ipptool \
         ippfind ippeveprinter ippeveps ippevepcl; do
	rm -f "${TARGET_DIR}/usr/bin/$f"
done

# Wi-Fi firmware: the Zero W only has the BCM43430
for d in brcm cypress; do
	dir="${TARGET_DIR}/lib/firmware/$d"
	[ -d "$dir" ] || continue
	find "$dir" -mindepth 1 -maxdepth 1 ! -name '*43430*' -exec rm -rf {} +
done

# Kernel modules: boot-critical drivers are built in. Of the ~2000
# bcmrpi modules keep only USB ones (hotplugged adapters, USB serial /
# ethernet for debugging) and the shared helpers they may need.
for moddir in "${TARGET_DIR}"/lib/modules/*; do
	[ -d "$moddir/kernel" ] || continue
	find "$moddir/kernel" -name '*.ko*' \
		! -path '*/kernel/drivers/usb/*' \
		! -path '*/kernel/drivers/net/usb/*' \
		! -path '*/kernel/drivers/net/mii.ko*' \
		! -path '*/kernel/lib/*' \
		! -path '*/kernel/crypto/*' \
		-delete
	find "$moddir/kernel" -type d -empty -delete
	"${HOST_DIR}/sbin/depmod" -a -b "${TARGET_DIR}" "$(basename "$moddir")"
done

# Man pages, headers and static libs never belong on the target
rm -rf "${TARGET_DIR}/usr/share/man" "${TARGET_DIR}/usr/include"
find "${TARGET_DIR}/usr/lib" -name '*.a' -delete

# Version stamp: "mobix update" only installs releases with a newer
# timestamp, "make release" signs exactly these two values
BOARD_DIR="$(dirname "$0")"
cat > "${TARGET_DIR}/etc/mobix-release" <<EOF
VERSION=$(git -C "${BOARD_DIR}" describe --tags --always --dirty 2>/dev/null || echo unknown)
TIMESTAMP=$(date +%s)
EOF

if [ ! -f "${TARGET_DIR}/etc/mobix/update.pub" ]; then
	echo "WARNING: no etc/mobix/update.pub in the rootfs overlay," \
		"this image cannot verify updates (see README, Releases)"
fi
