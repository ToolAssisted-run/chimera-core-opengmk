# Building the OpenGMK core

## What you need

- Linux (Ubuntu 24.04 is what CI uses) with `meson`, `ninja-build`,
  `build-essential`, `cmake`, `pkg-config`, `python3`, `bison`, `flex`,
  `python3-mako`, `python3-packaging` (Mesa) and `ffmpeg` (the engine test reads
  its screenshots with it).
- rustup with the nightly toolchain and its `rust-src`:
  `rustup toolchain install nightly --profile minimal -c rust-src`. The guest
  needs `-Z build-std`, so a stable compiler is not enough.
- A Chimera checkout, built (`tools/ci-chimera.sh`), with `git submodule update
  --init --recursive`, and its miniBox built with the C++ guest kit:
  `meson setup build/meson-linux . && meson compile -C build/meson-linux`, and
  the same with `-Dguest_cpp=true` into `build/meson-cpp`, in
  `<chimera>/extern/chimera-common-minibox`.
- This repository cloned with `--recursive` (OpenGMK is a submodule).

## The package

```sh
./waterbox/build-package.sh -m <minibox> -r <chimera>
```

1. `setup-mesa.sh -m <minibox>` builds Mesa 24.0.9 (pinned by SHA-256) as
   softpipe behind OSMesa, once for the guest (`build/mesa/build-guest2`).
   `setup-mesa.sh -n` builds the host flavor the native reference uses. About
   fifteen minutes each.
2. `apply-patches.sh` applies `patches/` to `extern/OpenGMK`. Idempotent; stops
   on a tree that is partly patched.
3. `build-guest.sh -m <minibox>` runs `cargo +nightly build --release` in
   `waterbox/guest` (a staticlib for the guest target
   `guest/waterbox-guest.json`), then links it with Mesa, miniBox's C++ runtime
   and `guest-syscalls.cpp` into `build/wbx/core.wbx` and runs miniBox's
   `check-wbx.sh` on it. It also fails if the C library's maths reached the
   core: the maths must be the core's own (`guest/src/mathlib.rs`).
4. `build-package.sh` strips it and packs `core.wbx`, `waterbox.config`,
   `file_slots.json`, `default_keybinds.json`, `languages/`, the licences and
   the window kit into `<chimera>/build/Cores/opengmk.chimeraCore`. The package
   is packed deterministically: its SHA-1 is the core's identity.

A package built by hand is stamped `<commit>+local`; CI stamps the commit.

## The test script

`waterbox/run-gate.sh -m <minibox> -r <chimera>` builds what is missing (host
Mesa, the native reference, `run-wbx`, the test game's maker and the game) and
runs the tests. `-n` does not rebuild; `-b` breaks each test on purpose, once,
and a test that still passes is the failure. Logs are in `build/gate/`.

Content: the test game, made by `waterbox/tests/mkgame` from this repository's
own code (a bouncing ball, text, shapes, a tone, a save file; it uses the
keyboard, the mouse, the date, `random()` and files). Nothing else is needed.

| Test | What it compares or checks |
|---|---|
| native == sandbox | clock, picture and sound hashes every 30 steps of 300, on a route with keys and the mouse |
| native twice | the same route lands the same |
| the host stalling | 700 ms stalls of the host change nothing |
| a savestate every step | a state saved and loaded before every step changes nothing |
| a fresh host | the state moved to a new process at step 150 lands the same |
| the keys and the pointer | the picture after the route differs from the untouched game's |
| another random seed | the `randomSeed` setting changes the game |
| the game's date | `startTime` is the date, and it runs with the steps |
| the tone | the sound is the same in both, then silence |
| save data | the files written are exported, and a game started from them carries on |
| a file that is not a game | it is refused, with its reason, in both |
| a function OpenGMK lacks | a game that calls `show_message` stops with "This game called show_message, which OpenGMK does not implement yet.", in both and through chimera-run |
| a sound DLL's stand-in | a game that calls `gmfmodsimple.dll` runs, the same in both and through a savestate before every step, and its answer shows in the picture |
| a DLL that cannot run | a game that calls `mystery.dll` stops (at its tenth step) or does not start (in its first event), with a sentence naming the DLL and the function, in both and through chimera-run, and no panic text |
| the engine | the package, through chimera-run, plays the route; its picture is `run-wbx`'s |

Then the frontend test, which needs the package in the Chimera checkout, the
native reference and the test game (the test script built them):
`waterbox/tests/run-frontend.sh --chimera-root <chimera>` plays the same route
in a project through chimera-run and through Chimera's window run `--headless`
(no display), each last picture the native reference's, and checks that the
package's key bindings become Chimera's defaults.

Then Chimera's contract tests on the package:
`<chimera>/build/meson-linux/source/ui/test-packages <folder holding the package>`.

## Trying a real game

Only the test game has been run so far; no real GameMaker 8 game has.

```sh
build/wbx/run-wbx build/wbx/core.wbx /path/to/game.exe --frames 600 --digest-every 100 --screenshot 599=/tmp/last.tga
build/run-native/release/run-native /path/to/game.exe --frames 600 --digest-every 100
```

The two print the same lines when the game plays the same in both. In Chimera,
create a project with the `.exe` in the Game slot.

## Troubleshooting

- `the series does not apply` or `partly patched`: `extern/OpenGMK` was moved or
  edited; see AGENTS.md.
- `miniBox C++ guest kit missing`: build `meson-cpp` in miniBox first.
- A step that fails in a fraction of a second with no output failed on stdout:
  read `build/gate/build-*.log`.
