"""Persian cat rig: writes scene.rml. Run from anywhere: python3 art/cat/make.py

Structure follows art/STYLE.md. Pose nodes (…Pose) are keyed by the mood and action clips; the
nodes inside them (Body, Head, Eyes, Ear L/R, Tail Mid/Tip) carry the ambient loops, so breathing,
blinking and the tail sway keep running under every pose.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "tools"))
from rml import (COAT, HIGH, LINE, Clip, Rig, coat_fill, curve, ellipse, fill, path, rect, shape, solid,  # noqa: E402
                 stroke, triangle, write)

rig = Rig("Cat")
N = rig.node

# ---- fixed palette (art/STYLE.md) ----
IRIS, PUPIL, WHITE = "FFD9902E", "FF2A1E18", "FFFFFFFF"
NOSE, INNER_EAR, BLUSH = "FFE79A9A", "FFF0B4B0", "40E77A7A"
BOWL, FOOD = "FF5B8DEF", "FFC98A4B"
BED, CUSHION = "FFB9A2E0", "FFE9DFF8"
CLOUD, DROP = "FF9AA4B5", "FF6FA8DC"
CARD, STAMP = "FFFFF6E0", "FFE2725B"
CONFETTI = ["FFF2C14E", "FFE2725B", "FF5B8DEF", "FF7BC47F"]
ZZZ, HEART = "FF8E8EA0", "FFE77A8F"


def eye(side: int) -> str:
    x = 27 * side
    return (
        shape(f"Glint {side}", ellipse(7, 7), fill(solid(WHITE)), x=x + 6, y=-6)
        + shape(f"Pupil {side}", ellipse(15, 22), fill(solid(PUPIL)), x=x, y=1)
        + shape(f"Iris {side}", ellipse(30, 31), fill(solid(IRIS)) + stroke(LINE, 2.5), x=x)
    )


def ear(side: int) -> str:
    name = "L" if side < 0 else "R"
    inner = shape("Inner", triangle(18, 18), fill(solid(INNER_EAR)), y=-6)
    outer = shape("Outer", triangle(36, 34), coat_fill(), y=-8)
    return N(f"Ear {name} Pose", N(f"Ear {name}", inner + outer), x=40 * side, y=-40, rotation=0.45 * side)


def z(size: float, x: float, y: float, i: int) -> str:
    s = size / 2
    return shape(f"Z{i}", path([(-s, -s), (s, -s), (-s, s), (s, s)]), stroke(solid(ZZZ), 3), x=x, y=y)


def heart(x: float, y: float, i: int) -> str:
    pts = [(0, 6), (-7, -1), (-6, -6), (-2, -7), (0, -4), (2, -7), (6, -6), (7, -1)]
    return shape(f"Heart {i}", curve(pts, closed=True), fill(solid(HEART)), x=x, y=y)


# ---- head ----
eyes_open = N("Eyes Pose", N("Eyes", eye(-1) + eye(1)), y=2)
arc = lambda x: path([(x - 12, 4), (x - 6, -4), (x + 6, -4), (x + 12, 4)], radius=6)  # noqa: E731
eyes_happy = N("Eyes Happy", shape("Arcs L", arc(-27), stroke(solid(PUPIL), 4)) + shape("Arcs R", arc(27), stroke(solid(PUPIL), 4)), y=2, opacity=0.0)
lid = lambda x: path([(x - 13, 0), (x, 5), (x + 13, 0)], radius=8)  # noqa: E731
eyes_closed = N("Eyes Closed", shape("Lid L", lid(-27), stroke(solid(PUPIL), 4)) + shape("Lid R", lid(27), stroke(solid(PUPIL), 4)), y=4, opacity=0.0)
brows = N("Brows", shape("Brow L", path([(-38, -22), (-18, -27)]), stroke(LINE, 3)) + shape("Brow R", path([(38, -22), (18, -27)]), stroke(LINE, 3)), opacity=0.0)
frown = shape("Frown", path([(-8, 35), (0, 30), (8, 35)], radius=6), stroke(LINE, 2.5), sid=rig.id("Frown"), opacity=0.0)
mouth = shape("Mouth", path([(-9, 31), (-4, 35), (0, 30), (4, 35), (9, 31)], radius=3), stroke(LINE, 2.5), sid=rig.id("Mouth"))
mouth_open = N("Mouth Open", shape("Yawn", ellipse(16, 18), fill(solid("FF8A3B3B")) + stroke(LINE, 2.5)), y=36, opacity=0.0, scaleY=0.2)
postcard = N(
    "Postcard",
    shape("Stamp", rect(9, 11, 1), fill(solid(STAMP)), x=10, y=-4)
    + shape("Lines", path([(-14, -2), (2, -2)]), stroke(solid("FFB8AFA0"), 2))
    + shape("Card", rect(40, 28, 3), fill(solid(CARD)) + stroke(LINE, 2.5)),
    y=44, rotation=-0.12, opacity=0.0,
)
head = N(
    "Head",
    postcard
    + brows + eyes_open + eyes_happy + eyes_closed
    + shape("Nose", triangle(14, 10), fill(solid(NOSE)), y=22, rotation=3.1415927)
    + mouth + frown + mouth_open
    + shape("Blush L", ellipse(18, 9), fill(solid(BLUSH)), x=-44, y=24)
    + shape("Blush R", ellipse(18, 9), fill(solid(BLUSH)), x=44, y=24)
    + shape("Head Shape", ellipse(148, 118), coat_fill())
    + shape("Cheek L", ellipse(58, 44), coat_fill(), x=-44, y=26)
    + shape("Cheek R", ellipse(58, 44), coat_fill(), x=44, y=26)
    + ear(-1) + ear(1),
)

# ---- body ----
body = N(
    "Body",
    shape("Paw L", ellipse(44, 26), coat_fill(), x=-30, y=-10, sid=rig.id("Paw L"))
    + shape("Paw R", ellipse(44, 26), coat_fill(), x=30, y=-10, sid=rig.id("Paw R"))
    + shape("Chest Fluff", ellipse(86, 76), fill(HIGH), y=-82)
    + shape("Body Shape", ellipse(156, 144), coat_fill(), y=-70),
)

tail_bones = (
    f'<RootBone x="45" y="-18" length="37" rotation="0" name="Tail Base" id="{rig.id("Tail Base")}">'
    f'<Bone length="36" rotation="-0.55" name="Tail Mid" id="{rig.id("Tail Mid")}">'
    f'<Bone length="32" rotation="-0.75" name="Tail Tip" id="{rig.id("Tail Tip")}"/></Bone></RootBone>'
)

cat = N("Cat", N("Head Pose", head, y=-178) + N("Body Pose", body) + tail_bones, x=200, y=370)

# tail: thick stroke skinned to the 3-bone chain, bound in artboard space at the cat's rest position
verts = []
for i in range(20):
    x = 238 + 6 * i
    if x <= 262:
        w = '<Weight values="255" indices="1"/>'
    elif x < 304:
        t = (x - 263.5) / 36.5
        a = round(255 * (1 - t)); b = 255 - a
        w = f'<Weight values="{a | (b << 8)}" indices="{1 | (2 << 8)}"/>'
    elif x < 334:
        t = (x - 300) / 34
        a = round(255 * (1 - t)); b = 255 - a
        w = f'<Weight values="{a | (b << 8)}" indices="{2 | (3 << 8)}"/>'
    else:
        w = '<Weight values="255" indices="3"/>'
    verts.append(f'<StraightVertex x="{x}" y="352" radius="20">{w}</StraightVertex>')
tail = (
    '<Shape name="Tail"><PointsPath isClosed="false" name="Path">' + "".join(verts)
    + f'<Skin tx="0" ty="0" name="Skin"><Tendon boneId="{rig.id("Tail Base")}" tx="245" ty="352" name="T Base"/>'
    + f'<Tendon boneId="{rig.id("Tail Mid")}" tx="282" ty="352" name="T Mid"/><Tendon boneId="{rig.id("Tail Tip")}" tx="318" ty="352" name="T Tip"/></Skin>'
    + "</PointsPath>" + stroke(LINE, 38) + stroke(COAT, 32) + "</Shape>"
)

# ---- props ----
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
zzz = N("Zzz", z(14, 0, 0, 1) + z(18, 18, -24, 2) + z(24, 40, -54, 3), x=262, y=150, opacity=0.0)
hearts = N("Hearts", heart(-34, 0, 1) + heart(0, -16, 2) + heart(34, -4, 3), x=200, y=120, opacity=0.0)

children = hearts + confetti + zzz + cloud + cat + tail + bowl + bed

# ---- pose-controlled properties and their rest values ----
for obj, prop, v in [
    ("Cat", "x", 200), ("Cat", "y", 370),
    ("Head Pose", "x", 0), ("Head Pose", "y", -178), ("Head Pose", "rotation", 0),
    ("Body Pose", "scaleX", 1), ("Body Pose", "scaleY", 1),
    ("Eyes Pose", "x", 0), ("Eyes Pose", "y", 2), ("Eyes Pose", "scaleY", 1),
    ("Brows", "opacity", 0), ("Frown", "opacity", 0), ("Eyes Pose", "opacity", 1),
    ("Eyes Happy", "opacity", 0), ("Eyes Closed", "opacity", 0),
    ("Mouth", "opacity", 1), ("Mouth Open", "opacity", 0), ("Mouth Open", "scaleY", 0.2),
    ("Ear L Pose", "rotation", -0.45), ("Ear R Pose", "rotation", 0.45),
    ("Tail Base", "rotation", 0),
    ("Paw L", "y", -10), ("Paw R", "y", -10),
    ("Bowl", "opacity", 0), ("Food", "scaleY", 0),
    ("Bed", "opacity", 0), ("Cloud", "opacity", 0), ("Drops", "y", 0),
    ("Postcard", "opacity", 0),
    ("Confetti", "opacity", 0), ("Confetti", "scaleX", 0.3), ("Confetti", "scaleY", 0.3),
    ("Zzz", "opacity", 0), ("Zzz", "y", 150),
    ("Hearts", "opacity", 0), ("Hearts", "y", 120),
]:
    rig.control(obj, prop, v)

# ---- ambient loops (always running; different lengths so they never sync) ----
breathe = Clip("Breathe", 240, True).key("Body", "scaleY", (0, 1), (110, 1.025), (240, 1)).key("Body", "scaleX", (0, 1), (110, 1.01), (240, 1)).key("Head", "y", (0, 0), (118, -3), (240, 0))
blink = Clip("Blink", 330, True).key("Eyes", "scaleY", (0, 1, "hold"), (120, 1), (125, 0.08), (132, 1, "hold"), (280, 1), (285, 0.08), (291, 1), (296, 0.08), (303, 1, "hold"), (330, 1))
sway = (Clip("Tail Sway", 420, True)
        .key("Tail Mid", "rotation", (0, -0.40), (40, -0.40), (250, -0.72), (420, -0.40))
        .key("Tail Tip", "rotation", (0, -0.95), (80, -0.95), (290, -0.45), (420, -0.95)))
twitch = Clip("Ear Twitch", 540, True).key("Ear L", "rotation", (0, 0, "hold"), (300, 0), (305, -0.3), (312, 0.07), (320, 0, "hold"), (540, 0))

# ---- mood poses (loop while mood = 1…5 and no action) ----
LIE_BODY, LIE_HEAD = 0.72, -138  # lying down: body squashed from the ground, head lowered onto it

joyful = (Clip("Mood Joyful", 180, True)
          .key("Eyes Happy", "opacity", (0, 1, "hold")).key("Eyes Pose", "opacity", (0, 0, "hold"))
          .key("Tail Base", "rotation", (0, -0.35, "hold"))
          .key("Cat", "y", (0, 370, "hold"), (120, 370), (136, 358), (152, 370, "hold"), (180, 370))
          .key("Body Pose", "scaleY", (0, 1, "hold"), (116, 1), (122, 0.94), (136, 1.04), (152, 0.97), (160, 1, "hold"), (180, 1))
          .key("Ear L Pose", "rotation", (0, -0.35, "hold")).key("Ear R Pose", "rotation", (0, 0.35, "hold")))
content = Clip("Mood Content", 240, True)
peckish = (Clip("Mood Peckish", 360, True)
           .key("Bowl", "opacity", (0, 1, "hold"))
           .key("Head Pose", "rotation", (0, 0, "hold"), (150, 0), (175, -0.16), (250, -0.16), (275, 0, "hold"), (360, 0))
           .key("Eyes Pose", "x", (0, 0, "hold"), (150, 0), (170, -7), (255, -7), (275, 0, "hold"), (360, 0)))
hungry = (Clip("Mood Hungry", 300, True)
          .key("Bowl", "opacity", (0, 1, "hold"))
          .key("Body Pose", "scaleY", (0, LIE_BODY, "hold")).key("Body Pose", "scaleX", (0, 1.08, "hold"))
          .key("Head Pose", "y", (0, LIE_HEAD), (200, LIE_HEAD), (230, LIE_HEAD + 4), (300, LIE_HEAD))
          .key("Eyes Pose", "scaleY", (0, 0.45, "hold"))
          .key("Tail Base", "rotation", (0, 0.25, "hold"))
          .key("Paw L", "y", (0, -6, "hold")).key("Paw R", "y", (0, -6, "hold"))
          .key("Ear L Pose", "rotation", (0, -0.7, "hold")).key("Ear R Pose", "rotation", (0, 0.7, "hold")))
gloomy = (Clip("Mood Gloomy", 120, True)
          .key("Cloud", "opacity", (0, 1, "hold"))
          .key("Drops", "y", (0, 0, "linear"), (120, 26))
          .key("Eyes Pose", "scaleY", (0, 0.62, "hold")).key("Eyes Pose", "y", (0, 7, "hold"))
          .key("Brows", "opacity", (0, 1, "hold")).key("Mouth", "opacity", (0, 0, "hold")).key("Frown", "opacity", (0, 1, "hold"))
          .key("Head Pose", "rotation", (0, 0.1, "hold")).key("Head Pose", "y", (0, -170, "hold"))
          .key("Body Pose", "scaleY", (0, 0.92, "hold"))
          .key("Tail Base", "rotation", (0, 0.3, "hold"))
          .key("Ear L Pose", "rotation", (0, -1.05, "hold")).key("Ear R Pose", "rotation", (0, 1.05, "hold")))


# ---- actions (one-off clips; each starts and ends on the seated rest pose unless noted) ----
def walk_keys(c: Clip, x0: float, x1: float, start: int, end: int) -> Clip:
    """Scoot across: x moves, small hops each step, paws alternate."""
    xs = [(start, x0)]
    ys, pl, pr = [(start, 370)], [(start, -10)], [(start, -10)]
    steps = max(2, (end - start) // 20)
    for i in range(steps):
        f = start + (end - start) * (i + 0.5) / steps
        g = start + (end - start) * (i + 1) / steps
        ys += [(int(f), 362), (int(g), 370)]
        (pl if i % 2 else pr).append((int(f), -18))
        (pl if i % 2 else pr).append((int(g), -10))
    xs.append((end, x1))
    c.key("Cat", "x", *[(f, v, "linear") for f, v in xs]).key("Cat", "y", *ys).key("Paw L", "y", *pl).key("Paw R", "y", *pr)
    return c


eat = (Clip("Eat", 240, False)
       .key("Bowl", "opacity", (0, 0), (20, 1, "hold"), (220, 1), (240, 0))
       .key("Food", "scaleY", (0, 1, "hold"), (60, 1), (180, 0.15, "hold"), (240, 0.15))
       .key("Head Pose", "x", (0, 0), (40, -46, "hold"), (190, -46), (220, 0))
       .key("Head Pose", "y", (0, -178), (40, -132), (60, -126), (80, -132), (100, -126), (120, -132), (140, -126), (160, -132), (190, -132), (220, -178))
       .key("Head Pose", "rotation", (0, 0), (40, -0.32, "hold"), (190, -0.32), (220, 0))
       .key("Eyes Pose", "opacity", (0, 1, "hold"), (40, 0, "hold"), (200, 1, "hold"), (240, 1))
       .key("Eyes Happy", "opacity", (0, 0, "hold"), (40, 1, "hold"), (200, 0, "hold"), (240, 0)))
petted = (Clip("Petted", 150, False)
          .key("Eyes Pose", "opacity", (0, 0, "hold"), (140, 1, "hold"), (150, 1))
          .key("Eyes Happy", "opacity", (0, 1, "hold"), (140, 0, "hold"), (150, 0))
          .key("Head Pose", "rotation", (0, 0), (30, 0.16), (110, 0.16), (140, 0))
          .key("Ear L Pose", "rotation", (0, -0.45), (20, -0.8), (120, -0.8), (140, -0.45))
          .key("Ear R Pose", "rotation", (0, 0.45), (20, 0.8), (120, 0.8), (140, 0.45))
          .key("Hearts", "opacity", (0, 0), (15, 1), (110, 1), (140, 0)).key("Hearts", "y", (0, 120), (140, 70)))
zoom = []  # zoomies: dash right and left twice, then sit proudly
for i, (f, x) in enumerate([(0, 200), (25, 255), (50, 145), (75, 255), (100, 145), (125, 200)]):
    zoom.append((f, x))
celebrate = (Clip("Celebrate", 270, False)
             .key("Cat", "x", *zoom, (270, 200))
             .key("Cat", "y", (0, 370), (12, 356), (25, 370), (37, 356), (50, 370), (62, 356), (75, 370), (87, 356), (100, 370), (112, 356), (125, 370, "hold"), (270, 370))
             .key("Body Pose", "scaleY", (0, 1, "hold"), (125, 1), (150, 1.06, "hold"), (240, 1.06), (270, 1))
             .key("Head Pose", "y", (0, -178, "hold"), (125, -178), (150, -186, "hold"), (240, -186), (270, -178))
             .key("Eyes Pose", "opacity", (0, 0, "hold"), (250, 1, "hold"), (270, 1))
             .key("Eyes Happy", "opacity", (0, 1, "hold"), (250, 0, "hold"), (270, 0))
             .key("Tail Base", "rotation", (0, -0.4, "hold"), (240, -0.4), (270, 0))
             .key("Confetti", "opacity", (0, 0, "hold"), (125, 0), (135, 1), (220, 1), (260, 0))
             .key("Confetti", "scaleX", (0, 0.3, "hold"), (125, 0.3), (170, 1.15, "hold"), (270, 1.15))
             .key("Confetti", "scaleY", (0, 0.3, "hold"), (125, 0.3), (170, 1.15, "hold"), (270, 1.15)))
exit_ = walk_keys(Clip("Exit", 180, False), 200, 560, 0, 180)
enter = walk_keys(Clip("Enter", 180, False), 560, 200, 0, 180)
fall_asleep = (Clip("Fall Asleep", 180, False)  # ends in the sleep pose
               .key("Mouth Open", "opacity", (0, 0), (20, 1, "hold"), (70, 1), (80, 0, "hold"), (180, 0))
               .key("Mouth Open", "scaleY", (0, 0.2), (40, 1), (70, 0.2, "hold"), (180, 0.2))
               .key("Mouth", "opacity", (0, 1), (20, 0, "hold"), (80, 1, "hold"), (180, 1))
               .key("Eyes Pose", "opacity", (0, 1, "hold"), (30, 0, "hold"), (180, 0))
               .key("Eyes Closed", "opacity", (0, 0, "hold"), (30, 1, "hold"), (180, 1))
               .key("Body Pose", "scaleY", (0, 1, "hold"), (80, 1), (140, LIE_BODY, "hold"), (180, LIE_BODY))
               .key("Body Pose", "scaleX", (0, 1, "hold"), (80, 1), (140, 1.08, "hold"), (180, 1.08))
               .key("Head Pose", "y", (0, -178, "hold"), (80, -178), (140, LIE_HEAD, "hold"), (180, LIE_HEAD))
               .key("Tail Base", "rotation", (0, 0, "hold"), (80, 0), (140, 0.3, "hold"), (180, 0.3))
               .key("Bed", "opacity", (0, 0, "hold"), (90, 0), (150, 1, "hold"), (180, 1)))
sleep = (Clip("Sleep", 240, True)
         .key("Eyes Pose", "opacity", (0, 0, "hold")).key("Eyes Closed", "opacity", (0, 1, "hold"))
         .key("Body Pose", "scaleY", (0, LIE_BODY, "hold")).key("Body Pose", "scaleX", (0, 1.08, "hold"))
         .key("Head Pose", "y", (0, LIE_HEAD, "hold")).key("Tail Base", "rotation", (0, 0.3, "hold"))
         .key("Bed", "opacity", (0, 1, "hold"))
         .key("Zzz", "opacity", (0, 0), (40, 1), (200, 1), (240, 0)).key("Zzz", "y", (0, 160, "linear"), (240, 120)))
wake = (Clip("Wake", 180, False)  # starts from the sleep pose
        .key("Eyes Pose", "opacity", (0, 0, "hold"), (60, 1, "hold"), (180, 1))
        .key("Eyes Closed", "opacity", (0, 1, "hold"), (60, 0, "hold"), (180, 0))
        .key("Body Pose", "scaleY", (0, LIE_BODY), (60, LIE_BODY), (100, 0.86), (130, 1.04), (160, 1, "hold"), (180, 1))
        .key("Body Pose", "scaleX", (0, 1.08), (60, 1.08), (100, 1.18), (130, 0.98), (160, 1, "hold"), (180, 1))
        .key("Head Pose", "y", (0, LIE_HEAD), (60, LIE_HEAD), (110, -160), (140, -182), (160, -178, "hold"), (180, -178))
        .key("Mouth Open", "opacity", (0, 0, "hold"), (80, 1, "hold"), (130, 1), (140, 0, "hold"), (180, 0))
        .key("Mouth Open", "scaleY", (0, 0.2, "hold"), (80, 0.2), (105, 1), (130, 0.2, "hold"), (180, 0.2))
        .key("Mouth", "opacity", (0, 1, "hold"), (80, 0, "hold"), (140, 1, "hold"), (180, 1))
        .key("Tail Base", "rotation", (0, 0.3), (100, -0.5), (160, 0, "hold"), (180, 0))
        .key("Bed", "opacity", (0, 1, "hold"), (130, 1), (170, 0, "hold"), (180, 0)))
postcard_in = walk_keys(Clip("Postcard", 240, False), 560, 200, 0, 170)
postcard_in.key("Postcard", "opacity", (0, 1, "hold"), (200, 1), (230, 0, "hold"), (240, 0))
postcard_in.key("Mouth", "opacity", (0, 0, "hold"), (230, 1, "hold"), (240, 1))
walk = walk_keys(Clip("Walk", 120, True), 200, 200, 0, 120)

write(
    rig,
    children,
    ambient=[breathe, blink, sway, twitch],
    moods=[joyful, content, peckish, hungry, gloomy],
    actions=[eat, petted, celebrate, exit_, enter, fall_asleep, sleep, wake, postcard_in, walk],
    instance={"coatBase": "FFEFE1CC", "coatShade": "FFD9C4A6", "coatHighlight": "FFFFF8EE", "outline": "FF7A5C46"},
    path_out=os.path.join(os.path.dirname(__file__), "scene.rml"),
)
print("wrote scene.rml")
