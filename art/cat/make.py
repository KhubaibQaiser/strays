"""Persian cat rig: writes scene.rml. Run from anywhere: python3 art/cat/make.py

Shared poses, clips and props come from art/tools/pet.py; this file draws the cat and picks its
celebration (zoomies, spec §10.3).
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "tools"))
from pet import accessories, BLUSH, INNER_EAR, NOSE, Fit, actions, ambient, controls, face, moods, props, skinned_tail, tail_bones, zoomies  # noqa: E402
from rml import HIGH, Rig, coat_fill, ellipse, fill, shape, solid, triangle, write  # noqa: E402

rig = Rig("Cat")
N = rig.node
fit = Fit()


def ear(side: int) -> str:
    name = "L" if side < 0 else "R"
    inner = shape("Inner", triangle(18, 18), fill(solid(INNER_EAR)), y=-6)
    outer = shape("Outer", triangle(36, 34), coat_fill(), y=-8)
    return N(f"Ear {name} Pose", N(f"Ear {name}", inner + outer), x=40 * side, y=-40, rotation=fit.ear * side)


acc_head, acc_body, acc_clips = accessories(rig, fit)
head = N(
    "Head",
    acc_head
    + face(rig, fit)
    + shape("Nose", triangle(14, 10), fill(solid(NOSE)), y=22, rotation=3.1415927)
    + shape("Blush L", ellipse(18, 9), fill(solid(BLUSH)), x=-44, y=24)
    + shape("Blush R", ellipse(18, 9), fill(solid(BLUSH)), x=44, y=24)
    + shape("Head Shape", ellipse(148, 118), coat_fill())
    + shape("Cheek L", ellipse(58, 44), coat_fill(), x=-44, y=26)
    + shape("Cheek R", ellipse(58, 44), coat_fill(), x=44, y=26)
    + ear(-1) + ear(1),
)
body = N(
    "Body",
    acc_body
    + shape("Paw L", ellipse(44, 26), coat_fill(), x=-30, y=fit.paw_y, sid=rig.id("Paw L"))
    + shape("Paw R", ellipse(44, 26), coat_fill(), x=30, y=fit.paw_y, sid=rig.id("Paw R"))
    + shape("Chest Fluff", ellipse(86, 76), fill(HIGH), y=-82)
    + shape("Body Shape", ellipse(156, 144), coat_fill(), y=-70),
)
root = N("Root", N("Head Pose", head, y=fit.head_y) + N("Body Pose", body) + tail_bones(rig, 45, -18, (37, 36, 32)), x=200, y=370)
tail = skinned_tail(rig, 238, 6, 20, 352, (245, 282, 318))

above, below = props(rig)
controls(rig, fit)
write(
    rig,
    above + root + tail + below,
    ambient=ambient(),
    moods=moods(fit),
    actions=actions(fit, zoomies),
    instance={"coatBase": "FFEFE1CC", "coatShade": "FFD9C4A6", "coatHighlight": "FFFFF8EE", "outline": "FF7A5C46"},
    path_out=os.path.join(os.path.dirname(__file__), "scene.rml"),
    accessories=acc_clips,
)
print("wrote scene.rml")
