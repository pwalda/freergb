#!/usr/bin/env bash
# Builds PawnIOLib.dll from PawnIO's source into target/pawniolib/, for the Store package,
# whose read-only folder cannot take a copy of PawnIO's own build. Windows only, with the MSVC
# environment set up (cl and rc on PATH).
#
# PawnIO's CMake build also builds the driver and requires the WDK; the library is one file
# that needs only the Windows SDK, so it is compiled here directly, with the flags upstream
# uses for it.
set -euo pipefail
# PawnIO 2.2.0, which the installer offers (freergb-setup.ps1).
commit=5cdf470831fdfff3f7f1d06363ca6b230f3bf35a
version=(2 2 0)
root=$(cd "$(dirname "$0")/../.." && pwd)
# The tools get Windows paths: with conversion off (below), /d/a/... would read as a /d switch.
root=$(cygpath -m "$root")
src="$root/target/pawnio-src"
out="$root/target/pawniolib"

if [ ! -d "$src/.git" ]; then
    git init -q "$src"
    git -C "$src" remote add origin https://github.com/namazso/PawnIO
fi
git -C "$src" fetch -q --depth 1 origin "$commit"
git -C "$src" checkout -q --force FETCH_HEAD

rm -rf "$out" && mkdir -p "$out"
# Git Bash rewrites arguments that start with '/' into paths; cl and rc take them as options.
export MSYS2_ARG_CONV_EXCL='*'
v="${version[0]}.${version[1]}.${version[2]}"
defs=(
    /DPAWNIO_NAME='"PawnIO"'
    /DPAWNIO_NAME_FULL='"PawnIO library, built from source by freeRGB"'
    /DPAWNIO_AUTHOR='"namazso"'
    /DPAWNIO_VERSION_STRING="\"$v\""
    /DPAWNIO_VERSION_STRING4="\"$v.0\""
    /DPAWNIO_MAJOR="${version[0]}" /DPAWNIO_MINOR="${version[1]}" /DPAWNIO_PATCH="${version[2]}"
)
cd "$out"
rc /nologo "${defs[@]}" /fo PawnIOLib.res "$src/PawnIOLib/resource.rc"
cl /nologo /std:c++20 /O2 /MT /GR- /W4 /WX /utf-8 /Brepro \
    /DPawnIOLib_EXPORTS "${defs[@]}" \
    /I "$src/PawnIOLib/include" /I "$src/PawnIO/include" \
    /LD "$src/PawnIOLib/PawnIOLib.cpp" PawnIOLib.res \
    /link /DLL /ENTRY:DllEntry /OUT:PawnIOLib.dll /BREPRO /OPT:REF /CETCOMPAT ntdll.lib

# What OpenRGB and the sensor service call.
exports=$(dumpbin /nologo /exports PawnIOLib.dll)
for f in pawnio_version pawnio_open pawnio_load pawnio_execute pawnio_close; do
    grep -qw "$f" <<<"$exports" || { echo "PawnIOLib.dll does not export $f" >&2; exit 1; }
done
echo "$commit" >"$out/SOURCE.txt"
ls -l "$out"
