# AGENTS.md - OpenGMK core for Chimera

This repository builds OpenGMK (a runner for GameMaker 8.0 and 8.1 games) as a
sandboxed guest for [Chimera](https://github.com/ToolAssisted-run/chimera), a
frontend for tool-assisted speedruns, and packs it into one file,
`opengmk.chimeraCore`. Upstream OpenGMK is a pinned submodule; everything here
is the patch series that makes it a step-at-a-time machine, the adapter, the
build scripts and the test script. The core is Rust, with one C dependency (the
LZ4 library) and Mesa. Builds run on Linux, and the one package works on Linux
and on Windows. It is a game core: it runs one game, the player's own `.exe`.

## Layout

- `extern/OpenGMK` - upstream OpenGMK, a submodule pinned by commit.
- `patches/` - the numbered patch series applied to `extern/OpenGMK` by
  `waterbox/apply-patches.sh`, which refuses a partly patched tree.
- `waterbox/guest/` - the core's crate (`src/lib.rs` holds the exports the
  engine calls, `src/mathlib.rs` the maths, `src/input_table.rs` the keys,
  generated), its `.cargo/config.toml` and its target, `waterbox-guest.json`.
  Built twice: for the guest and for the host.
- `waterbox/run-native/` - the native reference: the same crate on the host.
  `waterbox/run-wbx.c` - the runner that loads `core.wbx` through miniBox.
- `waterbox/tests/mkgame/` - makes the test game (a GameMaker 8.1 executable
  written from this repository's own code).
- `waterbox/guest-syscalls.cpp` - guest shims and Export Save Data's window.
- `waterbox/setup-mesa.sh`, `build-guest.sh`, `build-package.sh` - the builds.
- `waterbox/run-gate.sh` - the test script.
- `waterbox/input-table.py` - the one source of the 103 buttons, their keys and
  their mnemonics; it writes `guest/src/input_table.rs` and prints the config's
  `input` section.
- `waterbox/i18n/texts.py`, `write-languages.py` - the texts, in every language
  Chimera has; `waterbox/languages/*.json` are written from them.
- `waterbox/waterbox.config`, `file_slots.json`, `default_keybinds.json`,
  `package-licenses.json` - what the package declares.
- `docs/PLAN.md` - the design log. `docs/BUILDING.md` - every step.
  `.github/workflows/chimera.yml` - CI, the authoritative build recipe.
- `build/` - all build output, ignored by git.

## Build and test

`<chimera>` is a Chimera checkout, built (`tools/ci-chimera.sh`), and `<minibox>`
is `<chimera>/extern/chimera-common-minibox`, built with its C++ guest kit
(AGENTS.md of Chimera, "Build"). Needs rustup nightly with rust-src.

```sh
./waterbox/build-package.sh -m <minibox> -r <chimera>   # writes <chimera>/build/Cores/opengmk.chimeraCore
./waterbox/run-gate.sh -m <minibox> -r <chimera>        # the test script: every test
./waterbox/run-gate.sh -m <minibox> -n -b               # break each test on purpose: each must fail
<chimera>/build/meson-linux/source/ui/test-packages <folder holding the package>
```

The last line of the test script must read `test script: N passed, 0 failed,
...` and it must exit 0. A skipped test is not a passed one: `the engine` is
skipped without `-r`.

## Rules of this repository

- Never commit inside `extern/OpenGMK`. A change to upstream is a numbered file
  in `patches/`. After editing the tree, write the series again from it (every
  patch is the diff of the files of one topic against upstream's commit); the
  series is judged as a whole, so an edit in the tree that is not in a patch
  stops the next build as "partly patched".
- Determinism is the product. The guest must not read the host's time,
  randomness or anything else that differs between runs. The game's clock is
  the date in a setting plus the steps it has run; `randomize()` draws from the
  core's own sequence. A savestate must round-trip. The test script checks it.
- The native reference and the sandbox must be the same machine: both use the
  core's own maths (`mathlib.rs`) and the same pinned Mesa.
- A new test is broken on purpose once and seen to fail (`run-gate.sh -b`).
- Never commit a game. The test game is made from source; a player's games stay
  on their own disk. Never add network access to the core.
- Documentation prose is plain ASCII.
- Commit messages: `type(scope): what is now true (#issue)`; the body says what
  changed, why, how it was proven and the tally of the test script.
- Problems with this core are reported in Chimera's issue tracker, not here.
