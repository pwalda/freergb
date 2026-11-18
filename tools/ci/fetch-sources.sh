#!/usr/bin/env bash
# Puts the source of everything a release redistributes under a copyleft licence into $1:
# OpenRGB (GPL) and, from its Windows bundle, Qt and libusb (LGPL), plus the PawnIO modules
# the sensor service embeds (LGPL). Attached to every release beside the installers.
set -euo pipefail
out=$1
root=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$out"

get() { # url file [sha256]
  curl -fsSL --retry 3 -o "$out/$2" "$1"
  if [ -n "${3:-}" ]; then echo "$3  $out/$2" | sha256sum -c -; fi
}

commit=$(grep -m1 '^commit=' "$root/tools/ci/fetch-openrgb.sh" | cut -d= -f2)
get "https://codeberg.org/OpenRGB/OpenRGB/archive/$commit.tar.gz" OpenRGB-1.1-source.tar.gz
get https://download.qt.io/archive/qt/6.8/6.8.3/submodules/qtbase-everywhere-src-6.8.3.tar.xz \
  qtbase-everywhere-src-6.8.3.tar.xz 56001b905601bb9023d399f3ba780d7fa940f3e4861e496a7c490331f49e0b80
get https://download.qt.io/archive/qt/6.8/6.8.3/submodules/qt5compat-everywhere-src-6.8.3.tar.xz \
  qt5compat-everywhere-src-6.8.3.tar.xz 54b9c84bff34b423dd8c472862ce1009753ff505e418b4ef33907416da16b82e
get https://github.com/libusb/libusb/releases/download/v1.0.27/libusb-1.0.27.tar.bz2 \
  libusb-1.0.27.tar.bz2 ffaa41d741a8a3bee244ac8e54a72ea05bf2879663c098c82fc5757853441575
tag=$(grep -m1 '^tag=' "$root/tools/ci/fetch-pawnio-modules.sh" | cut -d= -f2)
get "https://github.com/namazso/PawnIO.Modules/archive/refs/tags/$tag.tar.gz" "PawnIO.Modules-$tag-source.tar.gz"
