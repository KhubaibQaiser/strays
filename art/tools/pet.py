"""What every Strays pet shares (art/STYLE.md): the fixed palette, face variants, props, the
pose-controlled properties, the ambient loops, the mood poses and the clips.

A species script draws its own head, body and tail, fills in a `Fit` with its measurements, and
passes a celebration clip (spec §10.3). Node names are shared, so these clips key any species.
"""
from __future__ import annotations

import math

from dataclasses import dataclass
from typing import Callable

from rml import COAT, LINE, Clip, Rig, curve, ellipse, fill, path, rect, shape, solid, stroke

# ---- fixed palette ----
IRIS, PUPIL, WHITE = "FFD9902E", "FF2A1E18", "FFFFFFFF"
NOSE, INNER_EAR, BLUSH = "FFE79A9A", "FFF0B4B0", "40E77A7A"
BOWL, FOOD = "FF5B8DEF", "FFC98A4B"
BED, CUSHION = "FFB9A2E0", "FFE9DFF8"
CLOUD, DROP = "FF9AA4B5", "FF6FA8DC"
CARD, STAMP = "FFFFF6E0", "FFE2725B"
CONFETTI = ["FFF2C14E", "FFE2725B", "FF5B8DEF", "FF7BC47F"]
ZZZ, HEART = "FF8E8EA0", "FFE77A8F"
CAP, TASSEL, SCARF, BOW, STAR, LEAF = "FF2E2E3A", "FFF2C14E", "FFD9534F", "FFE77A8F", "FFF2C14E", "FF7BC47F"
FLOWERS = ["FFF29BB0", "FFFFFFFF", "FFB9A2E0", "FFF29BB0", "FFFFFFFF"]
# graduation rewards (spec §10.2), in the order of the rig's `accessory` number (1-5); keep in step with app/src/main.ts
ACCESSORIES = ["Cap", "Scarf", "Bow", "Star", "Crown"]


@dataclass
class Fit:
    """A species' measurements, in artboard points relative to the root on the ground line."""
    head_y: float = -178
    lie_body: float = 0.72        # Body Pose scaleY when lying down
    lie_head: float = -138        # Head Pose y when lying down
    eye_dx: float = 27            # eyes sit at ±eye_dx
    ear: float = 0.45             # Ear R Pose rest rotation; Ear L is the mirror
    ear_up: float = 0.35          # joyful
    ear_soft: float = 0.8         # petted
    ear_low: float = 0.7          # hungry
    ear_flat: float = 1.05        # gloomy
    paw_y: float = -10
    mouth_y: float = 31
    eat_dx: float = -46           # head reach towards the bowl
    eat_dy: float = 46
    crown: float = -59            # top of the head, in Head coordinates (accessories)
    chin: float = 56              # bottom of the head, where the scarf sits
    head_w: float = 148
    chest: tuple[float, float] = (36, -86)   # star badge, in Body coordinates


def eye(side: int, dx: float) -> str:
    x = dx * side
    return (
        shape(f"Glint {side}", ellipse(7, 7), fill(solid(WHITE)), x=x + 6, y=-6)
        + shape(f"Pupil {side}", ellipse(15, 22), fill(solid(PUPIL)), x=x, y=1)
        + shape(f"Iris {side}", ellipse(30, 31), fill(solid(IRIS)) + stroke(LINE, 2.5), x=x)
    )


