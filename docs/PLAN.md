# Chimera core: OpenGMK (GameMaker 8 games)

Requested in Chimera issue 604 (a runner for GameMaker 8 games and one for
GameMaker Studio games). OpenGMK is the first; the second, OpenGM, is C# and
needs a .NET runtime the sandbox does not have, so it is not part of this.
Decided by the owner on 2026-11-17: OpenGMK becomes a game core, built only
(the player brings their own GameMaker 8 `.exe`; CI proves the build and the
package's declarations, and runs on a game made here).

## Is it feasible: the answers (2026-11-20)

- **May it be distributed, modified?** OpenGMK is GPL-2.0-only
  (`LICENCE.md`, SPDX `GPL-2.0-only`). A core is OpenGMK's source with patches,
  published as a binary package with its licence texts and a link to the
  source it was built from, which `package-licenses.json` and
  `package-licenses.py` do for every core. The Rust dependencies are MIT, BSD,
  Zlib, Unlicense or CC0 (each under that option where it offers more); none
  is Apache-2.0 only, which GPL-2.0-only cannot take. `gl_generator`, which is,
  runs when OpenGMK is built and is not in the package. udon (OpenGMK's sound
  library) is GPL-2.0-only too. The package may ship: the core, Mesa (MIT),
  OpenGMK's embedded data (the font GameMaker games draw text with, and the
  particle shapes), the licence texts. It ships no game.
  One point is open: OpenGMK's repository does not restate the licence of
  `data/arimo.dat` (a GameMaker 8 export of the Arimo font); Arimo is
  published under Apache-2.0 by Google Fonts. It is data, not linked code, and
  named in `package-licenses.json`; the owner may want to confirm it.
- **Can it be one thread on one clock?** Yes, with four changes. (1) Its
  window and its clock: OpenGMK runs a frame loop on the host's time and
  sleeps between frames; the core calls one step (`chimera_step`, patch 6),
  the game's date is a setting plus the steps run (OpenGMK already had a
  spoofed clock for replays), and the sleep is removed. (2) Randomness:
  `randomize()` drew from the host; it draws from the core's own sequence
  (a setting). (3) Threads: sound mixing and game-data reading used threads;
  the mixer is pulled a step at a time (OpenGMK already had that mode, for
  recording videos) and the data is read on one thread. (4) The host's time
  zone: the date is UTC.
