# Mo application identity

Original Mo artwork, Apache-2.0. Designed 2026-10-11.

- mo-app-icon.svg: 256 × 256 source; ink-green background, rounded M and mint input cursor.
- mo-mark-on-light.svg / mo-mark-on-dark.svg: transparent 32 × 32 monochrome marks,
  with a tighter viewBox for status-bar use.
- mo-app-icon.png: transparent 256 × 256 rendering of the owned SVG geometry with
  Windows WPF DrawingVisual / RenderTargetBitmap; no font dependence.
- mo-app-icon.ico: 16, 24, 32, 48, 64, 128 and 256 px entries, encoded with Pillow from
  the transparent source render. Size inventory and corner alpha were verified.

[Figma identity review](https://www.figma.com/design/pFUWDZYKrLw9dxdNQ4XanU?node-id=73-402).
SVG paths remain editable; no competitor art, imported logo or external font file.
These files have not yet been wired into the product executables or installer.