def face(rig: Rig, f: Fit) -> str:
    """Eyes (open, happy, closed), brows, mouth, frown, yawn and the carried postcard. Goes on top of the head."""
    N, dx, my = rig.node, f.eye_dx, f.mouth_y
    arc = lambda x: path([(x - 12, 4), (x - 6, -4), (x + 6, -4), (x + 12, 4)], radius=6)  # noqa: E731
    lid = lambda x: path([(x - 13, 0), (x, 5), (x + 13, 0)], radius=8)  # noqa: E731
    return (
        N("Postcard",
          shape("Stamp", rect(9, 11, 1), fill(solid(STAMP)), x=10, y=-4)
          + shape("Lines", path([(-14, -2), (2, -2)]), stroke(solid("FFB8AFA0"), 2))
          + shape("Card", rect(40, 28, 3), fill(solid(CARD)) + stroke(LINE, 2.5)),
          y=my + 13, rotation=-0.12, opacity=0.0)
        + N("Brows", shape("Brow L", path([(-dx - 11, -22), (-dx + 9, -27)]), stroke(LINE, 3)) + shape("Brow R", path([(dx + 11, -22), (dx - 9, -27)]), stroke(LINE, 3)), opacity=0.0)
        + N("Eyes Pose", N("Eyes", eye(-1, dx) + eye(1, dx)), y=2)
        + N("Eyes Happy", shape("Arcs L", arc(-dx), stroke(solid(PUPIL), 4)) + shape("Arcs R", arc(dx), stroke(solid(PUPIL), 4)), y=2, opacity=0.0)
        + N("Eyes Closed", shape("Lid L", lid(-dx), stroke(solid(PUPIL), 4)) + shape("Lid R", lid(dx), stroke(solid(PUPIL), 4)), y=4, opacity=0.0)
        + shape("Mouth", path([(-9, my), (-4, my + 4), (0, my - 1), (4, my + 4), (9, my)], radius=3), stroke(LINE, 2.5), sid=rig.id("Mouth"))
        + shape("Frown", path([(-8, my + 4), (0, my - 1), (8, my + 4)], radius=6), stroke(LINE, 2.5), sid=rig.id("Frown"), opacity=0.0)
        + N("Mouth Open", shape("Yawn", ellipse(16, 18), fill(solid("FF8A3B3B")) + stroke(LINE, 2.5)), y=my + 5, opacity=0.0, scaleY=0.2)
    )


def _z(size: float, x: float, y: float, i: int) -> str:
    s = size / 2
    return shape(f"Z{i}", path([(-s, -s), (s, -s), (-s, s), (s, s)]), stroke(solid(ZZZ), 3), x=x, y=y)


def _heart(x: float, y: float, i: int) -> str:
    pts = [(0, 6), (-7, -1), (-6, -6), (-2, -7), (0, -4), (2, -7), (6, -6), (7, -1)]
    return shape(f"Heart {i}", curve(pts, closed=True), fill(solid(HEART)), x=x, y=y)


def props(rig: Rig) -> tuple[str, str]:
    """(drawn above the pet, drawn below it). All hidden until a pose or clip shows them."""
    N = rig.node
    bowl = N(
        "Bowl",
        N("Food", shape("Kibble", ellipse(46, 16), fill(solid(FOOD)) + stroke(solid("FF8C5A2B"), 2)), y=-18, scaleY=0.0)
        + shape("Bowl Rim", ellipse(60, 14), fill(solid("FF4A76D0")), y=-18)
        + shape("Bowl Body", path([(-30, -18), (30, -18), (22, 0), (-22, 0)], closed=True, radius=6), fill(solid(BOWL)) + stroke(LINE, 2.5)),
        x=78, y=372, opacity=0.0,
    )
    bed = N("Bed", shape("Cushion", ellipse(200, 34), fill(solid(CUSHION)), y=-8) + shape("Bed Base", ellipse(240, 48), fill(solid(BED)) + stroke(LINE, 2.5)), x=200, y=376, opacity=0.0)
    drops = N("Drops", "".join(shape(f"Drop {i}", ellipse(5, 10), fill(solid(DROP)), x=-26 + 13 * i, y=22 + 8 * (i % 2)) for i in range(5)))
    cloud = N(
        "Cloud",
        drops
        + shape("Puff A", ellipse(56, 40), fill(solid(CLOUD)), x=-22) + shape("Puff B", ellipse(64, 50), fill(solid(CLOUD)), x=12, y=-8)
        + shape("Puff C", ellipse(44, 34), fill(solid(CLOUD)), x=34, y=4),
        x=200, y=96, opacity=0.0,
    )
    confetti = N(
        "Confetti",
        "".join(shape(f"Bit {i}", rect(9, 5, 1), fill(solid(CONFETTI[i % 4])), x=x, y=y, rotation=r)
                for i, (x, y, r) in enumerate([(-120, -40, 0.4), (-80, -110, -0.6), (-30, -140, 1.1), (40, -130, 0.2), (95, -100, -0.9), (130, -30, 0.7), (-140, 30, -0.3), (150, 40, 1.3)])),
        x=200, y=200, opacity=0.0, scaleX=0.3, scaleY=0.3,
    )
    zzz = N("Zzz", _z(14, 0, 0, 1) + _z(18, 18, -24, 2) + _z(24, 40, -54, 3), x=262, y=150, opacity=0.0)
    hearts = N("Hearts", _heart(-34, 0, 1) + _heart(0, -16, 2) + _heart(34, -4, 3), x=200, y=120, opacity=0.0)
    return hearts + confetti + zzz + cloud, bowl + bed


