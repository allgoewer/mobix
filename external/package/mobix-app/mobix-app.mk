################################################################################
#
# mobix-app
#
################################################################################

# The app lives in this repository (app/); rebuild after changes with
# make mobix-app-rebuild all
MOBIX_APP_VERSION = local
MOBIX_APP_SITE = $(BR2_EXTERNAL_MOBIX_PATH)/../app
MOBIX_APP_SITE_METHOD = local
# Don't copy host builds into the build directory
MOBIX_APP_OVERRIDE_SRCDIR_RSYNC_EXCLUSIONS = --exclude /target
MOBIX_APP_LICENSE = Proprietary
MOBIX_APP_DEPENDENCIES = host-pkgconf openssl

# openssl-sys (native-tls for MQTT over TLS) looks up the target OpenSSL
# through pkg-config
MOBIX_APP_CARGO_ENV = PKG_CONFIG_ALLOW_CROSS=1

# Buildroot vendors crates only for downloaded sources, but builds with
# --offline: fetch the dependencies of the local source tree into the cargo
# home first.
define MOBIX_APP_CARGO_FETCH
	cd $(@D) && \
	$(TARGET_MAKE_ENV) $(PKG_CARGO_ENV) cargo fetch --locked
endef
MOBIX_APP_PRE_BUILD_HOOKS += MOBIX_APP_CARGO_FETCH

$(eval $(cargo-package))
