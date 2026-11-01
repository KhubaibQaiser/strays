"""Helpers for writing Strays pet rigs as RML (see art/STYLE.md).

A rig script builds shapes with these helpers, declares the pose-controlled properties with their
rest values, and lists clips as keyframes. `Rig.write()` emits a complete scene.rml: the artboard,
a state machine driven by the `mood` and `action` view-model numbers, and the PetLook view model.
"""
from __future__ import annotations

import math
import os
from dataclasses import dataclass, field
from xml.sax.saxutils import quoteattr

VM = "0:500"
VM_PROPS = {"coatBase": "0:501", "coatShade": "0:502", "coatHighlight": "0:503", "outline": "0:504", "mood": "0:505", "action": "0:506"}
OUTLINE_W = 3
EASE = '<CubicEaseInterpolator x1="0.42" y1="0" x2="0.58" y2="1"/>'


def attrs(**kw) -> str:
    out = []
    for k, v in kw.items():
        if v is None:
            continue
        if isinstance(v, bool):
            v = "true" if v else "false"
        elif isinstance(v, float):
            v = f"{v:.4f}".rstrip("0").rstrip(".")
        out.append(f"{k}={quoteattr(str(v))}")
    return " ".join(out)


# ---- paint -------------------------------------------------------------------------------------

def solid(argb: str) -> str:
    return f'<SolidColor colorValue="{argb}" name="C"/>'


def bound(prop: str, fallback: str) -> str:
    """A colour bound to a PetLook property, so the app can recolour it."""
    return f'<SolidColor colorValue="{fallback}" name="C"><DataBindContext sourcePathIds="{VM}-{VM_PROPS[prop]}" propertyKey="37"/></SolidColor>'


def fill(color: str) -> str:
    return f'<Fill name="Fill">{color}</Fill>'


def stroke(color: str, width: float = OUTLINE_W, cap: str = "round", join: str = "round") -> str:
    return f'<Stroke {attrs(thickness=width, cap=cap, join=join)} name="Stroke">{color}</Stroke>'


COAT = bound("coatBase", "FFEFE1CC")
SHADE = bound("coatShade", "FFD9C4A6")
HIGH = bound("coatHighlight", "FFFFF8EE")
LINE = bound("outline", "FF7A5C46")


def coat_fill() -> str:
    return fill(COAT) + stroke(LINE)


# ---- geometry ----------------------------------------------------------------------------------

def ellipse(w: float, h: float) -> str:
    return f'<Ellipse {attrs(width=w, height=h)} originX="0.5" originY="0.5" name="Path"/>'


def rect(w: float, h: float, r: float = 0) -> str:
    return f'<Rectangle {attrs(width=w, height=h, cornerRadiusTL=r, cornerRadiusTR=r, cornerRadiusBL=r, cornerRadiusBR=r)} originX="0.5" originY="0.5" name="Path"/>'


def triangle(w: float, h: float) -> str:
    return f'<Triangle {attrs(width=w, height=h)} originX="0.5" originY="0.5" name="Path"/>'


def path(points: list[tuple[float, float]], closed: bool = False, radius: float = 0) -> str:
    verts = "".join(f'<StraightVertex {attrs(x=float(x), y=float(y), radius=radius or None)}/>' for x, y in points)
    return f'<PointsPath isClosed="{str(closed).lower()}" name="Path">{verts}</PointsPath>'


def curve(points: list[tuple[float, float]], closed: bool = False) -> str:
    """A smooth path through the points (cubic mirrored vertices, tangents from neighbours)."""
    n = len(points)
    verts = []
    for i, (x, y) in enumerate(points):
        a = points[i - 1] if (i > 0 or closed) else points[i]
        b = points[(i + 1) % n] if (i < n - 1 or closed) else points[i]
        tx, ty = (b[0] - a[0]) / 6, (b[1] - a[1]) / 6
        rot = math.atan2(ty, tx)
        dist = math.hypot(tx, ty)
        verts.append(f'<CubicMirroredVertex {attrs(x=float(x), y=float(y), rotation=rot, distance=dist)}/>')
    return f'<PointsPath isClosed="{str(closed).lower()}" name="Path">{"".join(verts)}</PointsPath>'


def shape(name: str, geom: str, paint: str, x: float = 0, y: float = 0, rotation: float | None = None, sid: str | None = None, opacity: float | None = None) -> str:
    return f'<Shape {attrs(x=float(x), y=float(y), rotation=rotation, opacity=opacity, name=name, id=sid)}>{geom}{paint}</Shape>'


# ---- the rig -----------------------------------------------------------------------------------