def skinned_tail(rig: Rig, start: float, step: float, count: int, y: float, base: tuple[float, float, float], width: float = 32) -> str:
    """A thick stroked tail along +x from `start`, skinned to Tail Base/Mid/Tip.
    `base` = the three tendon x positions (bind pose, artboard space)."""
    b0, b1, b2 = base
    verts = []
    for i in range(count):
        x = start + step * i
        if x <= b0 + (b1 - b0) * 0.5:
            w = '<Weight values="255" indices="1"/>'
        elif x < b1 + (b2 - b1) * 0.6:
            t = min(1, max(0, (x - (b0 + (b1 - b0) * 0.5)) / ((b1 + (b2 - b1) * 0.6) - (b0 + (b1 - b0) * 0.5))))
            a = round(255 * (1 - t))
            w = f'<Weight values="{a | ((255 - a) << 8)}" indices="{1 | (2 << 8)}"/>'
        elif x < b2 + (b2 - b1) * 0.45:
            t = min(1, max(0, (x - (b1 + (b2 - b1) * 0.6)) / ((b2 + (b2 - b1) * 0.45) - (b1 + (b2 - b1) * 0.6))))
            a = round(255 * (1 - t))
            w = f'<Weight values="{a | ((255 - a) << 8)}" indices="{2 | (3 << 8)}"/>'
        else:
            w = '<Weight values="255" indices="3"/>'
        verts.append(f'<StraightVertex x="{x:g}" y="{y:g}" radius="{width * 0.6:g}">{w}</StraightVertex>')
    return (
        '<Shape name="Tail"><PointsPath isClosed="false" name="Path">' + "".join(verts)
        + f'<Skin tx="0" ty="0" name="Skin"><Tendon boneId="{rig.id("Tail Base")}" tx="{b0:g}" ty="{y:g}" name="T Base"/>'
        + f'<Tendon boneId="{rig.id("Tail Mid")}" tx="{b1:g}" ty="{y:g}" name="T Mid"/><Tendon boneId="{rig.id("Tail Tip")}" tx="{b2:g}" ty="{y:g}" name="T Tip"/></Skin>'
        + "</PointsPath>" + stroke(LINE, width + 6) + stroke(COAT, width) + "</Shape>"
    )


def tail_bones(rig: Rig, x: float, y: float, lengths: tuple[float, float, float], mid: float = -0.55, tip: float = -0.75) -> str:
    l0, l1, l2 = lengths
    return (
        f'<RootBone x="{x:g}" y="{y:g}" length="{l0:g}" rotation="0" name="Tail Base" id="{rig.id("Tail Base")}">'
        f'<Bone length="{l1:g}" rotation="{mid:g}" name="Tail Mid" id="{rig.id("Tail Mid")}">'
        f'<Bone length="{l2:g}" rotation="{tip:g}" name="Tail Tip" id="{rig.id("Tail Tip")}"/></Bone></RootBone>'
    )


def controls(rig: Rig, f: Fit) -> None:
    for obj, prop, v in [
        ("Root", "x", 200), ("Root", "y", 370),
        ("Head Pose", "x", 0), ("Head Pose", "y", f.head_y), ("Head Pose", "rotation", 0),
        ("Body Pose", "scaleX", 1), ("Body Pose", "scaleY", 1),
        ("Eyes Pose", "x", 0), ("Eyes Pose", "y", 2), ("Eyes Pose", "scaleY", 1), ("Eyes Pose", "opacity", 1),
        ("Brows", "opacity", 0), ("Frown", "opacity", 0),
        ("Eyes Happy", "opacity", 0), ("Eyes Closed", "opacity", 0),
        ("Mouth", "opacity", 1), ("Mouth Open", "opacity", 0), ("Mouth Open", "scaleY", 0.2),
        ("Ear L Pose", "rotation", -f.ear), ("Ear R Pose", "rotation", f.ear),
        ("Tail Base", "rotation", 0),
        ("Paw L", "y", f.paw_y), ("Paw R", "y", f.paw_y),
        ("Bowl", "opacity", 0), ("Food", "scaleY", 0),
        ("Bed", "opacity", 0), ("Cloud", "opacity", 0), ("Drops", "y", 0),
        ("Postcard", "opacity", 0),
        ("Confetti", "opacity", 0), ("Confetti", "scaleX", 0.3), ("Confetti", "scaleY", 0.3),
        ("Zzz", "opacity", 0), ("Zzz", "y", 150),
        ("Hearts", "opacity", 0), ("Hearts", "y", 120),
    ]:
        rig.control(obj, prop, v)


