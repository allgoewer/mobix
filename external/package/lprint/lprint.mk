################################################################################
#
# lprint
#
################################################################################

LPRINT_VERSION = 54f1c462fe532eb59fb78cc4de6ad2c808b796c4
LPRINT_SITE = $(call github,michaelrsweet,lprint,$(LPRINT_VERSION))
LPRINT_LICENSE = Apache-2.0
LPRINT_LICENSE_FILES = LICENSE
LPRINT_DEPENDENCIES = host-pkgconf pappl cups

LPRINT_CONF_OPTS = --without-systemd

$(eval $(autotools-package))