@dataclass
class Clip:
    name: str
    frames: int
    loop: bool
    keys: dict[tuple[str, str], list[tuple[int, float, str]]] = field(default_factory=dict)

    def key(self, obj: str, prop: str, *frames: tuple) -> "Clip":
        """frames: (frame, value) or (frame, value, ease) with ease in cubic|linear|hold."""
        self.keys[(obj, prop)] = [(f[0], f[1], f[2] if len(f) > 2 else "cubic") for f in frames]
        return self


@dataclass
class Rig:
    name: str
    ids: dict[str, str] = field(default_factory=dict)
    rest: dict[tuple[str, str], float] = field(default_factory=dict)
    _next: int = 20

    def id(self, name: str) -> str:
        """A stable id for a named object, so clips can key it."""
        if name not in self.ids:
            self.ids[name] = f"0:{self._next}"
            self._next += 1
        return self.ids[name]

    def node(self, name: str, children: str, x: float = 0, y: float = 0, **kw) -> str:
        for prop, value in kw.items():
            if prop.startswith("rest_"):
                self.rest[(name, prop[5:])] = value
        own = {k: v for k, v in kw.items() if not k.startswith("rest_")}
        return f'<Node {attrs(x=float(x), y=float(y), **own, name=name, id=self.id(name))}>{children}</Node>'

    def control(self, obj: str, prop: str, value: float) -> None:
        """Declares a pose-layer property and its rest value; every pose and clip keys it."""
        self.rest[(obj, prop)] = value


def _keyed(rig: Rig, keys: dict, frames_total: int) -> str:
    out = []
    by_obj: dict[str, list[str]] = {}
    for (obj, prop), frames in keys.items():
        kfs = []
        for i, (f, v, ease) in enumerate(frames):
            last = i == len(frames) - 1
            if last:
                kfs.append(f'<KeyFrameDouble {attrs(value=float(v), frame=f)}/>')
            elif ease == "linear":  # interpolationType defaults to hold, so linear must be explicit
                kfs.append(f'<KeyFrameDouble {attrs(value=float(v), frame=f)} interpolationType="linear"/>')
            elif ease == "hold":
                kfs.append(f'<KeyFrameDouble {attrs(value=float(v), frame=f)} interpolationType="hold"/>')
            else:
                kfs.append(f'<KeyFrameDouble {attrs(value=float(v), frame=f)} interpolationType="cubic">{EASE}</KeyFrameDouble>')
        by_obj.setdefault(obj, []).append(f'<KeyedProperty property="{prop}">{"".join(kfs)}</KeyedProperty>')
    for obj, props in by_obj.items():
        out.append(f'<KeyedObject objectId="{rig.id(obj)}">{"".join(props)}</KeyedObject>')
    return "".join(out)


def animation(rig: Rig, clip: Clip, aid: str, posed: bool) -> str:
    keys = dict(clip.keys)
    if posed:
        # every pose-layer animation keys every controlled property, so switching never leaves a stale value
        for k, v in rig.rest.items():
            keys.setdefault(k, [(0, v, "hold")])
    loop = 'loopValue="loop" ' if clip.loop else ""
    return f'<LinearAnimation {loop}duration="{clip.frames}" name="{clip.name}" id="{aid}">{_keyed(rig, keys, clip.frames)}</LinearAnimation>'


def _eq(prop: str, value: float, op: str = "equal") -> str:
    return (
        f'<TransitionViewModelCondition opValue="{op}"><TransitionPropertyViewModelComparator><BindablePropertyNumber>'
        f'<DataBindContext sourcePathIds="{VM}-{VM_PROPS[prop]}" propertyKey="636"/></BindablePropertyNumber>'
        f'</TransitionPropertyViewModelComparator><TransitionValueNumberComparator value="{value}"/></TransitionViewModelCondition>'
    )


