#!/usr/bin/env python3
"""The one source of truth for the core's input wire order.

Emits guest/src/input_table.rs (the GameMaker key each button presses) and
prints the waterbox.config "input" section (what the frontend shows), so the
two cannot drift. Run it after editing the table.

Buttons are levels, as in every Chimera core: a key held across frames is a
key held. The guest turns level changes into the key presses and releases the
game sees at the start of each step. A GameMaker 8 game reads a keyboard and a
mouse: keys by Windows virtual-key code, the mouse's three buttons and its
place in the window.

The layout (the rows, the mnemonics, the axes) is the Ruffle core's: both
cores are played with the keyboard and mouse a desktop already has.
"""
import json, sys, os

# (name shown to the user, kind, code): "mouse" is GameMaker's mouse button
# number (1 left, 2 right, 3 middle); "vk" is a Windows virtual-key code.
T = [("Mouse Left Button", "mouse", 1), ("Mouse Middle Button", "mouse", 3),
     ("Mouse Right Button", "mouse", 2)]
for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ":
    T.append((f"Key {c}", "vk", ord(c)))
for d in "1234567890":
    T.append((f"Key {d}", "vk", ord(d)))
T.append(("Key Space", "vk", 0x20))
for name, vk in [("Enter", 0x0D), ("Escape", 0x1B), ("Tab", 0x09), ("Backspace", 0x08),
                 ("Delete", 0x2E), ("Insert", 0x2D), ("Home", 0x24), ("End", 0x23),
                 ("Page Up", 0x21), ("Page Down", 0x22), ("Up", 0x26), ("Down", 0x28),
                 ("Left", 0x25), ("Right", 0x27), ("Left Shift", 0xA0), ("Right Shift", 0xA1),
                 ("Left Control", 0xA2), ("Right Control", 0xA3), ("Left Alt", 0xA4),
                 ("Right Alt", 0xA5), ("Caps Lock", 0x14), ("Num Lock", 0x90),
                 ("Scroll Lock", 0x91), ("Pause", 0x13)]:
    T.append((f"Key {name}", "vk", vk))
for i in range(1, 13):
    T.append((f"Key F{i}", "vk", 0x6F + i))
for name, vk in [("Minus", 0xBD), ("Equals", 0xBB), ("Left Bracket", 0xDB),
                 ("Right Bracket", 0xDD), ("Backslash", 0xDC), ("Semicolon", 0xBA),
                 ("Quote", 0xDE), ("Comma", 0xBC), ("Period", 0xBE), ("Slash", 0xBF),
                 ("Backquote", 0xC0)]:
    T.append((f"Key {name}", "vk", vk))
for d in range(10):
    T.append((f"Numpad {d}", "vk", 0x60 + d))
for name, vk in [("Add", 0x6B), ("Subtract", 0x6D), ("Multiply", 0x6A), ("Divide", 0x6F),
                 ("Decimal", 0x6E), ("Enter", 0x0D)]:
    T.append((f"Numpad {name}", "vk", vk))

ROWS = [
    ["Escape"] + [f"F{i}" for i in range(1, 13)] + ["Scroll Lock", "Pause"],
    ["Backquote"] + list("1234567890") + ["Minus", "Equals", "Backspace"],
    ["Tab"] + list("QWERTYUIOP") + ["Left Bracket", "Right Bracket", "Backslash"],
    ["Caps Lock"] + list("ASDFGHJKL") + ["Semicolon", "Quote", "Enter"],
    ["Left Shift"] + list("ZXCVBNM") + ["Comma", "Period", "Slash", "Right Shift"],
    ["Left Control", "Left Alt", "Space", "Right Alt", "Right Control"],
    ["Insert", "Home", "Page Up", "Delete", "End", "Page Down"],
    ["Up", "Left", "Down", "Right"],
    ["Num Lock"],
]
PAD = ["Divide", "Multiply", "Subtract", "7", "8", "9", "Add",
       "4", "5", "6", "1", "2", "3", "Enter", "0", "Decimal"]
LISTED = [n for n, kind, _ in T if kind == "mouse"] + \
    ["Key " + k for row in ROWS for k in row] + ["Numpad " + k for k in PAD]
assert sorted(LISTED) == sorted(n for n, _, _ in T), "the rows are not the table's buttons"

AXES = [{"name": "Mouse X", "min": 0, "max": 8191, "neutral": 4096},
        {"name": "Mouse Y", "min": 0, "max": 8191, "neutral": 4096}]

