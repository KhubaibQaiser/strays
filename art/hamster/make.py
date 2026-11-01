"""Syrian hamster rig: writes scene.rml. Run from anywhere: python3 art/hamster/make.py

Shared poses, clips and props come from art/tools/pet.py. This file draws the hamster (round body
with no neck, small round ears, cream belly, muzzle and cheek pouches, a tiny stub tail) and its
celebration: stuffs its cheeks, then hops (spec §10.3).
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "tools"))
from pet import accessories, INNER_EAR, NOSE, Fit, actions, ambient, controls, face, moods, proud, props, skinned_tail, tail_bones  # noqa: E402
from rml import Clip, Rig, coat_fill, ellipse, fill, path, shape, solid, stroke, write, LINE  # noqa: E402

rig = Rig("Hamster")
N = rig.node
# the head sits right on the body; round ears barely move, so their poses are small
fit = Fit(crown=-61, chin=60, head_w=150, chest=(36, -82), head_y=-150, lie_body=0.78, lie_head=-120, eye_dx=30, ear=0.25, ear_up=0.12, ear_soft=0.5, ear_low=0.55, ear_flat=0.85,
          paw_y=-6, mouth_y=42, eat_dx=-58, eat_dy=58)
MARK = "FFFFF3DE"  # cream belly and muzzle, part of the Syrian look, so they don't follow the coat colour
PINK = "FFF2B8B0"


def ear(side: int) -> str:
    name = "L" if side < 0 else "R"
    inner = shape("Inner", ellipse(20, 20), fill(solid(INNER_EAR)), y=-2)
    outer = shape("Outer", ellipse(36, 36), coat_fill())
    return N(f"Ear {name} Pose", N(f"Ear {name}", inner + outer), x=48 * side, y=-50, rotation=fit.ear * side)


def cheek(side: int) -> str:
    name = "L" if side < 0 else "R"
    return N(f"Cheek {name} Pose", shape(f"Pouch {name}", ellipse(62, 50), fill(solid(MARK)) + stroke(LINE, 2.5)), x=46 * side, y=30)


def whiskers(side: int) -> str:
    return "".join(shape(f"Whisker {side}{i}", path([(0, 0), (24 * side, dy)]), stroke(LINE, 1.5), x=26 * side, y=30 + 6 * i) for i, dy in enumerate((-6, 0, 6)))


acc_head, acc_body, acc_clips = accessories(rig, fit)
head = N(
    "Head",
    acc_head
    + face(rig, fit)
    + shape("Nose", ellipse(14, 9), fill(solid(NOSE)), y=24)
    + whiskers(-1) + whiskers(1)
    + shape("Muzzle", ellipse(56, 34), fill(solid(MARK)), y=32)
    + cheek(-1) + cheek(1)
    + shape("Head Shape", ellipse(150, 122), coat_fill())
    + ear(-1) + ear(1),
)
body = N(
    "Body",
    acc_body
    + shape("Paw L", ellipse(26, 14), fill(solid(PINK)) + stroke(LINE, 2), x=-34, y=fit.paw_y, sid=rig.id("Paw L"))
    + shape("Paw R", ellipse(26, 14), fill(solid(PINK)) + stroke(LINE, 2), x=34, y=fit.paw_y, sid=rig.id("Paw R"))
    + shape("Hand L", ellipse(16, 12), fill(solid(PINK)) + stroke(LINE, 2), x=-16, y=-52)
    + shape("Hand R", ellipse(16, 12), fill(solid(PINK)) + stroke(LINE, 2), x=16, y=-52)
    + shape("Belly", ellipse(104, 84), fill(solid(MARK)), y=-56)
    + shape("Body Shape", ellipse(184, 150), coat_fill(), y=-72),
)
root = N("Root", N("Head Pose", head, y=fit.head_y) + N("Body Pose", body) + tail_bones(rig, 66, -30, (6, 6, 6), mid=-0.3, tip=-0.3), x=200, y=370)
tail = skinned_tail(rig, 264, 5, 4, 340, (266, 272, 278), width=12)


def stuff_and_hop(f: Fit) -> Clip:
    """Stuffs its cheeks in three gulps, hops four times with cheeks full, then sits proudly."""
    c = Clip("Celebrate", 300, False)
    full = [(0, 1), (12, 1.12), (22, 1.06), (36, 1.24), (46, 1.18), (60, 1.38, "hold"), (260, 1.38), (290, 1)]
    for side in "LR":
        c.key(f"Cheek {side} Pose", "scaleX", *full).key(f"Cheek {side} Pose", "scaleY", *full)
    hops = [(0, 370, "hold"), (80, 370)]
    for i in range(4):
        t = 80 + 22 * i
        hops += [(t + 11, 334), (t + 22, 370)]
    c.key("Root", "y", *hops, (300, 370))
    proud(c, f, 170)
    # squash on each landing, over proud's chest-up hold
    squash = [(0, 1, "hold"), (80, 1)] + [kf for i in range(4) for kf in ((80 + 22 * i + 11, 1.06), (80 + 22 * i + 22, 0.9))]
    c.key("Body Pose", "scaleY", *squash, (180, 1), (195, 1.06, "hold"), (270, 1.06), (300, 1))
    return c


for side in "LR":
    rig.control(f"Cheek {side} Pose", "scaleX", 1)
    rig.control(f"Cheek {side} Pose", "scaleY", 1)
above, below = props(rig)
controls(rig, fit)
write(
    rig,
    above + root + tail + below,
    ambient=ambient(),
    moods=moods(fit),
    actions=actions(fit, stuff_and_hop),
    instance={"coatBase": "FFE3A564", "coatShade": "FFC4843F", "coatHighlight": "FFF3C98F", "outline": "FF6E4320"},
    path_out=os.path.join(os.path.dirname(__file__), "scene.rml"),
    accessories=acc_clips,
)
print("wrote scene.rml")
