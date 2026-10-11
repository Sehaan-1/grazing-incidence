Committed renders land here. There is one scene graph, so the chapters that
share a scene share an image; `images/` therefore holds what the renderer
actually produces rather than one near-identical file per chapter.

- `final-spheres.png` — the current renderer, 400x225 at 100 samples per
  pixel: `cargo run --release -- --out images/final-spheres.png`
- `cover-probe.png` — `cover` at 200x112 and 8 samples per pixel, evidence
  that the cover path and the PNG encoder both work:
  `cargo run --release -- --scene cover --width 200 --samples 8 --out images/cover-probe.png`
- `ch02-gradient.ppm` — §2's vertical gradient, the first image the renderer
  ever wrote. Kept as P6 because that is the format §2 produced.

See docs/superpowers/specs/2026-10-10-grazing-incidence-design.md.
