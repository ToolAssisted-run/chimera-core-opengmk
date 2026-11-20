#!/usr/bin/env python3
"""The test game's route, in the two forms the tests use.

    make-route.py route            the route file make-project.py takes: one line
                                   a step, the controls held (names without
                                   spaces) and the pointer's place (X=, Y=,
                                   0..8191, which stays until given again)
    make-route.py flags            the same route as run-native's options

The route is the one run-gate.sh plays: the space bar tapped twice, the right
arrow held, the pointer moved and its button held.
"""
import json
import os
import sys

STEPS = 300
# (first step, last step + 1, control)
HOLDS = [(10, 13, "Key Space"), (30, 80, "Key Right"), (50, 120, "Mouse Left Button"), (140, 141, "Key Space")]
# (step, x, y)
POINTER = [(50, 2000, 3000), (90, 6000, 1000)]


def route():
    out = []
    for f in range(STEPS):
        toks = [c.replace(" ", "") for a, b, c in HOLDS if a <= f < b]
        toks += ["X=%d Y=%d" % (x, y) for s, x, y in POINTER if s == f]
        out.append(" ".join(toks))
    return "\n".join(out) + "\n"


def flags():
    here = os.path.dirname(os.path.abspath(__file__))
    buttons = json.load(open(os.path.join(here, "..", "waterbox.config")))["input"]["buttons"]
    out = []
    for a, b, c in HOLDS:
        out += ["--press", "%d:%d" % (a, buttons.index(c)), "--release", "%d:%d" % (b, buttons.index(c))]
    for s, x, y in POINTER:
        out += ["--mouse", "%d:%d:%d" % (s, x, y)]
    return " ".join(out)


if __name__ == "__main__":
    what = sys.argv[1] if len(sys.argv) > 1 else ""
    if what == "route":
        sys.stdout.write(route())
    elif what == "flags":
        print(flags())
    else:
        sys.exit(__doc__)