# ---- what the controls and the system are called ----
# The frontend keeps no table of these: a core says what its own are called.
# MNEMONICS is the letter each button writes into a movie's text and heads its
# input column with, by the button's name - whole, or without its player ("P2
# Up" is found under "Up"), so one line serves every pad. AXIS_HEADERS is the
# short header of each axis's column. (An entry is read by position: a letter
# may change and no movie made before it is harmed.)
MNEMONICS = {
    "Mouse Left Button": "B", "Mouse Middle Button": "B", "Mouse Right Button": "B", "Key A": "A",
    "Key B": "B", "Key C": "C", "Key D": "D", "Key E": "E", "Key F": "F", "Key G": "G",
    "Key H": "H", "Key I": "I", "Key J": "J", "Key K": "K", "Key L": "l", "Key M": "M",
    "Key N": "N", "Key O": "O", "Key P": "P", "Key Q": "Q", "Key R": "r", "Key S": "S",
    "Key T": "T", "Key U": "U", "Key V": "V", "Key W": "W", "Key X": "X", "Key Y": "Y",
    "Key Z": "Z", "Key 1": "1", "Key 2": "2", "Key 3": "3", "Key 4": "4", "Key 5": "5",
    "Key 6": "6", "Key 7": "7", "Key 8": "8", "Key 9": "9", "Key 0": "0", "Key Space": "S",
    "Key Enter": "E", "Key Escape": "E", "Key Tab": "T", "Key Backspace": "B", "Key Delete": "D",
    "Key Insert": "I", "Key Home": "H", "Key End": "E", "Key Page Up": "U", "Key Page Down": "D",
    "Key Up": "U", "Key Down": "D", "Key Left": "L", "Key Right": "R", "Key Left Shift": "S",
    "Key Right Shift": "S", "Key Left Control": "C", "Key Right Control": "C", "Key Left Alt": "A",
    "Key Right Alt": "A", "Key Caps Lock": "L", "Key Num Lock": "L", "Key Scroll Lock": "L",
    "Key Pause": "p", "Key F1": "1", "Key F2": "2", "Key F3": "3", "Key F4": "4", "Key F5": "5",
    "Key F6": "6", "Key F7": "7", "Key F8": "8", "Key F9": "9", "Key F10": "0", "Key F11": "F",
    "Key F12": "F", "Key Minus": "M", "Key Equals": "E", "Key Left Bracket": "B",
    "Key Right Bracket": "B", "Key Backslash": "B", "Key Semicolon": "S", "Key Quote": "Q",
    "Key Comma": "C", "Key Period": "P", "Key Slash": "S", "Key Backquote": "B", "Numpad 0": "0",
    "Numpad 1": "1", "Numpad 2": "2", "Numpad 3": "3", "Numpad 4": "4", "Numpad 5": "5",
    "Numpad 6": "6", "Numpad 7": "7", "Numpad 8": "8", "Numpad 9": "9", "Numpad Add": "A",
    "Numpad Subtract": "S", "Numpad Multiply": "M", "Numpad Divide": "D", "Numpad Decimal": "D",
    "Numpad Enter": "E",
}
AXIS_HEADERS = {
    "Mouse X": "mX", "Mouse Y": "mY",
}



def _bare(name):
    """A control's name without its player: "P2 Up" -> "Up"."""
    head, _, rest = name.partition(" ")
    return rest if rest and head[:1] == "P" and head[1:].isdigit() else name


def mnemonics_for(buttons):
    """The "mnemonics" of an input declaration: a letter for every one of its
    buttons, and for nothing else. A button nobody gave a letter stops the
    build - the engine would give it its rule's guess, and two columns of one
    pad would share a letter with nobody having decided it."""
    out = {}
    for b in buttons:
        key = b if b in MNEMONICS else _bare(b)
        if key not in MNEMONICS:
            raise SystemExit("no mnemonic for the button %r (MNEMONICS in %s)" % (b, __file__))
        out[key] = MNEMONICS[key]
    return out


def with_headers(axes):
    """The axes with their column headers; an axis nobody named stops the build."""
    missing = [a["name"] for a in axes if a["name"] not in AXIS_HEADERS]
    if missing:
        raise SystemExit("no header for the axes %s (AXIS_HEADERS in %s)" % (missing, __file__))
    return [dict(a, header=AXIS_HEADERS[a["name"]]) for a in axes]



def emit_rs(path):
    names = [n for n, _, _ in T]
    lines = ["// GENERATED by waterbox/input-table.py - do not edit; edit the table there.",
             "// One entry per button, in waterbox.config's wire order.",
             "",
             "#[derive(Clone, Copy)]",
             "pub enum Btn {",
             "    /// GameMaker's mouse button number",
             "    Mouse(i8),",
             "    /// a Windows virtual-key code",
             "    Vk(u8),",
             "}",
             "",
             f"pub const BUTTON_COUNT: usize = {len(T)};",
             "",
             "pub static BUTTONS: [Btn; BUTTON_COUNT] = ["]
    by = {n: (k, c) for n, k, c in T}
    for n in LISTED:
        k, c = by[n]
        lines.append(("    Btn::Mouse(%d)," if k == "mouse" else "    Btn::Vk(0x%02X),") % c + f" // {n}")
    lines.append("];")
    open(path, "w").write("\n".join(lines) + "\n")

if __name__ == "__main__":
    here = os.path.dirname(os.path.abspath(__file__))
    emit_rs(os.path.join(here, "guest", "src", "input_table.rs"))
    inp = {"name": "Keyboard and Mouse",
           "_comment": "Mouse Left/Middle/Right, then the keyboard row by row as it sits on a desk. The two axes are the pointer across the game's window. Buttons are levels; the core turns level changes into the key and mouse presses the game reads at the start of each step. The wire order is generated by waterbox/input-table.py.",
           "buttons": LISTED, "mnemonics": mnemonics_for(LISTED),
           "axes": with_headers(AXES),
           "_comment_axes": "The pointer is normalised across this range and scaled to the game's window inside the core, because a game decides its window size and this file cannot know it. Bound window-relative (WMouse), since the game's window is the picture."}
    if "--index" in sys.argv:
        # the number of a button in the wire order: input-table.py --index "Key Space"
        print(LISTED.index(sys.argv[sys.argv.index("--index") + 1]))
    elif "--json" in sys.argv:
        print(json.dumps(inp, indent=2))
    else:
        print(f"{len(T)} buttons, {len(AXES)} axes -> guest/src/input_table.rs")
