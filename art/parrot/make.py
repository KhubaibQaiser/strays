"""Budgerigar rig: writes scene.rml. Run from anywhere: python3 art/parrot/make.py

Shared poses, clips and props come from art/tools/pet.py. This file draws the budgie (face mask,
barred crown, cheek patches, hooked beak, wings, long tail) on its own perch, and swaps the walking
clips for flights (spec §10.2: the parrot adds fly + perch):
- enter and postcard fly in and land on the perch; exit takes off; walk is a flutter in place.
- the wings use the shared ear poses, so mood poses lift or droop them.
- it sleeps fluffed up on the perch, so the bed never shows.
Celebration (spec §10.3): a loop flight, then a feather fluff.
"""
import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "tools"))
from pet import accessories, PUPIL, Fit, actions, ambient, controls, face, moods, proud, props  # noqa: E402
from rml import Clip, Rig, coat_fill, curve, ellipse, fill, path, rect, shape, solid, stroke, write, LINE, SHADE  # noqa: E402

rig = Rig("Parrot")
N = rig.node
# wings stand in for ears: rest folded (0); negative lifts them outwards, positive tucks them in
fit = Fit(crown=-55, chin=56, head_w=116, chest=(28, -92), head_y=-150, lie_body=0.88, lie_head=-138, eye_dx=26, ear=0.0, ear_up=-0.35, ear_soft=-0.15, ear_low=0.08, ear_flat=0.14,
          paw_y=-4, mouth_y=22, eat_dx=-50, eat_dy=60)
MASK, BEAK, CERE, CHEEK, PERCH = "FFF7E27A", "FFE8C88A", "FF6C8FD6", "FF4A6FD8", "FF9A6A3E"


def wing(side: int) -> str:
    name = "L" if side < 0 else "R"
    bars = "".join(shape(f"Bar {name}{i}", path([(-14, 10 + 22 * i), (0, 18 + 22 * i), (14, 10 + 22 * i)], radius=8), stroke(LINE, 2)) for i in range(4))
    feathers = shape(f"Wing {name} Shape", ellipse(48, 112), fill(SHADE) + stroke(LINE), y=46)
    return N(f"Ear {name} Pose", N(f"Ear {name}", bars + feathers), x=46 * side, y=-138)


def tail() -> str:
    """Three rigid feather segments, one per bone, hanging down behind the body."""
    segs = [("Tail Base", 30, 22, 0.0), ("Tail Mid", 26, 18, 0.05), ("Tail Tip", 22, 12, 0.05)]
    out = ""
    for name, length, width, rot in reversed(segs):
        feather = shape(f"{name} Feather", rect(length + 8, width, width / 2), fill(SHADE) + stroke(LINE), x=length / 2)
        tag = "RootBone" if name == "Tail Base" else "Bone"
        pos = ' x="0" y="0"' if tag == "RootBone" else ""
        out = f'<{tag}{pos} length="{length}" rotation="{rot}" name="{name}" id="{rig.id(name)}">{feather}{out}</{tag}>'
    return N("Tail Mount", out, y=-46, rotation=math.pi / 2 - 0.45)


acc_head, acc_body, acc_clips = accessories(rig, fit)
head = N(
    "Head",
    acc_head
    + shape("Beak", curve([(0, 26), (-9, 12), (-8, 2), (8, 2), (9, 12)], closed=True), fill(solid(BEAK)) + stroke(LINE, 2.5), y=4)
    + shape("Cere", ellipse(22, 9), fill(solid(CERE)) + stroke(LINE, 2), y=4)
    + face(rig, fit)
    + "".join(shape(f"Spot {i}", ellipse(7, 7), fill(solid(PUPIL)), x=x, y=50) for i, x in enumerate((-22, 0, 22)))
    + shape("Cheek L", ellipse(14, 18), fill(solid(CHEEK)), x=-38, y=30)
    + shape("Cheek R", ellipse(14, 18), fill(solid(CHEEK)), x=38, y=30)
    + shape("Mask", ellipse(98, 88), fill(solid(MASK)), y=12)
    + "".join(shape(f"Crown Bar {i}", path([(-30 + 4 * i, 0), (0, -6), (30 - 4 * i, 0)], radius=14), stroke(solid(PUPIL), 2.5), y=-30 - 8 * i) for i in range(3))
    + shape("Head Shape", ellipse(116, 110), coat_fill()),
)
body = N(
    "Body",
    acc_body
    + shape("Paw L", ellipse(18, 12), fill(solid("FFC9A9A0")) + stroke(LINE, 2), x=-18, y=fit.paw_y, sid=rig.id("Paw L"))
    + shape("Paw R", ellipse(18, 12), fill(solid("FFC9A9A0")) + stroke(LINE, 2), x=18, y=fit.paw_y, sid=rig.id("Paw R"))
    + wing(-1) + wing(1)
    + shape("Body Shape", curve([(0, -150), (52, -112), (56, -50), (30, -12), (-30, -12), (-56, -50), (-52, -112)], closed=True), coat_fill()),
)
root = N("Root", N("Head Pose", head, y=fit.head_y) + N("Body Pose", body) + tail(), x=200, y=370)
perch = N("Perch", shape("Perch Bar", rect(190, 14, 7), fill(solid(PERCH)) + stroke(LINE, 2.5)), x=200, y=376)