def ears(c: Clip, r: float, frames: list[tuple] | None = None) -> Clip:
    """Keys both ears: R at +r, L mirrored. `frames` overrides with (frame, r[, ease]) tuples."""
    fr = frames or [(0, r, "hold")]
    c.key("Ear L Pose", "rotation", *[(t[0], -t[1], *t[2:]) for t in fr])
    c.key("Ear R Pose", "rotation", *fr)
    return c


def ambient() -> list[Clip]:
    """Breathing, blinking, tail sway, ear twitch: always running, never in sync."""
    return [
        Clip("Breathe", 240, True).key("Body", "scaleY", (0, 1), (110, 1.025), (240, 1)).key("Body", "scaleX", (0, 1), (110, 1.01), (240, 1)).key("Head", "y", (0, 0), (118, -3), (240, 0)),
        Clip("Blink", 330, True).key("Eyes", "scaleY", (0, 1, "hold"), (120, 1), (125, 0.08), (132, 1, "hold"), (280, 1), (285, 0.08), (291, 1), (296, 0.08), (303, 1, "hold"), (330, 1)),
        Clip("Tail Sway", 420, True)
        .key("Tail Mid", "rotation", (0, -0.40), (40, -0.40), (250, -0.72), (420, -0.40))
        .key("Tail Tip", "rotation", (0, -0.95), (80, -0.95), (290, -0.45), (420, -0.95)),
        Clip("Ear Twitch", 540, True).key("Ear L", "rotation", (0, 0, "hold"), (300, 0), (305, -0.3), (312, 0.07), (320, 0, "hold"), (540, 0)),
    ]


def moods(f: Fit) -> list[Clip]:
    joyful = (Clip("Mood Joyful", 180, True)
              .key("Eyes Happy", "opacity", (0, 1, "hold")).key("Eyes Pose", "opacity", (0, 0, "hold"))
              .key("Tail Base", "rotation", (0, -0.35, "hold"))
              .key("Root", "y", (0, 370, "hold"), (120, 370), (136, 358), (152, 370, "hold"), (180, 370))
              .key("Body Pose", "scaleY", (0, 1, "hold"), (116, 1), (122, 0.94), (136, 1.04), (152, 0.97), (160, 1, "hold"), (180, 1)))
    ears(joyful, f.ear_up)
    content = Clip("Mood Content", 240, True)
    peckish = (Clip("Mood Peckish", 360, True)
               .key("Bowl", "opacity", (0, 1, "hold"))
               .key("Head Pose", "rotation", (0, 0, "hold"), (150, 0), (175, -0.16), (250, -0.16), (275, 0, "hold"), (360, 0))
               .key("Eyes Pose", "x", (0, 0, "hold"), (150, 0), (170, -7), (255, -7), (275, 0, "hold"), (360, 0)))
    hungry = (Clip("Mood Hungry", 300, True)
              .key("Bowl", "opacity", (0, 1, "hold"))
              .key("Body Pose", "scaleY", (0, f.lie_body, "hold")).key("Body Pose", "scaleX", (0, 1.08, "hold"))
              .key("Head Pose", "y", (0, f.lie_head), (200, f.lie_head), (230, f.lie_head + 4), (300, f.lie_head))
              .key("Eyes Pose", "scaleY", (0, 0.45, "hold"))
              .key("Tail Base", "rotation", (0, 0.25, "hold"))
              .key("Paw L", "y", (0, f.paw_y + 4, "hold")).key("Paw R", "y", (0, f.paw_y + 4, "hold")))
    ears(hungry, f.ear_low)
    gloomy = (Clip("Mood Gloomy", 120, True)
              .key("Cloud", "opacity", (0, 1, "hold"))
              .key("Drops", "y", (0, 0, "linear"), (120, 26))
              .key("Eyes Pose", "scaleY", (0, 0.62, "hold")).key("Eyes Pose", "y", (0, 7, "hold"))
              .key("Brows", "opacity", (0, 1, "hold")).key("Mouth", "opacity", (0, 0, "hold")).key("Frown", "opacity", (0, 1, "hold"))
              .key("Head Pose", "rotation", (0, 0.1, "hold")).key("Head Pose", "y", (0, f.head_y + 8, "hold"))
              .key("Body Pose", "scaleY", (0, 0.92, "hold"))
              .key("Tail Base", "rotation", (0, 0.3, "hold")))
    ears(gloomy, f.ear_flat)
    return [joyful, content, peckish, hungry, gloomy]


