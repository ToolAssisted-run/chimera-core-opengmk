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
  llvmpipe (softpipe until 2026-11-28: see "Drawing with llvmpipe") compiled
  into the core (the second way in porting-a-core.md).
  OpenGMK needs OpenGL 3.3; the software driver behind OSMesa gives a 3.3 compatibility
  context, which its shaders accept. The picture is read from OSMesa's buffer,
  so it is the same on every host. The GPU bridge is not used (and would put a
  game's textures outside the savestate).
- **Does it recompile?** No. GML is compiled by OpenGMK to its own
  instructions and interpreted.
- **What must the user bring?** The game: a GameMaker 8.0 or 8.1 `.exe`. A
  game that calls a Windows DLL (`external_call`, or a function of a DLL inside
  a GameMaker extension) cannot, except for the sound DLLs OpenGMK answers
  itself and the core runs silently (there is no Windows in the sandbox); any
  other DLL stops the game, when it calls it, with a sentence naming it. See
  "A Windows library the game calls (2026-11-25)".
- **What can a test run on a public runner?** The game made here
  (`waterbox/tests/mkgame`). OpenGMK reads a game's `.exe` through its own
  reader (gm8exe), which also has the writers for every asset, so the maker is
  small: it writes the Windows executable's frame, the two scramblings of the
  data and the assets (one object with GML in three events, one room, one
  generated tone). It is all this repository's own work, so it may be kept.
- **How big is it?** `memoryLayoutMiB` 512/16/16/256/3072 (the last was 1024 until issue 646): a
  game's pictures and sounds are unpacked on the heap and its textures are
  made in the last area; the test game uses little of it.

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
  clipboard, `execute_program`, `execute_shell`: nothing to run them on. The
  one exception is the sound DLLs listed in "A Windows library the game calls
  (2026-11-25)", which run silently. A game that calls any other DLL stops,
  and the person is told which DLL.
- `file_find_first` and the directory functions see only the files the game
  has written, and the folder it started in is empty, except for what a game
  given as a zip brings (see "A game given as a zip (2026-11-26)"). A game
  that loads files at run time (a sprite, a sound) from a bare .exe does not
  find them.
- Games for GameMaker Studio and later.
- Video and sound capture, and OpenGMK's tool-assisted-speedrun window: those
  are Chimera's.
- The same real game has not been run (none whose licence allows it was
  found); see "What was proved" below.

## A function OpenGMK does not have (2026-11-20)

OpenGMK does not implement every GameMaker function: 264 of them (`show_message`
for one) are stubs that panic with "Called unimplemented kernel function
<name>". Before, the core stopped with that panic's text, a Rust source file and
a line number. Decided by the owner on 2026-11-20: the person gets a plain
sentence naming the function, "This game called show_message, which OpenGMK does
not implement yet."

- The core installs a panic hook when it starts (`say_unimplemented_functions`
  in `guest/src/lib.rs`). For that one panic it writes the sentence, taken from
  `languages/en.json` (`error.unimplemented`), as the machine's last words and
  nothing else; any other panic keeps its own text. The machine then stops as
  any panic stops it.
- Chimera shows what a stopped machine said: "the core stopped: the core
  aborted. It said: This game called show_message, which OpenGMK does not
  implement yet." The game's stopped step is not played, and the person can go
  back to an earlier state as after any stop.
- The text exists in all 26 tables Chimera has (`error.unimplemented`, in
  `waterbox/i18n/texts.py`; `write-languages.py` writes the files). What the
  window shows for a stopped machine is the machine's own words, which Chimera
  does not look up in the tables (it does for a game that fails to start), so
  today the sentence shows in English in every language. A later change in
  Chimera that puts a stop's reason through the tables would show the
  translations; the core would then write the key and its argument instead.
  Chimera was not changed for this.
- The test script has a test for it: `mkgame <exe> show_message` makes a game
  that calls `show_message` at its tenth step; the native reference, the
  sandbox and chimera-run (the package in a project) each must print the
  sentence. Broken on purpose, the test runs the game that calls nothing and
  fails.
- Not covered: the six other places OpenGMK stops with `todo!` (reading or
  setting `keyboard_lastchar` and `keyboard_string`, and the functions
  `keyboard_key_press` and `keyboard_key_release`), which keep the panic's text.
- Only the test game made by this repository's own tests was run. No real
  GameMaker 8 game was run: which functions real games call that OpenGMK lacks
  is not known. The owner chose to publish the core as tested on 2026-11-20.

## A Windows library the game calls (2026-11-25)

Some games call a Windows DLL with `external_define` and `external_call`.
Upstream OpenGMK runs it in a helper program (`gm8emulator-wow64.exe`, through
wine on Linux) that it starts as a second process. The sandbox has no Windows,
no second process and no path of its own program, so upstream's code panicked
with "no /proc/self/exe available" (issue 641, a fan game that calls a DLL).
Decided by the owner on 2026-11-25:

- A game that asks for a DLL the core cannot run stops with one sentence naming
  it: "This game calls a Windows library (mystery.dll, function Foo), which
  this core cannot run." Patch 0008 makes `ExternalManager::define` answer with
  an error that carries the library's file name and the function instead of
  starting the helper. The core turns that into the sentence
  (`error.no_library`, in all 26 tables). If the game asks in its first event
  the game does not start and Chimera says so as for any game that cannot
  start; if it asks later the machine stops with the sentence as its last
  words, as for a function OpenGMK lacks.
- OpenGMK's own silent stand-ins for sound DLLs are on. Upstream turns them on
  only when it records (`dummy_audio`); the core passes true, so a game that
  only uses `gmfmodsimple.dll`, `ssound.dll`, `supersound.dll`, `sgaudio.dll`,
  `sxms-3.dll` or `caster.dll` runs, without that sound. (`gmeffect_0.1.dll`
  was already answered silently upstream.) Each call answers a fixed value and
  does nothing else, so a game that checks what the DLL answered gets "all
  zero" (or "1" from `FMODSoundAdd`, "Yes" from `SS_Init`).
- Determinism and savestates: a stand-in reads nothing of the host and keeps no
  state. A savestate holds only the list of declared externals (library,
  function, types), and loading it makes the stand-ins again, so it round-trips;
  the sound a game would have made through the DLL is not made, and the
  core's sound is already outside the savestate. The test script checks a game
  that uses a stand-in under a savestate before every step.
- A DLL inside a GameMaker extension (not `external_define`): the same two
  rules. A DLL with a stand-in runs on it (upstream's list goes by the DLL's
  file name, which is the extension file's name). Any other is passed over at
  start with a warning on the log, as upstream does for a library that fails to
  load, and the game starts and runs; but upstream then answers 0 to every call
  of its functions, so a game could go on, different from the real thing,
  without saying so. Chosen by the owner on 2026-11-25: a call to a function of
  such a DLL stops the game with the same sentence (the DLL's file name and the
  function's name in the DLL, not the name GML calls it by). Patch 0009 keeps a
  marker for that function in place of "nothing" (`ExtensionFunction::NoLibrary`,
  holding patch 0008's text) and calling it is an error carrying the text, which
  the core turns into the sentence through the same path as for
  `external_define`. A game that never calls the DLL is unchanged. The
  extension's initializer and finalizer functions are called by the engine, not
  by the game, so for such a DLL they are passed over (they answer nothing), or
  every game with that extension would stop at its start. GML functions of an
  extension are not affected. Which DLL a real game needs, and whether any real
  game works now, was not tried: no game was run, only the test game.
- Tests: `mkgame <exe> dll:gmfmodsimple.dll:FMODSoundAdd` calls a stand-in at
  its tenth step (`standin`: native, sandbox and a savestate before every step
  agree, and the answer shows in the picture); `dll:mystery.dll:Foo` and
  `dllc:mystery.dll:Foo` ask for an unknown DLL at the tenth step and in the
  first event (`nolibrary`: the sentence, in the native reference, the sandbox
  and chimera-run, and no mention of the helper program or a panic).
  `mkgame <exe> ext:LIBRARY:FUNCTION` makes a game with a GameMaker extension
  holding that library and function (GML name `ext_FUNCTION`) and never calling
  it, `extcall:...` calls it at the tenth step too. Three tests: `extunused`
  (`ext:mystery.dll:Foo`: the game starts and runs, with the same digests as
  the game without the extension, in the native reference and the sandbox, and
  the log says the function was passed over), `extcalled`
  (`extcall:mystery.dll:Foo`: the sentence, in the native reference, the sandbox
  and chimera-run, and no panic text) and `extstandin`
  (`extcall:gmfmodsimple.dll:FMODSoundAdd`: it runs silently on the stand-in,
  the same in both and through a savestate before every step, and the answer
  shows in the picture).

## A game with a lot of pictures (2026-11-26, issue 646)

Sonic.exe Nightmare Beginning (the 2017 version, a free fan game, English
build from its author's Game Jolt page) did not start: "Failed to allocate
texture on GPU! (OpenGL code 1285)", which is GL_OUT_OF_MEMORY. It is not the
player's graphics card: the core draws with Mesa's softpipe inside the
sandbox on every machine, so the same failure happens on Linux and Windows.

What was seen (Linux, through `run-wbx`, the sandbox's own memory):

- OpenGMK asks the OpenGL for its largest texture size (softpipe says 16384)
  and packs all the game's sprites and backgrounds into textures that grow
  by doubling up to that size. This game's pictures need one texture of
  16384 x 8192 pixels, which is 512 MiB.
- The pictures are first unpacked on the heap and then copied into that
  texture, and both live at once. The memory the game maps itself (the last
  number of `memoryLayoutMiB`) was 1024 MiB. miniBox printed "arena
  exhausted: 512 MiB wanted, 442..500 MiB free", and Mesa answered
  GL_OUT_OF_MEMORY. (With the texture limited to 8192 x 8192 it needed two
  such textures and failed on the second one, 244 MiB free of 256 wanted.)
- With the area at 1536 MiB the game starts; 3072 MiB was chosen so that the
  largest texture Mesa allows (16384 x 16384, 1 GiB) also fits with the
  pictures. Only the pages a game touches cost anything, and other cores
  declare as much.

Test: `run-gate.sh` makes a game of six sprites of 4096 x 4096 pixels
(`mkgame OUT.exe bigpics`), runs it natively and in the sandbox and compares
the pictures. Before the change it stops with the error above (checked by
giving the sandbox 1024 MiB again, which is what `-b` does); after it, both
agree. Not tried: the Windows build of the package, and a real card (the core
never uses one).

## Speed on a real game (2026-11-27, issue 665)

Sonic.exe Nightmare Beginning (2017) plays at about 2.5 steps a second in its
data-select screen on a machine under heavy load, native and sandbox alike
(the game asks for 60). Measured: about 90 percent of the time is softpipe
running the game's pixel shader in its interpreter (about 475,000 pixels and
3 million shader instructions a step); sampling the textures, the 16384 x 8192
texture, the copy of the picture and the sandbox itself are each under 5
percent. The sandbox does not slow the picture down: the native reference is
as slow.

Mesa is now built with `-Dbuildtype=release` (optimization level 3, no
assertions) for both flavours: 1.2 to 1.4 times as many steps a second, the
same picture (digests at steps 500, 600 and 700, native and sandbox, equal to
the build before). Nothing that would change the picture was changed; the
options measured are in issue 665.

## Drawing with llvmpipe (user-decided, 2026-11-28, issue 665)

The owner chose to draw with Mesa's llvmpipe, which turns each shader into
machine code with LLVM, instead of softpipe, which interprets it. Chimera's
Godot, Ikemen GO and Supermodel cores draw this way already; this core uses
their recipe (Mesa 24.0.9, LLVM 18.1.8, `waterbox/build-llvm.sh`,
`waterbox/mesa-patches/0001`, `waterbox/jit-symbols.c`).

Measured on Sonic.exe Nightmare Beginning, the same 700 steps of input, one
pinned core of a machine with a load of about 1 (steps a second):

| steps | softpipe native | softpipe sandbox | llvmpipe native | llvmpipe sandbox |
|---|---|---|---|---|
| 200 to 400 | 85 to 126 | 85 to 124 | 556 to 483 | 550 to 365 |
| 500 to 600 | 17.1 | 16.9 | 138 | 136 |
| 600 to 700 | 8.8 | 8.7 | 73.1 | 73.0 |

The game asks for 60. (The 2.5 to 3.5 steps a second of the issue were
measured on a machine under a load of 340; the proportion, 8.4 times, is the
same.) Starting the game takes about 11 s in the sandbox and 4.5 s natively,
as before.

What was decided and checked:

- **One picture on every computer.** Mesa is told of one baseline x86-64 CPU
  (SSE2, 128-bit vectors, one processor) and LLVM generates code for
  "x86-64" whatever the computer is. No AVX2 setting (the owner, 2026-11-28).
  Built without that, the same Mesa on this machine drew another picture at 53
  of 71 checkpoints; with it, every run is the same. `LP_NUM_THREADS` and
  `LP_NATIVE_VECTOR_WIDTH` change nothing in the pinned build. Only one CPU
  model (an AMD) was tried; the sandbox does not use the host's maths library
  and agrees with the native reference to the pixel.
- **Savestates and determinism.** Checkpoints every 10 steps over 700 steps
  (and every 50 over 1500) are the same native and sandbox, run to run, with a
  save and load before every step, and with the host torn down and rebuilt
  half way. The compiled shader code is in the machine's memory, so it is
  saved and restored. A state is 23 MB instead of 18 MB (1.5 MB instead of
  0.45 MB compressed).
- **Machine spec 3.** The core declares `__wbx_machine_spec = 3`
  (`waterbox/guest-syscalls.cpp`). LLVM's code generator maps memory with a
  bare address hint; under spec 2 miniBox takes that as a fixed address even
  over memory in use (the same cause as Supermodel's first-frame crash);
  under spec 3 it puts it elsewhere. Pictures and speed are the same under
  both here; the declaration is a safety.
- **Mesa's shader cache is off** (`-Dshader-cache=disabled`). llvmpipe's
  cache wrote compiled shaders to `~/.cache/mesa_shader_cache` from the native
  reference, a file written on the host.
- **The pictures change once.** llvmpipe's picture is close to softpipe's (on
  screenshots at steps 100 to 700, 53 to 2130 of 1,228,800 bytes differ) but
  not equal, so a movie recorded with the softpipe core must be played again
  from the start to see the same pictures; the game's own state and its steps
  do not change. No test stores a softpipe picture: the test script's checks
  compare two runs.
- **Size.** Package 10.5 MB with softpipe, 35.1 MB with LLVM built for speed,
  26.5 MB with the sandbox's LLVM built for size (`-Os`; core.wbx without debug
  sections 46 MB, 114 MB, 95 MB). LLVM's own optimisation level does not
  change the code it generates: the pictures are equal and the speed is equal
  (73.0 steps a second). Two libraries, LLVM's disassemblers (Mesa uses them
  only for debug dumps), are built at `-O2`, because GNU ld 2.42 refuses their
  `-Os` objects. `-ffunction-sections` with `--gc-sections` was tried and
  gave the same ld error; it is not used.
- **Build.** `build-llvm.sh native` and `guest` run before `setup-mesa.sh`
  (`build-package.sh` and `run-gate.sh` do it); each LLVM takes a few minutes
  on a machine with 64 jobs and about fifteen on a small one. CI caches
  `build/llvm-native`, `build/llvm-guest`, `build/mesa` and `build/deps`,
  keyed by the scripts and the Mesa patch. The package is one x86-64 guest
  for Windows, Linux and macOS: only Linux builds it (CI has one job), so
  there is nothing LLVM or Mesa to build for the other two.
- **Licence.** LLVM (Apache-2.0 with the LLVM exception) is in
  `package-licenses.json` and its licence text in `waterbox/licenses/`.
- **Not proved.** Windows and macOS (the same package; macOS cannot map memory
  writable and executable at once, and the sandbox gives a page RW or RX
  there, which is how LLVM's loader uses it; not run), a second CPU model,
  aarch64 (the core ships for x86-64 only; llvmpipe's CPU pin is an x86
  patch), and any game other than Sonic.exe Nightmare Beginning.

## A game given as a zip (2026-11-26, issue 646; the way chosen by the owner, 2026-11-26)

After the memory fix above, Sonic.exe Nightmare Beginning started but stopped
at step 63 with "nonexistent asset id -1 (sound)". The cause was found by
reading the game's code (a small program built on OpenGMK's own `gm8exe`
reader listed every object, script and room code):

- The game has no sounds inside its .exe (all 140 sound slots are empty). Its
  first object, `Sounder`, builds the names `working_directory +
  "/Sound/<name>.mp3"` (or .wav) and calls `sound_add` on each.
- The core mounted only the .exe, so every `sound_add` found no file and
  answered -1, as GameMaker does for a missing file. The game then played
  sound -1, and OpenGMK's `sound_play` stops on an unknown sound (its source,
  `kernel.rs`). So the -1 was a missing-file answer, not a failure of the
  core's sound; no stand-in DLL or extension was involved. Whether the
  original runner tolerates `sound_play(-1)` was not looked up and was not
  needed; the core does not change that.

What was decided (owner, 2026-11-26): the game slot takes the .exe as before,
or a .zip of the game's whole folder, as the ScummVM core's slot does. No
other slot.

- With a zip the core finds the game's .exe inside: the only .exe at any
  depth, or, if there are several, the one OpenGMK can read as a GameMaker 8
  game (it tries to read each; the others are setup programs and the like).
  If none is a game, or more than one is, the game does not start and the
  sentence names the .exe files it found ("None of the .exe files in the
  game's zip is a GameMaker 8 game: A, B"). A zip with no .exe says so. The
  texts are in every language the core has (translated by the agent that wrote
  them, not reviewed by a native speaker).
- The game's working directory is the .exe's folder inside the zip: its other
  files, and what is below them, are read into read-only memory at start
  (patch 0010, `memfs::put_asset`), before the machine is sealed, so
  savestates and replays see them and they are never listed as save data. A
  name is matched without regard to upper or lower case, as Windows does, and
  without the game's `working_directory` in front (that is `/` in the sandbox
  and a temporary folder in the native reference). Every read of a game's file
  goes through `memfs` (`sound_add`, `file_exists`, `file_bin_open`...), so
  they all find these files. Files outside the .exe's folder cannot be named by
  the game and are dropped.
- A plain .exe is untouched. The project pins the zip's hash as it pins the
  .exe's.

What was seen (Linux, sandbox, `run-wbx`, the real game from a zip of its
downloaded folder): see "What was proved" in the commit; the game runs past
step 63 where it stopped.

Tests (`run-gate.sh`): `ownfiles` (a zip with `Folder/Game.EXE` and
`Folder/Sound/tone.WAV`, the game asks for `/sound/Tone.wav`: native, sandbox
and a savestate before every step agree, and the picture shows the sound was
loaded, unlike the bare .exe), `zipchoice` (two .exe files that are not games
beside the game: it runs the same), `zipnogame` (no game in the zip: refused
with the .exe files named; no .exe: says so). Each was broken on purpose once.

## What was proved (2026-11-20)

On the test game (a 320x240 room at 30 steps a second: a ball, text, shapes,
a tone, a save file), on Linux, with a build of Chimera made the same day:

- The test script, `run-gate.sh -r <chimera>`: 14 of 14 pass (table in
  BUILDING.md). Native and sandbox print the same clock, picture and sound
  hashes at every thirtieth of 300 steps on a route of keys and the mouse; a
  savestate saved and loaded before every step, and the state carried to a new
  process half way, change nothing; the host stalling 700 ms twice changes
  nothing; a poke of the score through "Game State" reaches the game in both.
- `run-gate.sh -b`: every one of those tests fails when broken on purpose (13
  of 13; the engine test is not run in that mode). The first version of the
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
  longer sounds, and a GML function OpenGMK has not implemented (the core
  stops there and says which function; see "A function OpenGMK does not have").
- **Windows and macOS.** The package is the same file; it was run only on
  Linux. (Chimera and miniBox were not changed, so there is nothing for the
  Windows build to compile.)
- **Speed on a real game.** softpipe is a CPU rasteriser: a game that fills a
  large window with many layers each step may run slower than real time.
- **Lag frames.** Every step counts as one that read the controls.
- **A game's own extra files** (see "What the core does not do").