def flap(c: Clip, start: int, end: int, period: int = 10) -> Clip:
    """Wings beat from `start` to `end`, then fold."""
    fr = [(0, 0, "hold"), (start, 0)]
    for t in range(start, end - period // 2, period):
        fr += [(t + period // 2, -1.5), (t + period, -0.5)]
    fr += [(end + 12, 0, "hold"), (c.frames, 0)]
    c.key("Ear L Pose", "rotation", *[(t[0], -t[1], *t[2:]) for t in fr])
    return c.key("Ear R Pose", "rotation", *fr)


def tuck(c: Clip, start: int, end: int) -> Clip:
    """Feet tucked up while airborne."""
    for p in ("Paw L", "Paw R"):
        c.key(p, "y", (0, fit.paw_y, "hold"), (start, fit.paw_y), (start + 8, fit.paw_y - 12, "hold"), (end, fit.paw_y - 12), (end + 8, fit.paw_y, "hold"), (c.frames, fit.paw_y))
    return c


def land(c: Clip, at: int) -> Clip:
    """Touch down on the perch at frame `at` with a little squash."""
    return c.key("Body Pose", "scaleY", (0, 1, "hold"), (at - 4, 1), (at + 4, 0.9), (at + 16, 1.03), (at + 26, 1, "hold"), (c.frames, 1))


def fly_in(name: str, frames: int, at: int) -> Clip:
    c = Clip(name, frames, False)
    c.key("Root", "x", (0, 560), (at, 200, "hold"), (frames, 200))
    c.key("Root", "y", (0, 210), (at - 30, 300), (at, 370, "hold"), (frames, 370))
    return land(tuck(flap(c, 0, at), 0, at - 6), at)


def fly_out(frames: int) -> Clip:
    c = Clip("Exit", frames, False)
    c.key("Body Pose", "scaleY", (0, 1), (14, 0.88), (24, 1.04, "hold"), (frames, 1.04))
    c.key("Root", "x", (0, 200, "hold"), (20, 200), (frames - 20, 560, "hold"), (frames, 560))
    c.key("Root", "y", (0, 370, "hold"), (20, 370), (70, 300), (frames - 20, 200, "hold"), (frames, 200))
    return tuck(flap(c, 18, frames), 24, frames)


def flutter() -> Clip:
    """Walk, in place: two short fluttering hops."""
    c = Clip("Walk", 120, True)
    c.key("Root", "y", (0, 370), (20, 346), (40, 370), (60, 370), (80, 346), (100, 370, "hold"), (120, 370))
    c.key("Ear R Pose", "rotation", *[(t, -1.2 if i % 2 else 0) for i, t in enumerate(range(0, 101, 8))], (120, 0))
    c.key("Ear L Pose", "rotation", *[(t, 1.2 if i % 2 else 0) for i, t in enumerate(range(0, 101, 8))], (120, 0))
    return c


def loop_and_fluff(f: Fit) -> Clip:
    """Takes off, flies a full loop and lands, fluffs its feathers twice, then sits proudly."""
    c = Clip("Celebrate", 330, False)
    cx, cy, r, t0, t1 = 200, 250, 120, 20, 140
    pts = [(t, t0 + (t1 - t0) * t / 24) for t in range(25)]
    th = [math.pi / 2 - 2 * math.pi * i / 24 for i, _ in pts]
    c.key("Root", "x", (0, 200, "hold"), *[(int(fr), cx + r * math.cos(a), "linear") for (_, fr), a in zip(pts, th)], (330, 200))
    c.key("Root", "y", (0, 370, "hold"), *[(int(fr), cy + r * math.sin(a), "linear") for (_, fr), a in zip(pts, th)], (330, 370))
    c.key("Root", "rotation", (0, 0, "hold"), (t0, 0, "linear"), (t1, -2 * math.pi, "hold"), (t1 + 1, 0, "hold"), (330, 0))
    flap(tuck(c, t0, t1), t0 - 6, t1)
    proud(c, f, 240)
    puff = [(0, 1, "hold"), (t1 - 4, 1), (t1 + 4, 0.9), (t1 + 16, 1)]
    for a in (170, 205):
        puff += [(a, 1.18), (a + 6, 1.1), (a + 12, 1.18), (a + 24, 1)]
    c.key("Body Pose", "scaleX", *puff, (330, 1))
    c.key("Body Pose", "scaleY", *puff, (265, 1.06, "hold"), (300, 1.06), (330, 1))
    c.key("Head Pose", "rotation", (0, 0, "hold"), (165, 0), (172, 0.12), (179, -0.12), (186, 0.08), (194, 0, "hold"), (330, 0))
    return c


rig.control("Root", "rotation", 0)
above, below = props(rig)
controls(rig, fit)
acts = actions(fit, loop_and_fluff)
acts[3], acts[4] = fly_out(180), fly_in("Enter", 180, 140)
acts[8] = fly_in("Postcard", 240, 150)
acts[8].key("Postcard", "opacity", (0, 1, "hold"), (200, 1), (230, 0, "hold"), (240, 0))
acts[8].key("Mouth", "opacity", (0, 0, "hold"), (230, 1, "hold"), (240, 1))
acts[9] = flutter()
for clip in acts:  # it sleeps on its perch
    clip.keys.pop(("Bed", "opacity"), None)
amb = ambient()
amb[2] = Clip("Tail Sway", 420, True).key("Tail Mid", "rotation", (0, 0.05), (210, -0.08), (420, 0.05)).key("Tail Tip", "rotation", (0, 0.05), (250, 0.14), (420, 0.05))
write(
    rig,
    above + root + perch + below,
    ambient=amb,
    moods=moods(fit),
    actions=acts,
    instance={"coatBase": "FF7DC44E", "coatShade": "FF5EA233", "coatHighlight": "FFA6DE7B", "outline": "FF2F5A1C"},
    path_out=os.path.join(os.path.dirname(__file__), "scene.rml"),
    accessories=acc_clips,
)
print("wrote scene.rml")
