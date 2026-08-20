# Third-Party Notices

StickyMD is licensed under the MIT License. The following bundled third-party assets retain their
own licenses.

## RaTeX 0.1.14

StickyMD uses the MIT-licensed `ratex-parser`, `ratex-layout`, `ratex-types`, `ratex-font`,
`ratex-font-loader`, `ratex-katex-fonts`, and `ratex-unicode-font` crates. The native DisplayList
painter in `stickymd-render` is narrowly adapted from RaTeX 0.1.14's MIT-licensed renderer and
retains source attribution in that module.

Project: <https://github.com/needle-tools/ratex>

## KaTeX Fonts

The embedded `KaTeX_*.ttf` font binaries distributed through `ratex-katex-fonts` originate from
the KaTeX project and are licensed under the SIL Open Font License 1.1.

- Font notice: [`assets/licenses/KaTeX-fonts-NOTICE.txt`](assets/licenses/KaTeX-fonts-NOTICE.txt)
- Full OFL 1.1 text: [`assets/licenses/SIL-OFL-1.1.txt`](assets/licenses/SIL-OFL-1.1.txt)
- Upstream: <https://github.com/KaTeX/KaTeX>

Rust dependency licenses remain governed by `Cargo.lock` and `deny.toml`; release packaging must
include this notice and the two font-license files above.