def walk_keys(c: Clip, f: Fit, x0: float, x1: float, start: int, end: int) -> Clip:
    """Scoot across: x moves, small hops each step, paws alternate."""
    xs = [(start, x0)]
    ys, pl, pr = [(start, 370)], [(start, f.paw_y)], [(start, f.paw_y)]
    steps = max(2, (end - start) // 20)
    for i in range(steps):
        a = start + (end - start) * (i + 0.5) / steps
        b = start + (end - start) * (i + 1) / steps
        ys += [(int(a), 362), (int(b), 370)]
        (pl if i % 2 else pr).append((int(a), f.paw_y - 8))
        (pl if i % 2 else pr).append((int(b), f.paw_y))
    xs.append((end, x1))
    return c.key("Root", "x", *[(t, v, "linear") for t, v in xs]).key("Root", "y", *ys).key("Paw L", "y", *pl).key("Paw R", "y", *pr)


def zoomies(f: Fit) -> Clip:
    """The cat's celebration (§10.3): dash right and left twice, then sit proudly."""
    zoom = [(0, 200), (25, 255), (50, 145), (75, 255), (100, 145), (125, 200)]
    return proud(Clip("Celebrate", 270, False)
                 .key("Root", "x", *zoom, (270, 200))
                 .key("Root", "y", (0, 370), (12, 356), (25, 370), (37, 356), (50, 370), (62, 356), (75, 370), (87, 356), (100, 370), (112, 356), (125, 370, "hold"), (270, 370)), f, 125)


def proud(c: Clip, f: Fit, start: int) -> Clip:
    """The shared end of every celebration: chest up, happy eyes, tail up, confetti from `start`."""
    end = c.frames
    return (c.key("Body Pose", "scaleY", (0, 1, "hold"), (start, 1), (start + 25, 1.06, "hold"), (end - 30, 1.06), (end, 1))
            .key("Head Pose", "y", (0, f.head_y, "hold"), (start, f.head_y), (start + 25, f.head_y - 8, "hold"), (end - 30, f.head_y - 8), (end, f.head_y))
            .key("Eyes Pose", "opacity", (0, 0, "hold"), (end - 20, 1, "hold"), (end, 1))
            .key("Eyes Happy", "opacity", (0, 1, "hold"), (end - 20, 0, "hold"), (end, 0))
            .key("Tail Base", "rotation", (0, -0.4, "hold"), (end - 30, -0.4), (end, 0))
            .key("Confetti", "opacity", (0, 0, "hold"), (start, 0), (start + 10, 1), (end - 50, 1), (end - 10, 0))
            .key("Confetti", "scaleX", (0, 0.3, "hold"), (start, 0.3), (start + 45, 1.15, "hold"), (end, 1.15))
            .key("Confetti", "scaleY", (0, 0.3, "hold"), (start, 0.3), (start + 45, 1.15, "hold"), (end, 1.15)))


def actions(f: Fit, celebrate: Callable[[Fit], Clip]) -> list[Clip]:
    hy, lb, lh = f.head_y, f.lie_body, f.lie_head
    eat = (Clip("Eat", 240, False)
           .key("Bowl", "opacity", (0, 0), (20, 1, "hold"), (220, 1), (240, 0))
           .key("Food", "scaleY", (0, 1, "hold"), (60, 1), (180, 0.15, "hold"), (240, 0.15))
           .key("Head Pose", "x", (0, 0), (40, f.eat_dx, "hold"), (190, f.eat_dx), (220, 0))
           .key("Head Pose", "y", (0, hy), *[(40 + 20 * i, hy + f.eat_dy - (0 if i % 2 == 0 else -6)) for i in range(8)], (190, hy + f.eat_dy), (220, hy))
           .key("Head Pose", "rotation", (0, 0), (40, -0.32, "hold"), (190, -0.32), (220, 0))
           .key("Eyes Pose", "opacity", (0, 1, "hold"), (40, 0, "hold"), (200, 1, "hold"), (240, 1))
           .key("Eyes Happy", "opacity", (0, 0, "hold"), (40, 1, "hold"), (200, 0, "hold"), (240, 0)))
    petted = (Clip("Petted", 150, False)
              .key("Eyes Pose", "opacity", (0, 0, "hold"), (140, 1, "hold"), (150, 1))
              .key("Eyes Happy", "opacity", (0, 1, "hold"), (140, 0, "hold"), (150, 0))
              .key("Head Pose", "rotation", (0, 0), (30, 0.16), (110, 0.16), (140, 0))
              .key("Hearts", "opacity", (0, 0), (15, 1), (110, 1), (140, 0)).key("Hearts", "y", (0, 120), (140, 70)))
    ears(petted, 0, [(0, f.ear), (20, f.ear_soft), (120, f.ear_soft), (140, f.ear)])
    exit_ = walk_keys(Clip("Exit", 180, False), f, 200, 560, 0, 180)
    enter = walk_keys(Clip("Enter", 180, False), f, 560, 200, 0, 180)
    fall_asleep = (Clip("Fall Asleep", 180, False)  # ends in the sleep pose
                   .key("Mouth Open", "opacity", (0, 0), (20, 1, "hold"), (70, 1), (80, 0, "hold"), (180, 0))
                   .key("Mouth Open", "scaleY", (0, 0.2), (40, 1), (70, 0.2, "hold"), (180, 0.2))
                   .key("Mouth", "opacity", (0, 1), (20, 0, "hold"), (80, 1, "hold"), (180, 1))
                   .key("Eyes Pose", "opacity", (0, 1, "hold"), (30, 0, "hold"), (180, 0))
                   .key("Eyes Closed", "opacity", (0, 0, "hold"), (30, 1, "hold"), (180, 1))
                   .key("Body Pose", "scaleY", (0, 1, "hold"), (80, 1), (140, lb, "hold"), (180, lb))
                   .key("Body Pose", "scaleX", (0, 1, "hold"), (80, 1), (140, 1.08, "hold"), (180, 1.08))
                   .key("Head Pose", "y", (0, hy, "hold"), (80, hy), (140, lh, "hold"), (180, lh))
                   .key("Tail Base", "rotation", (0, 0, "hold"), (80, 0), (140, 0.3, "hold"), (180, 0.3))
                   .key("Bed", "opacity", (0, 0, "hold"), (90, 0), (150, 1, "hold"), (180, 1)))
    sleep = (Clip("Sleep", 240, True)
             .key("Eyes Pose", "opacity", (0, 0, "hold")).key("Eyes Closed", "opacity", (0, 1, "hold"))
             .key("Body Pose", "scaleY", (0, lb, "hold")).key("Body Pose", "scaleX", (0, 1.08, "hold"))
             .key("Head Pose", "y", (0, lh, "hold")).key("Tail Base", "rotation", (0, 0.3, "hold"))
             .key("Bed", "opacity", (0, 1, "hold"))
             .key("Zzz", "opacity", (0, 0), (40, 1), (200, 1), (240, 0)).key("Zzz", "y", (0, 160, "linear"), (240, 120)))
    wake = (Clip("Wake", 180, False)  # starts from the sleep pose
            .key("Eyes Pose", "opacity", (0, 0, "hold"), (60, 1, "hold"), (180, 1))
            .key("Eyes Closed", "opacity", (0, 1, "hold"), (60, 0, "hold"), (180, 0))
            .key("Body Pose", "scaleY", (0, lb), (60, lb), (100, 0.86), (130, 1.04), (160, 1, "hold"), (180, 1))
            .key("Body Pose", "scaleX", (0, 1.08), (60, 1.08), (100, 1.18), (130, 0.98), (160, 1, "hold"), (180, 1))
            .key("Head Pose", "y", (0, lh), (60, lh), (110, hy + 18), (140, hy - 4), (160, hy, "hold"), (180, hy))
            .key("Mouth Open", "opacity", (0, 0, "hold"), (80, 1, "hold"), (130, 1), (140, 0, "hold"), (180, 0))
            .key("Mouth Open", "scaleY", (0, 0.2, "hold"), (80, 0.2), (105, 1), (130, 0.2, "hold"), (180, 0.2))
            .key("Mouth", "opacity", (0, 1, "hold"), (80, 0, "hold"), (140, 1, "hold"), (180, 1))
            .key("Tail Base", "rotation", (0, 0.3), (100, -0.5), (160, 0, "hold"), (180, 0))
            .key("Bed", "opacity", (0, 1, "hold"), (130, 1), (170, 0, "hold"), (180, 0)))
    postcard_in = walk_keys(Clip("Postcard", 240, False), f, 560, 200, 0, 170)
    postcard_in.key("Postcard", "opacity", (0, 1, "hold"), (200, 1), (230, 0, "hold"), (240, 0))
    postcard_in.key("Mouth", "opacity", (0, 0, "hold"), (230, 1, "hold"), (240, 1))
    walk = walk_keys(Clip("Walk", 120, True), f, 200, 200, 0, 120)
    return [eat, petted, celebrate(f), exit_, enter, fall_asleep, sleep, wake, postcard_in, walk]


def accessories(rig: Rig, f: Fit) -> tuple[str, str, list[Clip]]:
    """The five graduation accessories: (on top of the head, on top of the body, one clip per `accessory` value).
    All hidden; the Accessory layer shows the one `accessory` picks (0 = none)."""
    N, w, top = rig.node, f.head_w, f.crown
    cap = N("Acc Cap",
            shape("Tassel End", ellipse(8, 11), fill(solid(TASSEL)), x=38, y=26)
            + shape("Tassel", path([(0, 0), (34, 4), (38, 22)], radius=6), stroke(solid(TASSEL), 3))
            + shape("Board", path([(-48, 0), (0, -13), (48, 0), (0, 13)], closed=True, radius=2), fill(solid(CAP)) + stroke(solid("FF55556A"), 2.5))
            + shape("Skull", rect(56, 24, 6), fill(solid(CAP)) + stroke(solid("FF55556A"), 2.5), y=10),
            y=top - 6, rotation=-0.12, opacity=0.0)
    bow = N("Acc Bow",
            shape("Knot", ellipse(11, 11), fill(solid(BOW)) + stroke(LINE, 2))
            + shape("Loop L", ellipse(24, 16), fill(solid(BOW)) + stroke(LINE, 2), x=-13, rotation=0.3)
            + shape("Loop R", ellipse(24, 16), fill(solid(BOW)) + stroke(LINE, 2), x=13, rotation=-0.3),
            x=w * 0.3, y=top + 10, rotation=0.35, opacity=0.0)
    spots = [(-w * 0.4, top + 18), (-w * 0.21, top + 5), (0, top + 1), (w * 0.21, top + 5), (w * 0.4, top + 18)]
    crown = N("Acc Crown",
              "".join(shape(f"Centre {i}", ellipse(5, 5), fill(solid(TASSEL)), x=x, y=y) + shape(f"Flower {i}", ellipse(15, 15), fill(solid(FLOWERS[i])) + stroke(LINE, 2), x=x, y=y)
                      for i, (x, y) in enumerate(spots))
              + shape("Vine", curve(spots), stroke(solid(LEAF), 4)),
              opacity=0.0)
    scarf = N("Acc Scarf",
              shape("Stripe", path([(-w * 0.3, 0), (w * 0.3, 0)]), stroke(solid("FFF6F1E7"), 3))
              + shape("Band", rect(w * 0.78, 20, 10), fill(solid(SCARF)) + stroke(LINE, 2.5))
              + shape("Scarf End", rect(18, 44, 6), fill(solid(SCARF)) + stroke(LINE, 2.5), x=w * 0.22, y=22, rotation=-0.15),
              y=f.chin, opacity=0.0)
    pts = [(math.cos(-math.pi / 2 + i * math.pi / 5) * (17 if i % 2 == 0 else 7), math.sin(-math.pi / 2 + i * math.pi / 5) * (17 if i % 2 == 0 else 7)) for i in range(10)]
    star = N("Acc Star", shape("Star Shape", path(pts, closed=True, radius=1.5), fill(solid(STAR)) + stroke(LINE, 2.5)), x=f.chest[0], y=f.chest[1], opacity=0.0)
    clips = []
    for k, name in enumerate(["None"] + ACCESSORIES):
        c = Clip(f"Accessory {name}", 1, False)
        for j, other in enumerate(ACCESSORIES):
            c.key(f"Acc {other}", "opacity", (0, 1 if j + 1 == k else 0))
        clips.append(c)
    return cap + bow + crown + scarf, star, clips
