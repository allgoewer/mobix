################################################################################
#
# typst
#
################################################################################

# Needs Rust >= 1.92, i.e. Buildroot >= 2026.05
TYPST_VERSION = 0.15.1
TYPST_SITE = $(call github,typst,typst,v$(TYPST_VERSION))
TYPST_LICENSE = Apache-2.0
TYPST_LICENSE_FILES = LICENSE
TYPST_DEPENDENCIES = host-pkgconf openssl

# openssl-sys looks up the target OpenSSL through pkg-config
TYPST_CARGO_ENV = PKG_CONFIG_ALLOW_CROSS=1

# The repository is a workspace whose default member is the CLI, so the
# default "cargo install --path ./" does not work.
define TYPST_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 \
		$(@D)/target/$(RUSTC_TARGET_NAME)/release/typst \
		$(TARGET_DIR)/usr/bin/typst
endef

$(eval $(cargo-package))
