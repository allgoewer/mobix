################################################################################
#
# mobix-app
#
################################################################################

MOBIX_APP_VERSION = $(call qstrip,$(BR2_PACKAGE_MOBIX_APP_GIT_VERSION))
MOBIX_APP_GIT_URL = $(call qstrip,$(BR2_PACKAGE_MOBIX_APP_GIT_URL))
# Buildroot requires a site even when MOBIX_APP_OVERRIDE_SRCDIR is used
MOBIX_APP_SITE = $(or $(MOBIX_APP_GIT_URL),git-url-not-set)
MOBIX_APP_SITE_METHOD = git
MOBIX_APP_LICENSE = Proprietary
MOBIX_APP_BIN = $(call qstrip,$(BR2_PACKAGE_MOBIX_APP_BIN))

ifeq ($(BR_BUILDING)$(BR2_PACKAGE_MOBIX_APP),yy)
ifeq ($(MOBIX_APP_GIT_URL)$(MOBIX_APP_OVERRIDE_SRCDIR),)
$(error BR2_PACKAGE_MOBIX_APP_GIT_URL is empty; set it or MOBIX_APP_OVERRIDE_SRCDIR in local.mk)
endif
endif

# Install only the app binary under a fixed name, regardless of the
# crate's binary name, so init scripts don't depend on it.
define MOBIX_APP_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 \
		$(@D)/target/$(RUSTC_TARGET_NAME)/release/$(MOBIX_APP_BIN) \
		$(TARGET_DIR)/usr/bin/mobix-app
endef

$(eval $(cargo-package))