- **Where does the picture come from?** An OpenGL renderer drawn by Mesa's
  softpipe compiled into the core (the second way in porting-a-core.md).
  OpenGMK needs OpenGL 3.3; softpipe behind OSMesa gives a 3.3 compatibility
  context, which its shaders accept. The picture is read from OSMesa's buffer,
  so it is the same on every host. The GPU bridge is not used (and would put a
  game's textures outside the savestate).
- **Does it recompile?** No. GML is compiled by OpenGMK to its own
  instructions and interpreted.
- **What must the user bring?** The game: a GameMaker 8.0 or 8.1 `.exe`. A
  game that calls a Windows DLL (`external_call`) cannot (there is no Windows
  in the sandbox); OpenGMK's own loading of DLLs is Windows-only code and is
  not carried.
- **What can a test run on a public runner?** The game made here
  (`waterbox/tests/mkgame`). OpenGMK reads a game's `.exe` through its own
  reader (gm8exe), which also has the writers for every asset, so the maker is
  small: it writes the Windows executable's frame, the two scramblings of the
  data and the assets (one object with GML in three events, one room, one
  generated tone). It is all this repository's own work, so it may be kept.
- **How big is it?** `memoryLayoutMiB` 512/16/16/256/1024: a game's pictures
  and sounds are unpacked on the heap; the test game uses little of it.

No change to Chimera or miniBox was needed.

## What the core is

Rust, like the Ruffle and touchHLE cores; the recipe is theirs: a nightly
compiler, `-Z build-std` for a copy of the musl target with the large code
model, no red zone and no native thread-local variables
(`guest/waterbox-guest.json`), linked with miniBox's C++ kit and a guest Mesa.
`guest/src/lib.rs` has the exports; `run-native` is the same crate on the host
so the two can be compared.

| Part | Decision |
|---|---|
| Window | a stand-in crate, `ramen`, with the types OpenGMK touches and nothing behind them (patch 1) |
| Sound | OpenGMK's mixer, pulled a step at a time: 44.1 kHz stereo, a step's share of a second of samples with the remainder carried; udon without its devices (patch 2) |
| Picture | OSMesa; the window's framebuffer is bound for the one call that makes Mesa copy the picture out (patch 3) |
| Clock, randomness | patch 4 |
| One thread | patch 5 |
| Steps | `chimera_start` and `chimera_step` (patch 6), which is the body of OpenGMK's frame loop with the window, the limiter and the host's clock taken out |
| Files | the files a game writes are in memory (`memfs.rs`); a file it did not write is read from the files the project mounted. Export Save Data lists them, a `savedata` slot puts them back |
| Game State | the game's own numbers are a memory domain with a property table; a value changed there reaches the game before the next step |

A room transition (a fade, a slide) is drawn by OpenGMK in one call, several
pictures long; it is inside one step of the movie, as upstream's replays have
it. A step is as long as the room's speed makes it (`GetVsyncNumerator`).

### What the core does not do

- `external_call`/`external_define` of a DLL, GameMaker extensions' DLLs, the
  clipboard, `execute_program`, `execute_shell`: nothing to run them on.
- `file_find_first` and the directory functions see only the files the game
  has written, and the folder it started in is empty: a game with extra files
  next to it (a sprite it loads at run time) does not find them. A game that
  needs them is a request for a multi-file slot (docs/multi-file.md).
- Games for GameMaker Studio and later.
- Video and sound capture, and OpenGMK's tool-assisted-speedrun window: those
  are Chimera's.
- The same real game has not been run (none whose licence allows it was
  found); see "What was proved" below.

## What was proved (2026-11-20)

On the test game (a 320x240 room at 30 steps a second: a ball, text, shapes,
a tone, a save file), on Linux, with a build of Chimera made the same day:

- The test script, `run-gate.sh -r <chimera>`: 13 of 13 pass (table in
  BUILDING.md). Native and sandbox print the same clock, picture and sound
  hashes at every thirtieth of 300 steps on a route of keys and the mouse; a
  savestate saved and loaded before every step, and the state carried to a new
  process half way, change nothing; the host stalling 700 ms twice changes
  nothing; a poke of the score through "Game State" reaches the game in both.
- `run-gate.sh -b`: every one of those tests fails when broken on purpose (12
  of 12; the engine test is not run in that mode). The first version of the
  breaking mode let three tests pass because the baseline was broken too; the
  baseline is now made once, unbroken.
- `waterbox/tests/run-frontend.sh`: the package played through chimera-run and
  through Chimera's window run `--headless` gives the native reference's last
  picture, pixel for pixel (320x240); the package's bindings are Chimera's
  defaults. Broken on purpose: a different reference route fails both picture
  checks; a changed binding fails the bindings check.
- Chimera's contract tests (`test-packages`) on the package: 0 failed.
- The package carries Export Save Data (miniBox's kit window) and the texts in
  all 25 languages.
- Speed, on a machine under a load of about 250: the native reference runs
  the test game at about 80 steps a second after its start, the sandbox at
  about 80; starting a game costs about 7 s in the sandbox (reading the 3.8 MB
  file and compiling the game's GML, with softpipe compiling the shaders). A
  state of the test game is 4.4 MB.

### What was not proved

- **No real game.** OpenGMK's own repository has none, and a freeware
  GameMaker 8 game whose licence plainly allows this use was not found. Every
  claim above is about the test game. What a real game would test that this
  one does not: sprites and backgrounds (textures through softpipe), surfaces,
  3D (`d3d_*`), particles, paths, timelines, views, room transitions, mp3 and
  longer sounds, and a GML function OpenGMK has not implemented (upstream
  panics there, and the core stops with the panic's message).
- **Windows and macOS.** The package is the same file; it was run only on
  Linux. (Chimera and miniBox were not changed, so there is nothing for the
  Windows build to compile.)
- **Speed on a real game.** softpipe is a CPU rasteriser: a game that fills a
  large window with many layers each step may run slower than real time.
- **Lag frames.** Every step counts as one that read the controls.
- **A game's own extra files** (see "What the core does not do").
