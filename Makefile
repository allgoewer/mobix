# Thin wrapper around Buildroot with the mobix BR2_EXTERNAL tree.
#
#   make              configure (first time) and build output/images/sdcard.img
#   make menuconfig   any Buildroot target is passed through
#   make savedefconfig  write config changes back to external/configs/
#   make release      build, then sign image and update payload into output/release

TOP        := $(CURDIR)
DEFCONFIG  ?= mobix_pi0w_defconfig
O          ?= $(TOP)/output
export BR2_DL_DIR ?= $(TOP)/dl

BR_MAKE = $(MAKE) -C $(TOP)/buildroot O=$(O) BR2_EXTERNAL=$(TOP)/external

all: $(O)/.config
	$(BR_MAKE)

$(O)/.config: | buildroot/Makefile
	$(BR_MAKE) $(DEFCONFIG)

buildroot/Makefile:
	git submodule update --init buildroot

savedefconfig: $(O)/.config
	$(BR_MAKE) savedefconfig BR2_DEFCONFIG=$(TOP)/external/configs/$(DEFCONFIG)

# MINISIGN_KEY=<file> selects the secret key (default ~/.minisign/mobix.key)
release: all
	O=$(O) $(TOP)/external/board/pi0w/release.sh

%: | buildroot/Makefile
	$(BR_MAKE) $@

Makefile: ;

.PHONY: all savedefconfig release
