"""Cuts and levels the shipped sounds from the downloaded Freesound previews (all CC0; see CREDITS.md).
Usage: python3 assets/audio/make.py <folder with the downloaded previews>. Needs ffmpeg.

Each sound: trim to the clearest moment, high-pass away rumble, short fades, peak-normalise to -3 dBFS,
mono MP3. The previews themselves aren't committed.
"""
import os
import subprocess
import sys

SRC = sys.argv[1]
HERE = os.path.dirname(os.path.abspath(__file__))
# name: (preview file, start s, end s, extra filters)
CUTS = {
    "cat-greeting": ("cat-greeting--412017.mp3", 0.25, 1.40, ""),
    "cat-happy": ("cat-happy--585775.mp3", 3.20, 5.20, ""),
    "cat-sad": ("cat-sad--732521.mp3", 0.00, 1.40, ""),
    "dog-greeting": ("dog-greeting--630648.mp3", 0.68, 1.20, ""),
    "dog-happy": ("dog-happy--452180.mp3", 0.72, 1.60, ""),
    "dog-sad": ("dog-sad--118970.mp3", 0.00, 1.25, ""),
    "hamster-greeting": ("hamster-greeting--815243.mp3", 2.10, 2.65, ""),
    "hamster-happy": ("hamster-happy--204762.mp3", 28.90, 30.10, ""),
    "hamster-sad": ("hamster-sad--165906.mp3", 9.00, 10.00, ""),
    "parrot-greeting": ("parrot-greeting--620381.mp3", 0.45, 1.40, ""),
    "parrot-happy": ("parrot-happy--436497.mp3", 1.00, 3.00, ""),
    # no sad budgie recording exists: one soft chirp, slowed and lowered a little
    "parrot-sad": ("parrot-happy--436498.mp3", 1.28, 1.62, "asetrate=44100*0.8,aresample=44100,"),
    "celebrate": ("celebrate--626259.mp3", 0.00, 3.75, ""),
    "wake": ("wake--846674.mp3", 0.28, 2.20, "agate=threshold=0.003:ratio=4,"),
    "tick": ("tick--876010.mp3", 0.00, 0.30, ""),
}


def peak(path: str, filters: str) -> float:
    out = subprocess.run(["ffmpeg", "-i", path, "-af", filters + "volumedetect", "-f", "null", "-"], capture_output=True, text=True).stderr
    return float(out.split("max_volume:")[1].split("dB")[0])


for name, (src, a, b, extra) in CUTS.items():
    d = b - a
    base = f"atrim={a}:{b},asetpts=PTS-STARTPTS,{extra}highpass=f=90,afade=t=in:d=0.015,afade=t=out:st={max(0, d - 0.08):.3f}:d=0.08,"
    gain = -3 - peak(os.path.join(SRC, src), base)
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", os.path.join(SRC, src), "-af", base + f"volume={gain:.2f}dB", "-ac", "1", "-ar", "44100",
                    "-codec:a", "libmp3lame", "-b:a", "128k", os.path.join(HERE, f"{name}.mp3")], check=True)
    print(f"{name}: {d:.2f}s, +{gain:.1f} dB")

# the sad loop (played while a pet is gloomy): the whole seamless loop, no trim or fades (they'd click at
# the seam), stereo, peak-normalised to -6 dBFS so it sits under the pet sounds
LOOPS = {"sad-loop": "sad-loop--580833.mp3"}
for name, src in LOOPS.items():
    gain = -6 - peak(os.path.join(SRC, src), "")
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", os.path.join(SRC, src), "-af", f"volume={gain:.2f}dB", "-ar", "44100",
                    "-codec:a", "libmp3lame", "-b:a", "112k", os.path.join(HERE, f"{name}.mp3")], check=True)
    print(f"{name}: loop, +{gain:.1f} dB")
