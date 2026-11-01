"""Pembroke Welsh Corgi rig: writes scene.rml. Run from anywhere: python3 art/dog/make.py

Shared poses, clips and props come from art/tools/pet.py. This file draws the corgi (tall pointed
ears, white blaze, muzzle, bib and socks, short legs, fluffy stub tail) and its celebration:
spins, then a play bow (spec §10.3).
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "tools"))
from pet import accessories, INNER_EAR, PUPIL, Fit, actions, ambient, controls, face, moods, proud, props, skinned_tail, tail_bones  # noqa: E402
from rml import Clip, Rig, coat_fill, curve, ellipse, fill, rect, shape, solid, stroke, triangle, write, LINE, SHADE  # noqa: E402

rig = Rig("Dog")
N = rig.node
# a corgi's head sits a little lower on its long body; ears stand tall and barely flop
fit = Fit(crown=-61, chin=58, head_w=156, chest=(34, -92), head_y=-170, lie_head=-132, eye_dx=30, ear=0.22, ear_up=0.12, ear_soft=0.55, ear_low=0.7, ear_flat=1.1, mouth_y=42, eat_dx=-48, eat_dy=48)
MARK = "FFFFFAF2"  # corgi white markings are part of the breed, so they don't follow the coat colour


def ear(side: int) -> str:
    name = "L" if side < 0 else "R"
    inner = shape("Inner", triangle(26, 38), fill(solid(INNER_EAR)), y=-4)
    outer = shape("Outer", triangle(54, 66), coat_fill(), y=-10)
    return N(f"Ear {name} Pose", N(f"Ear {name}", inner + outer), x=44 * side, y=-46, rotation=fit.ear * side)


acc_head, acc_body, acc_clips = accessories(rig, fit)
head = N(
    "Head",
    acc_head
    + face(rig, fit)
    + shape("Nose", ellipse(20, 13), fill(solid(PUPIL)), y=24)
    + shape("Muzzle", ellipse(66, 40), fill(solid(MARK)) + stroke(LINE, 2.5), y=34)
    + shape("Blaze", curve([(0, -58), (8, -30), (6, 0), (0, 10), (-6, 0), (-8, -30)], closed=True), fill(solid(MARK)), y=0)
    + shape("Head Shape", ellipse(156, 122), coat_fill())
    + shape("Cheek L", ellipse(52, 40), coat_fill(), x=-50, y=22)
    + shape("Cheek R", ellipse(52, 40), coat_fill(), x=50, y=22)
    + ear(-1) + ear(1),
)
body = N(
    "Body",
    acc_body
    + shape("Paw L", ellipse(36, 20), fill(solid(MARK)) + stroke(LINE), x=-28, y=fit.paw_y, sid=rig.id("Paw L"))
    + shape("Paw R", ellipse(36, 20), fill(solid(MARK)) + stroke(LINE), x=28, y=fit.paw_y, sid=rig.id("Paw R"))
    + shape("Leg L", rect(28, 40, 12), coat_fill(), x=-28, y=-26)
    + shape("Leg R", rect(28, 40, 12), coat_fill(), x=28, y=-26)
    + shape("Bib", curve([(0, -126), (36, -104), (34, -60), (0, -40), (-34, -60), (-36, -104)], closed=True), fill(solid(MARK)))
    + shape("Haunch L", ellipse(50, 38), fill(SHADE) + stroke(LINE), x=-54, y=-20)
    + shape("Haunch R", ellipse(50, 38), fill(SHADE) + stroke(LINE), x=54, y=-20)
    + shape("Body Shape", ellipse(150, 132), coat_fill(), y=-70),
)
root = N("Root", N("Head Pose", head, y=fit.head_y) + N("Body Pose", body) + tail_bones(rig, 60, -26, (18, 16, 14), mid=-0.6, tip=-0.5), x=200, y=370)
tail = skinned_tail(rig, 254, 4, 13, 344, (260, 278, 294), width=30)


def spin_and_bow(f: Fit) -> Clip:
    """Two quick spins (flips with hops), a play bow with a wagging tail, then sits proudly."""
    hy = f.head_y
    c = Clip("Celebrate", 300, False)
    c.key("Root", "scaleX", (0, 1), (15, -1), (30, 1), (45, -1), (60, 1, "hold"), (300, 1))
    c.key("Root", "y", (0, 370), (8, 356), (15, 370), (23, 356), (30, 370), (38, 356), (45, 370), (53, 356), (60, 370, "hold"), (300, 370))
    c.key("Head Pose", "rotation", (0, 0, "hold"), (70, 0), (90, -0.14, "hold"), (150, -0.14), (165, 0, "hold"), (300, 0))
    c.key("Tail Base", "rotation", (0, -0.4), *[(70 + 10 * i, -0.6 if i % 2 else -0.1) for i in range(9)], (170, -0.4, "hold"), (300, -0.4))
    proud(c, f, 170)
    # the bow lives between the spins and the proud sit, so add it over proud's holds
    c.key("Body Pose", "scaleY", (0, 1, "hold"), (70, 1), (90, 0.8, "hold"), (150, 0.8), (195, 1.06, "hold"), (270, 1.06), (300, 1))
    c.key("Head Pose", "y", (0, hy, "hold"), (70, hy), (90, hy + 40, "hold"), (150, hy + 40), (195, hy - 8, "hold"), (270, hy - 8), (300, hy))
    return c


rig.control("Root", "scaleX", 1)
above, below = props(rig)
controls(rig, fit)
write(
    rig,
    above + root + tail + below,
    ambient=ambient(),
    moods=moods(fit),
    actions=actions(fit, spin_and_bow),
    instance={"coatBase": "FFD98A4A", "coatShade": "FFB86C2E", "coatHighlight": "FFF2B57E", "outline": "FF6B3A17"},
    path_out=os.path.join(os.path.dirname(__file__), "scene.rml"),
    accessories=acc_clips,
)
print("wrote scene.rml")
