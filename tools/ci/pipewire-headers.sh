#!/usr/bin/env bash
# Ubuntu 22.04 (the release build host, for an old glibc floor) ships PipeWire
# 0.3.48, too old for the libspa crate's bindings. This installs newer PipeWire/SPA
# *headers* into PREFIX, plus pkg-config files that point at them but still link the
# system libpipewire-0.3.so.0 (needs libpipewire-0.3-dev). A symbol 0.3.48 lacks then
# fails the link, and the packages run against the distro's own PipeWire.
# Usage: tools/ci/pipewire-headers.sh PREFIX, then PKG_CONFIG_PATH=PREFIX/lib/pkgconfig.
set -euo pipefail
version=1.0.5
sha256=c5a5de26d684a1a84060ad7b6131654fb2835e03fccad85059be92f8e3ffe993
prefix=${1:?usage: $0 PREFIX}

src=$(mktemp -d)
trap 'rm -rf "$src"' EXIT
curl -fsSL -o "$src/pw.tar.gz" \
  "https://gitlab.freedesktop.org/pipewire/pipewire/-/archive/$version/pipewire-$version.tar.gz"
echo "$sha256  $src/pw.tar.gz" | sha256sum -c -
tar -xzf "$src/pw.tar.gz" -C "$src" --strip-components=1

inc=$prefix/include
mkdir -p "$inc/spa-0.2" "$inc/pipewire-0.3" "$prefix/lib/pkgconfig"
cp -r "$src/spa/include/spa" "$inc/spa-0.2/"
(cd "$src/src" && find pipewire -name '*.h' -exec cp --parents {} "$inc/pipewire-0.3/" \;)
IFS=. read -r major minor micro <<<"$version"
sed -e "s/@PIPEWIRE_VERSION_MAJOR@/$major/g" -e "s/@PIPEWIRE_VERSION_MINOR@/$minor/g" \
    -e "s/@PIPEWIRE_VERSION_MICRO@/$micro/g" -e "s/@PIPEWIRE_API_VERSION@/\"0.3\"/g" \
    "$src/src/pipewire/version.h.in" > "$inc/pipewire-0.3/pipewire/version.h"

cat > "$prefix/lib/pkgconfig/libspa-0.2.pc" <<PC
Name: libspa
Description: PipeWire SPA headers $version
Version: $version
Cflags: -I$inc/spa-0.2 -D_REENTRANT
PC
cat > "$prefix/lib/pkgconfig/libpipewire-0.3.pc" <<PC
Name: libpipewire
Description: PipeWire $version headers, system library
Version: $version
Requires: libspa-0.2
Libs: -lpipewire-0.3
Cflags: -I$inc/pipewire-0.3 -D_REENTRANT
PC
