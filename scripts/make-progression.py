#!/usr/bin/env python3
"""
grazing-incidence — progression GIF builder.

Renders the real historical state of the renderer at each milestone, composites
a caption, and assembles an animated GIF for the README.

    python scripts/make-progression.py

Nothing here is simulated. Every frame is produced by checking out the commit
that introduced the feature and running that commit's own binary. The frames
are therefore an honest record of the progression, not a reconstruction.

Output: images/progression.gif


THE SCRIPT
==========

Tone: polished. This is a serious piece of engineering, not a joke, so the
edit plays it straight: real numbers, real commit output, no adjectives.

Arc (hook, reveal, highlights, outro):

  HOOK      a red-and-green gradient. The whole renderer, at its smallest.
  REVEAL    a sphere appears. Then a ground plane. The scene exists.
  HIGHLIGHT materials arrive one at a time -- diffuse, metal, glass.
  HIGHLIGHT the lens. The subtlest bug in the repo, and its real cost.
  OUTRO     484 random spheres. Then the numbers.

Beat by beat, and what each one is claiming:

  1  PPM output          256x256, binary P6. A header and 196,623 bytes.
  2  rays and sky        No objects yet. A camera that shoots into a gradient.
  3  sphere              Ray-sphere as a quadratic; t = 10.5 exactly.
  4  hittable            A trait, a hit list, a normal colour map.
  5  antialiasing        Jittered samples over a hand-rolled PCG32.
  6  diffuse             Lambertian scatter, 0.001 shadow offset, gamma.
  7  metal               Reflect about the normal, then blur by fuzz.
  8  dielectric          Snell, Schlick, total internal reflection past 41.8.
  9  defocus             The viewport sits at focus_dist. Get it wrong and
                         the cover's aperture is 25.9% too narrow.
 10  cover               484 random spheres, 1200x675 at 500 samples.
 11  outro               173 tests. Zero dependencies.

VISUAL DIRECTION

Borrowed in method, not in aesthetic. The source skill's genre vocabulary is
for music videos -- iridescent bokeh, candy-pink chrome, winged figures -- and
none of it survives contact with a ray tracer. Three of its disciplines do:

  * Decide the direction before the first frame renders, then let it constrain
    everything. This file fixes the palette and the timing up front.
  * Coherence over novelty. One look across every beat, rather than eleven
    looks that each try to be interesting.
  * One accent per shot. Warm gold appears on the money shot and nowhere else.

Translated to a silent still-frame GIF:

  * Base      near-black #05060a, bone white #f4f1ea. Every caption.
  * Accent    gold #ffb24d. The milestone number, and the outro only.
  * Motion    "slow, floating, weightless" becomes long holds -- 2s a beat,
              no fast cuts. With no audio there is no beat to cut on, so the
              only rhythm available is patience.
  * Grain     a fixed-seed dither before quantisation. Two jobs: the grain the
              aesthetic asks for, and breaking up the banding a 256-colour GIF
              would otherwise put across every smooth sky.

The frames are real renders, so nothing is restyled, recoloured or filtered.
The grain is the only thing added to any pixel, and it is applied after the
render, uniformly, at low amplitude.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont, ImageFilter

REPO = Path(__file__).resolve().parent.parent
WORKTREE = Path(tempfile.gettempdir()) / "gi-progression"
FRAMES = Path(tempfile.gettempdir()) / "gi-progression-frames"
GIF = REPO / "images" / "progression.gif"

FPS = 10  # still frames, so this only sets how long a hold can be

# --- palette ---------------------------------------------------------------
VOID = (5, 6, 10)
BONE = (244, 241, 234)
GOLD = (255, 178, 77)
DIM = (138, 143, 158)

FRAME_W, FRAME_H = 400, 225
CAPTION_H = 74
GUTTER = 14
TEXT_W = FRAME_W - GUTTER * 2 - 16  # a column wider than this is a wrap point


# --- the beats -------------------------------------------------------------
# sha, milestone, headline, caption, render args, hold seconds
BEATS = [
    ("4af3ea3", "01", "Output an image",
     "A binary PPM. 256 x 256, one header line, 196,623 bytes.",
     ["--width", "320", "--height", "180"], 2.4),

    ("3d8f19e", "02", "Rays, camera, sky",
     "Nothing to hit yet. A camera that shoots into a gradient.",
     ["--width", "400", "--height", "225"], 2.0),

    ("57e3400", "03", "One sphere",
     "Ray-sphere as a quadratic. The book's own example solves to t = 10.5, exactly.",
     ["--width", "400", "--height", "225"], 2.2),

    ("cbf77ad", "04", "Surface normals, many objects",
     "A trait, a hit list, a normal colour map: each normal component, "
     "[-1,1] to [0,1]. One sample per pixel.",
     ["--width", "400", "--height", "225"], 1.6),

    ("f26d0da", "05", "Antialiasing",
     "Jittered samples per pixel, averaged, drawn from a PCG32 written "
     "from scratch. Same seed, same bytes, every run. Smooth.",
     ["--width", "400", "--height", "225"], 2.6),

    ("a06bcb6", "06", "Diffuse materials",
     "Lambertian scatter, a 0.001 offset against shadow acne, and a sqrt "
     "gamma encode at write time.",
     ["--width", "400", "--height", "225"], 2.2),

    ("d7ee6ea", "07", "Metal",
     "Reflect about the normal, then blur by fuzz. Below the horizon it "
     "reflects ground; above, sky.",
     ["--width", "400", "--height", "225"], 2.2),

    ("44a385b", "08", "Dielectrics",
     "Snell's law for refraction, Schlick for reflectivity, total internal "
     "reflection past the critical angle.",
     ["--width", "400", "--height", "225"], 2.4),

    ("e9b794e", "09", "A thin lens",
     "The viewport sits at focus_dist, not the eye-to-target distance. "
     "Wrong: 0.4448deg, not 0.6.",
     ["--width", "400", "--samples", "60", "--out", "images/f09.ppm"], 2.4),

    ("HEAD", "10", "The cover scene",
     "484 random spheres. 1200 x 675 at 500 samples is the full render; "
     "shown here small.",
     ["--scene", "cover", "--width", "360", "--samples", "48",
      "--out", "images/f10.ppm"], 2.8),
]


def cargo() -> str:
    """cargo, on PATH. Windows without MSVC does not have it by default."""
    exe = shutil.which("cargo")
    if exe:
        return exe
    home = Path(os.environ["USERPROFILE"]) / ".cargo" / "bin" / "cargo.exe"
    if home.exists():
        return str(home)
    sys.exit("cargo not found -- install Rust from https://rustup.rs")


def load_font(size: int, bold: bool = False) -> ImageFont.FreeTypeFont:
    names = (
        ["consolab.ttf", "courbd.ttf"] if bold else ["consola.ttf", "cour.ttf"]
    ) + ["segoeui.ttf", "arial.ttf", "calibri.ttf", "DejaVuSans.ttf"]
    for name in names:
        for root in ("C:/Windows/Fonts", "/usr/share/fonts/truetype/dejavu"):
            p = Path(root) / name
            if p.exists():
                try:
                    return ImageFont.truetype(str(p), size)
                except OSError:
                    pass
    return ImageFont.load_default()


def newest_render(directory: Path, before: set[str]) -> Path | None:
    """The file this render wrote, whatever the commit called it."""
    after = {p.name for p in directory.iterdir() if p.is_file()}
    fresh = [directory / n for n in after - before]
    if fresh:
        return max(fresh, key=lambda p: p.stat().st_mtime)
    candidates = [
        p for p in directory.iterdir()
        if p.is_file() and p.name not in {"README.md", "ch02-gradient.ppm"}
    ]
    return max(candidates, key=lambda p: p.stat().st_mtime) if candidates else None


def capture(beat) -> Path:
    sha, _, _, _, args, _ = beat
    before = {p.name for p in (WORKTREE / "images").iterdir() if p.is_file()}
    if sha != "HEAD":
        subprocess.run(["git", "checkout", "-q", sha], cwd=WORKTREE, check=True)
    subprocess.run([cargo(), "run", "-q", "--release", "--", *args],
                   cwd=WORKTREE, check=True)
    src = newest_render(WORKTREE / "images", before)
    if src is None:
        raise RuntimeError(f"{sha}: no image written")
    return src


def grain(img: Image.Image, seed: int) -> Image.Image:
    """Fixed-seed dither. Breaks the banding a 256-colour GIF forces on every
    smooth sky, and is reproducible because the seed is the beat index."""
    import random
    rng = random.Random(seed)
    px = img.load()
    for y in range(img.height):
        for x in range(img.width):
            r, g, b = px[x, y][:3]
            n = rng.randint(-4, 4)
            px[x, y] = (
                max(0, min(255, r + n)),
                max(0, min(255, g + n)),
                max(0, min(255, b + n)),
            )
    return img


def wrap(draw, text: str, font, width: int) -> list[str]:
    """Greedy wrap measured with the font's own advance widths.

    Fixed line breaks in the beat table are a lie the moment the text does not
    fit the frame -- it silently runs off the right edge. Measure instead."""
    out: list[str] = []
    for para in text.split("\n"):
        line = ""
        for word in para.split():
            trial = f"{line} {word}".strip()
            if line and draw.textlength(trial, font=font) > width:
                out.append(line)
                line = word
            else:
                line = trial
        out.append(line)
    return out


def compose(img: Image.Image, num: str, head: str, lines: list[str]) -> Image.Image:
    canvas = Image.new("RGB", (FRAME_W, FRAME_H + CAPTION_H), VOID)
    canvas.paste(img, (0, 0))
    draw = ImageDraw.Draw(canvas)

    f_num = load_font(19, bold=True)
    f_head = load_font(15, bold=True)
    f_body = load_font(11)

    y = FRAME_H + 6
    draw.text((GUTTER, y), num, font=f_num, fill=GOLD)
    draw.text((GUTTER + 28, y + 1), head.upper(), font=f_head, fill=BONE)
    draw.line([(GUTTER, y + 23), (FRAME_W - GUTTER, y + 23)],
              fill=(30, 33, 43), width=1)

    y += 29
    for line in wrap(draw, " ".join(lines), f_body, TEXT_W):
        if y > FRAME_H + CAPTION_H - 12:
            break
        draw.text((GUTTER, y), line, font=f_body, fill=DIM)
        y += 14
    return canvas


def outro_card() -> Image.Image:
    canvas = Image.new("RGB", (FRAME_W, FRAME_H + CAPTION_H), VOID)
    d = ImageDraw.Draw(canvas)
    f_big = load_font(27, bold=True)
    f_mid = load_font(13)
    f_small = load_font(11)

    cy = FRAME_H // 2
    d.text((FRAME_W // 2, cy - 46), "grazing-incidence", font=f_big,
           fill=BONE, anchor="mm")
    d.text((FRAME_W // 2, cy - 6), "173 tests  ·  zero dependencies",
           font=f_mid, fill=GOLD, anchor="mm")
    d.text((FRAME_W // 2, cy + 18),
           "PCG32 · PPM · PNG · CRC-32 · Adler-32 · DEFLATE · arg parsing",
           font=f_small, fill=DIM, anchor="mm")
    d.text((FRAME_W // 2, cy + 38),
           "every one written from scratch, because a ray tracer needs no crates",
           font=f_small, fill=DIM, anchor="mm")
    return canvas


def main() -> None:
    if WORKTREE.exists():
        subprocess.run(["git", "worktree", "remove", str(WORKTREE), "--force"],
                       cwd=REPO, capture_output=True)
        shutil.rmtree(WORKTREE, ignore_errors=True)
    if FRAMES.exists():
        shutil.rmtree(FRAMES)
    FRAMES.mkdir(parents=True)

    subprocess.run(["git", "worktree", "add", "-q", "--detach", str(WORKTREE), "HEAD"],
                   cwd=REPO, check=True)

    print(f"rendering {len(BEATS)} milestones from real commits")
    timeline: list[tuple[Path, int]] = []

    try:
        for i, beat in enumerate(BEATS):
            sha, num, head, cap, _, hold = beat
            print(f"  [{num}] {sha[:7]} {head}")
            src = capture(beat)
            img = Image.open(src).convert("RGB")
            if (img.width, img.height) != (FRAME_W, FRAME_H):
                img = img.resize((FRAME_W, FRAME_H), Image.LANCZOS)
            img = grain(img, i)
            frame = compose(img, num, head, [l for l in cap.split("\n")])
            for _ in range(int(hold * FPS)):
                timeline.append((frame, i))

        for _ in range(int(2.6 * FPS)):
            timeline.append((outro_card(), len(BEATS)))
    finally:
        subprocess.run(["git", "worktree", "remove", str(WORKTREE), "--force"],
                       cwd=REPO, capture_output=True)
        shutil.rmtree(WORKTREE, ignore_errors=True)

    for idx, (frame, _) in enumerate(timeline):
        frame.save(FRAMES / f"{idx:05d}.png")

    print(f"  {len(timeline)} frames -> ffmpeg")

    pal = FRAMES / "palette.png"
    subprocess.run([
        "ffmpeg", "-y", "-v", "error", "-framerate", str(FPS),
        "-i", str(FRAMES / "%05d.png"),
        "-vf", "palettegen=stats_mode=diff", str(pal),
    ], check=True)
    subprocess.run([
        "ffmpeg", "-y", "-v", "error", "-framerate", str(FPS),
        "-i", str(FRAMES / "%05d.png"), "-i", str(pal),
        "-lavfi", "paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle",
        str(GIF),
    ], check=True)

    kb = GIF.stat().st_size / 1024
    print(f"wrote {GIF.relative_to(REPO)}  {FRAME_W}x{FRAME_H + CAPTION_H}  "
          f"{len(timeline) / FPS:.1f}s  {kb / 1024:.2f} MB")
    if kb > 4096:
        print("WARNING: over 4 MB. Drop FRAME_W or the hold times.")


if __name__ == "__main__":
    main()