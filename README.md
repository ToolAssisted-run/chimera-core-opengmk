# chimera-core-opengmk

[OpenGMK](https://github.com/OpenGMK/OpenGMK)'s runner for games made with
GameMaker 8.0 and 8.1 as a [Chimera](https://github.com/ToolAssisted-run/chimera)
core, running in miniBox's sandbox. A project is one game: the `.exe` the
player brings (optionally, the save data an earlier session exported). The core
carries no game.

- The keyboard and the mouse a GameMaker game reads: 103 keys and buttons, and
  the pointer across the game's window.
- One step of the game is one row of a movie. The game's date starts at a
  setting and runs with the steps, `randomize()` takes its seed from the core's
  own sequence (a setting), and nothing reads the host's clock.
- The picture is drawn by Mesa's softpipe inside the core, the sound mixed by
  OpenGMK's own mixer a step at a time, and the files a game writes (its
  `ini` and text files) live in the machine: savestates carry them, Export Save
  Data takes them out, a save-data slot puts them back.
- A game's own numbers (room, score, lives, health, the random seed, the
  instance count, the pointer) are a labelled memory domain, "Game State".

Layout: `extern/OpenGMK` (upstream, pinned; clone with `--recursive`),
`patches/` (the series that makes it a step-at-a-time machine), `waterbox/`
(the core, the native reference, the builds and the test script) and
`docs/PLAN.md` (the design and the reasoning behind every decision).

The tests run on a small game this repository makes itself
(`waterbox/tests/mkgame`), so no game is needed or downloaded. Only that test
game has been run: no real GameMaker 8 game has. OpenGMK does not implement
every GameMaker function (`show_message`, for one); a game that calls one stops
with a sentence naming it. Report problems
in [chimera's issues](https://github.com/ToolAssisted-run/chimera/issues).

## Using it in Chimera

Chimera ships no cores and downloads nothing. Download the `.chimeraCore`
package from this repository's
[Releases](https://github.com/ToolAssisted-run/chimera-core-opengmk/releases)
page, or build it, and put it in the `cores` folder beside Chimera's program (or
the folder chosen in Tools > Core Manager... > Change folder...). Tools > Core
Manager... lists the cores in that folder. The same package works on Linux and
on Windows. You bring the game: a game made with GameMaker 8.0 or 8.1, as its
`.exe`. Games made with GameMaker Studio or later do not run here.

## Building

`waterbox/build-package.sh -m <miniBox dir> -r <chimera checkout>` builds the
guest Mesa and the core and writes
`<chimera checkout>/build/Cores/opengmk.chimeraCore`. The requirements, the
steps that come before it and the test script are in
[docs/BUILDING.md](docs/BUILDING.md). [AGENTS.md](AGENTS.md) is the short
operating guide for a coding agent.
