#!/bin/bash
#
# Sign the image and the update payload of the current build with
# minisign and collect the release files in $O/release. Run through
# "make release"; nothing is uploaded.
#
# The secret key never enters the repository or CI: whoever holds it can
# install software on every deployed device.
#

set -euo pipefail

TOP="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
O="${O:-${TOP}/output}"
KEY="${MINISIGN_KEY:-${HOME}/.minisign/mobix.key}"
PUBKEY="${TOP}/external/board/pi0w/rootfs-overlay/etc/mobix/update.pub"
MINISIGN="${O}/host/bin/minisign"
OUT="${O}/release"

die() {
	echo "release: $*" >&2
	exit 1
}

[ -f "${KEY}" ] && [ -f "${PUBKEY}" ] || die "key pair not found. Create it once with
  ${MINISIGN} -G -s ${KEY} -p ${PUBKEY}
then commit update.pub and rebuild."

# VERSION and TIMESTAMP of the image that was just built
. "${O}/target/etc/mobix-release"

# ALLOW_UNTAGGED=1 is for trying this out, not for real releases
if [ -z "${ALLOW_UNTAGGED:-}" ]; then
	tag="$(git -C "${TOP}" describe --tags --exact-match 2>/dev/null)" ||
		die "HEAD is not tagged; tag the release commit first (git tag vX.Y)"
	[ "${VERSION}" = "${tag}" ] ||
		die "the image was built as ${VERSION}, not ${tag}: uncommitted changes or a stale build"
fi

rm -rf "${OUT}"
mkdir -p "${OUT}"
image="${OUT}/mobix-${VERSION}.img.xz"
update="${OUT}/mobix-update.tar"
cp "${O}/images/mobix-update.tar" "${update}"
xz -T0 -c "${O}/images/sdcard.img" > "${image}"

# The trusted comment is covered by the signature; the device compares
# the timestamp with its own to refuse downgrades
"${MINISIGN}" -S -s "${KEY}" -t "version=${VERSION} timestamp=${TIMESTAMP}" \
	-m "${update}" "${image}"

# The key baked into this very image must accept what was just signed
for f in "${update}" "${image}"; do
	"${MINISIGN}" -V -q -p "${PUBKEY}" -m "${f}" ||
		die "$(basename "${f}") does not verify against ${PUBKEY}: wrong secret key?"
done

echo
echo "Release ${VERSION} is in ${OUT}:"
ls -lh "${OUT}"
echo
echo "Publish it with:"
echo "  git push origin ${VERSION}"
echo "  gh release create ${VERSION} --verify-tag --generate-notes ${OUT}/*"