def write(rig: Rig, artboard_children: str, ambient: list[Clip], moods: list[Clip], actions: list[Clip], instance: dict[str, str], path_out: str) -> None:
    """moods[i] plays while mood == i+1 and action == 0; actions[j] plays while action == j+1."""
    sid = 1000
    aid = 2000
    anims = []
    layers = []

    def next_ids():
        nonlocal sid, aid
        sid += 1
        aid += 1
        return f"0:{sid}", f"0:{aid}"

    # ambient loops: one layer each, running forever
    for i, clip in enumerate(ambient):
        s, a = next_ids()
        anims.append(animation(rig, clip, a, posed=False))
        layers.append(
            f'<StateMachineLayer name="{clip.name}" id="0:{900 + i}"><AnyState x="400" y="-120"/><ExitState x="600" y="-120"/>'
            f'<EntryState><StateTransition stateToId="{s}"/></EntryState><AnimationState x="160" y="0" animationId="{a}" id="{s}"/></StateMachineLayer>'
        )

    # pose layer: mood states and action states, every pair connected by data conditions
    mood_states, mood_anims, action_states = [], [], []
    for clip in moods:
        s, a = next_ids()
        anims.append(animation(rig, clip, a, posed=True))
        mood_states.append(s)
        mood_anims.append(a)
    for clip in actions:
        s, a = next_ids()
        anims.append(animation(rig, clip, a, posed=True))
        action_states.append((s, a))

    states = []
    for i, s in enumerate(mood_states):
        tr = []
        for j, t in enumerate(mood_states):
            if j != i:
                tr.append(f'<StateTransition stateToId="{t}" duration="400">{_eq("action", 0)}{_eq("mood", j + 1)}</StateTransition>')
        for j, (t, _) in enumerate(action_states):
            tr.append(f'<StateTransition stateToId="{t}" duration="150">{_eq("action", j + 1)}</StateTransition>')
        states.append(f'<AnimationState x="{160 + 200 * i}" y="0" animationId="{mood_anims[i]}" id="{s}">{"".join(tr)}</AnimationState>')
    for i, (s, a) in enumerate(action_states):
        tr = []
        for j, t in enumerate(mood_states):
            tr.append(f'<StateTransition stateToId="{t}" duration="250">{_eq("action", 0)}{_eq("mood", j + 1)}</StateTransition>')
        for j, (t, _) in enumerate(action_states):
            if j != i:
                tr.append(f'<StateTransition stateToId="{t}" duration="150">{_eq("action", j + 1)}</StateTransition>')
        states.append(f'<AnimationState x="{160 + 200 * i}" y="300" animationId="{a}" reset="true" id="{s}">{"".join(tr)}</AnimationState>')
    # entry: rest in Content until the app sets a mood
    layers.append(
        f'<StateMachineLayer name="Pose" id="0:950"><AnyState x="160" y="-200"/><ExitState x="360" y="-200"/>'
        f'<EntryState><StateTransition stateToId="{mood_states[1]}"/></EntryState>{"".join(states)}</StateMachineLayer>'
    )

    props = "".join(
        f'<ViewModelPropertyColor name="{n}" id="{VM_PROPS[n]}"/>' for n in ("coatBase", "coatShade", "coatHighlight", "outline")
    ) + f'<ViewModelPropertyNumber name="mood" id="{VM_PROPS["mood"]}"/><ViewModelPropertyNumber name="action" id="{VM_PROPS["action"]}"/>'
    inst = "".join(
        f'<ViewModelInstanceColor propertyValue="{instance[n]}" viewModelPropertyId="{VM_PROPS[n]}"/>' for n in ("coatBase", "coatShade", "coatHighlight", "outline")
    ) + f'<ViewModelInstanceNumber propertyValue="2" viewModelPropertyId="{VM_PROPS["mood"]}"/><ViewModelInstanceNumber propertyValue="0" viewModelPropertyId="{VM_PROPS["action"]}"/>'

    doc = (
        '<Rive version="1" kind="fragment">\n'
        f'<!-- Generated by art/{rig.name.lower()}/make.py; edit that, not this. -->\n'
        f'<Artboard defaultStateMachineId="0:7" styleId="0:5" viewModelId="{VM}" viewModelInstanceId="0:510" width="400" height="400" name="{rig.name}" id="0:2">\n'
        '<LayoutComponentStyle name="Artboard Style" id="0:5"/>\n'
        f'{artboard_children}\n'
        f'<StateMachine name="Idle" id="0:7">{"".join(layers)}</StateMachine>\n'
        f'{"".join(anims)}\n'
        '</Artboard>\n'
        f'<ViewModel defaultInstanceId="0:510" name="PetLook" id="{VM}">{props}<ViewModelInstance exports="true" name="Default" id="0:510">{inst}</ViewModelInstance></ViewModel>\n'
        '</Rive>\n'
    )
    with open(path_out, "w") as f:
        f.write(doc)
    # what art/tools/review.py renders: every pose and clip, with the data that selects it
    import json
    review = [{"name": c.name, "data": {"mood": i + 1, "action": 0}, "frames": c.frames} for i, c in enumerate(moods)]
    review += [{"name": c.name, "data": {"mood": 2, "action": j + 1}, "frames": c.frames} for j, c in enumerate(actions)]
    build = os.path.join(os.path.dirname(path_out), "build")
    os.makedirs(build, exist_ok=True)
    with open(os.path.join(build, "review.json"), "w") as f:
        json.dump(review, f, indent=1)
