################################################################################
#
# pappl
#
################################################################################

PAPPL_VERSION = 1.4.12
PAPPL_SITE = https://github.com/michaelrsweet/pappl/releases/download/v$(PAPPL_VERSION)
PAPPL_LICENSE = Apache-2.0
PAPPL_LICENSE_FILES = LICENSE
PAPPL_INSTALL_STAGING = YES
PAPPL_DEPENDENCIES = host-pkgconf cups libusb libpng jpeg openssl zlib

PAPPL_CONF_OPTS = \
	--with-tls=openssl \
	--enable-libusb \
	--enable-libpng \
	--enable-libjpeg \
	--disable-libpam \
	--disable-static

ifeq ($(BR2_PACKAGE_AVAHI_LIBAVAHI_CLIENT),y)
PAPPL_DEPENDENCIES += avahi
PAPPL_CONF_OPTS += --with-dnssd=avahi
else
PAPPL_CONF_OPTS += --with-dnssd=no
endif

# Only build the library, not the test suite
PAPPL_MAKE_OPTS = DIRS=pappl
PAPPL_INSTALL_STAGING_OPTS = DIRS=pappl DESTDIR=$(STAGING_DIR) install
PAPPL_INSTALL_TARGET_OPTS = DIRS=pappl DESTDIR=$(TARGET_DIR) install

$(eval $(autotools-package))
