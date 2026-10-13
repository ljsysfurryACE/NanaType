# Built-in skin fonts

Fonts here are embedded in the binary (feature `builtin-fonts`) and named by
a built-in skin as `file = "builtin:fonts/<name>.ttf"`. Each `<name>.ttf` or
`<name>.otf` needs its licence beside it as `<name>.LICENSE.txt`, or the
build fails. See [docs/fonts.md](../../../docs/fonts.md) for which fonts,
where they come from, how to subset them and the size budget.

A build with no font files here works: skins fall back to their `families`.
