# Native UI assets

SVGs are exact exports from the approved Mo Figma file pFUWDZYKrLw9dxdNQ4XanU.
Previous/next/expand/collapse/close/add/shield are original Mo geometry (Apache-2.0).
Smile is Figma Simple Design System (MIT); see third_party/figma-sds-MIT.txt and
third_party/design-assets-20261011.md.

tools/export-ui-icons.ps1 uses Windows WPF Geometry to render original SVG paths,
round caps and joins into alpha masks at 20/25/30/40/50/60/80 px.
Smile retains its 18.6667 px extent centered in the 20 px design slot.
Native rendering applies semantic tint then AlphaBlend.
Masks are embedded; no runtime URL or current-directory asset search is used.
No font binary is packaged.
