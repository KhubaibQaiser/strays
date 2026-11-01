"""Renders review sheets for a rig (spec §10.6): 12 frames of every pose and clip, tiled 6 x 2.

Usage: python3 art/tools/review.py art/cat   →  art/review/<species>/<clip>.png
Needs the rive CLI and ffmpeg.
"""
import json
import os
import shutil
import subprocess
import sys

rig_dir = os.path.abspath(sys.argv[1])
species = os.path.basename(rig_dir)
only = set(sys.argv[2:])
out_dir = os.path.join(rig_dir, "..", "review", species)
tmp = os.path.join(rig_dir, "build", "review-frames")
os.makedirs(out_dir, exist_ok=True)

for clip in json.load(open(os.path.join(rig_dir, "build", "review.json"))):
    if only and clip["name"] not in only:
        continue
    shutil.rmtree(tmp, ignore_errors=True)
    os.makedirs(tmp)
    total = clip["frames"] + 15  # include the blend in
    for i in range(12):
        frame = max(1, round(total * i / 11))
        data = [f"--data={k}={v}" for k, v in clip["data"].items()]
        # --data must come before --advance, or it's applied at the capture instead of at the start
        subprocess.run(["rive", ".", f"--screenshot={tmp}/f{i:02d}.png", *data, f"--advance={frame}", "--scale=0.5"],
                       cwd=rig_dir, check=True, capture_output=True)
    name = clip["name"].lower().replace(" ", "-")
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", f"{tmp}/f%02d.png", "-filter_complex", "tile=6x2:padding=4:color=white",
                    "-frames:v", "1", os.path.join(out_dir, f"{name}.png")], check=True)
    print("sheet", name)
