# Third-party notices

freeRGB is free software under the GNU General Public License, version 2 or later (`LICENSE`).
It is a front-end for OpenRGB and is not part of the OpenRGB project. This file lists what
freeRGB's installers and packages include from others, and under which terms.

Every freeRGB release on GitHub (https://github.com/pwalda/freergb/releases) carries, beside the
installers, the source of the copyleft components below: OpenRGB, Qt, libusb and the PawnIO
modules.

## OpenRGB 1.1 (Windows installer)

The Windows installer includes OpenRGB 1.1 for Windows as published by the OpenRGB project
(https://openrgb.org), in its `openrgb` folder, without Microsoft's DirectX shader compiler
(`dxcompiler.dll`, `dxil.dll`), which only OpenRGB's window uses. OpenRGB is free software under the GNU
General Public License, version 2 or later; its licence text is `openrgb\LICENSE.txt`. Its source,
release_1.1 at commit 8e16de27858cf2be6af54a33ce003df74cb9fd3e:
https://codeberg.org/OpenRGB/OpenRGB/src/commit/8e16de27858cf2be6af54a33ce003df74cb9fd3e

`OpenRGB.exe` itself contains further libraries (mbedTLS, cpp-httplib, nlohmann/json,
hueplusplus, libe131, CRCpp, mdns, stb and others); their source and licences are in the
`dependencies` folder of OpenRGB's source.

OpenRGB's folder also carries these libraries, as OpenRGB ships them:

- **Qt 6.8.3** (`Qt6Core`, `Qt6Core5Compat`, `Qt6Gui`, `Qt6OpenGL`, `Qt6Widgets` and the
  `platforms`, `styles` and `imageformats` plugins). Copyright (C) The Qt Company Ltd. and other
  contributors. Used under the GNU Lesser General Public License, version 3
  (https://www.gnu.org/licenses/lgpl-3.0.html). Source: qtbase and qt5compat 6.8.3, attached
  to every release; also https://download.qt.io/archive/qt/6.8/6.8.3/submodules/. You may
  replace these DLLs with your own build of the same Qt version.
- **libusb 1.0.27** (`libusb-1.0.dll`). Copyright the libusb authors. GNU Lesser General Public
  License, version 2.1 or later. Source attached to every release; also
  https://github.com/libusb/libusb/releases/tag/v1.0.27.
- **HIDAPI 0.15.0** (`hidapi-hotplug.dll`, OpenRGB's build with hotplug support). Copyright (c)
  2010, Alan Ott, Signal 11 Software, and the libusb/hidapi team. HIDAPI may be used under the
  GNU GPL v3, a BSD-style licence, or the original HIDAPI licence; it is redistributed here
  under the BSD-style licence, reproduced at the end of this file.
- **PawnIOLib** (`PawnIOLib.dll`) and PawnIO modules (`SmbusI801.bin`, `SmbusPIIX4.bin`,
  `SmbusIntelSkylakeIMC.bin`, `SmbusNCT6793.bin`, `LpcIO.bin`, `LedsValve.bin`), by namazso
  (https://pawnio.eu), as OpenRGB 1.1 ships them. PawnIOLib's open-source edition is under the
  GNU Lesser General Public License, version 2.1 (https://github.com/namazso/PawnIO); the
  modules are under the LGPL, version 2.1 or later (https://github.com/namazso/PawnIO.Modules).

The PawnIO driver is not part of freeRGB: the installer offers to download it from PawnIO's
official release.

## PawnIO modules (Windows sensor service)

The Windows sensor service (`freergb-sensord.exe`) contains two PawnIO modules, IntelMSR and
AMDFamily17, unmodified from PawnIO.Modules 0.2.12 by namazso
(https://github.com/namazso/PawnIO.Modules). They are free software under the GNU Lesser General
Public License version 2.1 or later; their source is attached to every release and is at
https://github.com/namazso/PawnIO.Modules/tree/0.2.12.

## OpenRGB's udev rules (Linux)

The Linux app contains OpenRGB 1.1's udev rules (`60-openrgb.rules`), unmodified from OpenRGB's
official Debian package, which it installs when you choose "Set up device access". They are
part of OpenRGB, under the GNU General Public License, version 2 or later (source as above).

## Geist, Geist Mono and Doto (UI typefaces)

The window's text is set in Geist, and its readouts and numbers in Geist Mono, both bundled as
variable fonts (latin and latin-ext subsets) from the `@fontsource-variable/geist` and
`@fontsource-variable/geist-mono` packages. Both are licensed under the SIL Open Font License
1.1, reproduced below. They are bundled unmodified.

The freeRGB wordmark is set in Doto (latin subset), bundled unmodified from the
`@fontsource-variable/doto` package, under the same SIL Open Font License 1.1.
Copyright 2024 The Doto Project Authors (https://github.com/oliverlalan/Doto).

```
Copyright 2024 The Geist Project Authors (https://github.com/vercel/geist-font)

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
http://scripts.sil.org/OFL


-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

## HIDAPI licence (BSD-style)

```
Copyright (c) 2010, Alan Ott, Signal 11 Software
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

    * Redistributions of source code must retain the above copyright notice,
      this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above copyright
      notice, this list of conditions and the following disclaimer in the
      documentation and/or other materials provided with the distribution.
    * Neither the name of Signal 11 Software nor the names of its
      contributors may be used to endorse or promote products derived from
      this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.
```
