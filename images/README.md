Committed renders land here. There is one scene graph, so the milestones that
share a scene share an image; `images/` therefore holds what the renderer
actually produces rather than one near-identical file per milestone.

- `final-spheres.png` — the current renderer, 400×225 at 100 samples per
  pixel: `cargo run --release -- --out images/final-spheres.png`
- `cover-probe.png` — `cover` at 200×112 and 8 samples per pixel, evidence
  that the cover path and the PNG encoder both work:
  `cargo run --release -- --scene cover --width 200 --samples 8 --out images/cover-probe.png`
- `ch02-gradient.ppm` — the first image the renderer ever wrote: a vertical
  red/green gradient, from the milestone before there was a camera. Kept as P6
  because that is the format it was written in.