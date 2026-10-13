# Third-party notices

NanaType includes the third-party fonts below. Their full licence texts ship
with the program: in the installer's `licenses` folder (beside this file) and
on the About page under "Open-source fonts". Sources, versions and checksums
are recorded in [crates/theme/fonts/README.md](crates/theme/fonts/README.md).

This file covers fonts only. It does not yet list the Rust crates NanaType is
built from or the DirectX Shader Compiler (`dxcompiler.dll`) the Windows
package ships beside the executable.

## Fonts embedded for the built-in skins

### Fusion Pixel Font 12px Proportional (Simplified Chinese), 2026.09.25

Used by the `pixel` and `rpg` skins, as a subset
(`FusionPixel12pxProportionalSC.ttf`; family `Fusion Pixel 12px Prop zh-Hans`).

- Fusion Pixel Font — Copyright (c) 2022, TakWolf (https://takwolf.com).
  <https://github.com/TakWolf/fusion-pixel-font>. SIL Open Font License 1.1.
- Includes glyphs from Ark Pixel Font — Copyright (c) 2021, TakWolf
  (https://takwolf.com). <https://github.com/TakWolf/ark-pixel-font>. SIL Open
  Font License 1.1.
- Includes glyphs from Cubic 11, with JF Dot M+H 12 (Copyright (c) 2005 M+
  FONTS PROJECT) and M+ BITMAP FONTS (Copyright (C) 2002-2004 COZ). SIL Open
  Font License 1.1.
- Includes glyphs from Galmuri — Copyright (c) 2019–2025 Lee Minseo
  (quiple@quiple.dev). SIL Open Font License 1.1.

Licence texts: `FusionPixel12pxProportionalSC.LICENSE.txt`.

### LXGW WenKai (霞鹜文楷) Regular, 1.522

Used by the `ink` skin, as a subset (`LXGWWenKai-Regular.ttf`; family
`LXGW WenKai`).

- Copyright 2021-2026 LXGW (https://github.com/lxgw/LxgwWenKai)
- Copyright 2020 The Klee Project Authors (https://github.com/fontworks-fonts/Klee)
- SIL Open Font License 1.1.

Licence text: `LXGWWenKai-Regular.LICENSE.txt`.

No Reserved Font Name is declared for either font, so the subsets keep the
upstream names.

## Fonts embedded by NanaUI

### Noto Sans SC

NanaType's interface font, and the last fallback of the `echo_*` skins, comes
with the NanaUI toolkit. Copyright 2014-2021 Adobe (http://www.adobe.com/),
with Reserved Font Name 'Source'. SIL Open Font License 1.1.
