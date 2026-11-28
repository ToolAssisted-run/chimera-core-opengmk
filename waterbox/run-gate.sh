#!/bin/bash
# The OpenGMK core's test script: every test compares two runs that must agree,
# or checks a thing that must hold, and says what it compared.
#
# Content: the test game, made by tests/mkgame from nothing but this
# repository's own code (a GameMaker 8.1 executable: a bouncing ball that
# reads the keyboard and the mouse, draws shapes and text, plays a tone, keeps
# a save file). No one else's game is used or needed; a real GameMaker 8 game
# the person brings is tried by hand (docs/PLAN.md says which).
#
# Usage: ./run-gate.sh -m <miniBox dir> [-r <chimera root>] [-n] [-b] [-j N]
#   -n   do not rebuild (use what build/ holds)
#   -r   a chimera checkout with build/meson-linux/chimera-run, for the engine test
#   -b   break each test on purpose, once: a test that still passes is the
#        failure. (Every test that compares two runs makes the second one
#        different; every test that checks a property is given the case where
#        it does not hold.)
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
minibox="${MINIBOX_DIR:-}"
chimera_root=""
jobs="$(nproc)"
[ "$jobs" -gt 16 ] && jobs=16
rebuild=1
breaking=0
while getopts "m:r:nbj:" opt; do
	case "$opt" in
		m) minibox="$OPTARG" ;;
		r) chimera_root="$OPTARG" ;;
		n) rebuild=0 ;;
		b) breaking=1 ;;
		j) jobs="$OPTARG" ;;
		*) exit 2 ;;
	esac
done
[ -n "$minibox" ] || { echo "run-gate.sh: say where miniBox is (-m <dir> or MINIBOX_DIR)" >&2; exit 2; }
minibox="$(cd "$minibox" && pwd)"

build="$root/build"
work="$build/gate"
mkdir -p "$work"
native="$build/run-native/release/run-native"
wbx="$build/wbx/run-wbx"
core="$build/wbx/core.wbx"
mkgame="$build/mkgame/release/mkgame"
game="$build/content/testgame.exe"

pass=0
fail=0
skip=0
report() {
	local verdict="$1" what="$2" detail="${3:-}"
	case "$verdict" in
		PASS) pass=$((pass + 1)) ;;
		FAIL) fail=$((fail + 1)) ;;
		SKIP) skip=$((skip + 1)) ;;
	esac
	printf '%-4s %s%s\n' "$verdict" "$what" "${detail:+ - $detail}"
}

# ------------------------------------------------------------------ building
if [ "$rebuild" = 1 ]; then
	echo "building (the logs are in $work/)"
	bash "$here/build-llvm.sh" native -j "$jobs" > "$work/build-llvm-native.log" 2>&1 &&
	bash "$here/build-llvm.sh" guest -m "$minibox" -j "$jobs" > "$work/build-llvm-guest.log" 2>&1 &&
	bash "$here/setup-mesa.sh" -n -j "$jobs" > "$work/build-mesa-native.log" 2>&1 &&
	bash "$here/setup-mesa.sh" -m "$minibox" -j "$jobs" > "$work/build-mesa-guest.log" 2>&1 &&
	# the reference needs the patched tree too, and on a fresh clone it is
	# built before build-guest.sh has applied the series
	sh "$here/apply-patches.sh" > "$work/build-patches.log" 2>&1 &&
	(cd "$here/run-native" && CARGO_TARGET_DIR="$build/run-native" cargo +nightly build --release -j "$jobs") \
		> "$work/build-native.log" 2>&1 &&
	(cd "$here/tests/mkgame" && CARGO_TARGET_DIR="$build/mkgame" cargo +nightly build --release -j "$jobs") \
		> "$work/build-mkgame.log" 2>&1 &&
	bash "$here/build-guest.sh" -m "$minibox" -j "$jobs" > "$work/build-guest.log" 2>&1 &&
	cc -O2 -Wall -I"$minibox/source/host" -o "$wbx" "$here/run-wbx.c" \
		"$minibox/build/meson-linux/source/host/libminiboxhost.so" \
		-Wl,-rpath,"$minibox/build/meson-linux/source/host" > "$work/build-run-wbx.log" 2>&1 || {
		echo "the build failed; see $work/build-*.log" >&2
		exit 1
	}
fi
mkdir -p "$build/content"
"$mkgame" "$game" > /dev/null || { echo "mkgame failed" >&2; exit 1; }
for f in "$native" "$wbx" "$core" "$game"; do
	[ -e "$f" ] || { echo "missing $f (run without -n)" >&2; exit 1; }
done

# a sandboxed run under a memory cap where systemd allows one (not on CI)
if systemd-run --user --scope -q true 2>/dev/null; then
	box() { systemd-run --user --scope -q -p MemoryMax=8G "$@"; }
else
	box() { "$@"; }
fi
# a zip of files, every entry dated the same so the file is a function of its
# contents: make_zip OUT.zip NAME=PATH...
make_zip() {
	python3 - "$@" <<'PY'
import sys, zipfile
out = sys.argv[1]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for arg in sys.argv[2:]:
        name, path = arg.split("=", 1)
        info = zipfile.ZipInfo(name, (2026, 11, 26, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        z.writestr(info, open(path, "rb").read())
PY
}
# a short sine wave as a .wav file
make_tone() {
	python3 - "$1" <<'PY'
import math, struct, sys, wave
with wave.open(sys.argv[1], "wb") as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(8000)
    w.writeframes(b"".join(struct.pack("<h", int(8000 * math.sin(i / 5.0))) for i in range(800)))
PY
}
# digest lines only: what the game prints on stdout is not the machine's state
digest() { grep -E '^(frame |frames=)' "$1"; }
button() { python3 "$here/input-table.py" --index "$1"; }
SPACE="$(button 'Key Space')"
RIGHT="$(button 'Key Right')"
LEFT="$(button 'Key Left')"
MLEFT="$(button 'Mouse Left Button')"

# A route through the game: a tap of the space bar, the right arrow held, the
# pointer moved and its button held, another tap.
route=(--press "10:$SPACE" --release "13:$SPACE" --press "30:$RIGHT" --release "80:$RIGHT"
	--mouse 50:2000:3000 --press "50:$MLEFT" --release "120:$MLEFT" --mouse 90:6000:1000
	--press "140:$SPACE" --release "141:$SPACE")
# what makes the second run of a test different when that test is being broken
sabotage() { if [ "$breaking" = 1 ] && [ "${BREAK:-}" = "$1" ]; then echo "--press 5:$LEFT --press 5:$SPACE --release 6:$SPACE"; fi; }
# a test that is being broken passes only by failing
verdict() { # verdict <test> <ok 0|1> <what> <detail>
	local test="$1" ok="$2" what="$3" detail="${4:-}"
	if [ "$breaking" = 1 ]; then
		if [ "${BREAK:-}" = "$test" ]; then
			if [ "$ok" = 1 ]; then report FAIL "broken on purpose: $what" "it still passed"; else report PASS "broken on purpose: $what" "it failed, as it must"; fi
		fi
	else
		if [ "$ok" = 1 ]; then report PASS "$what" "$detail"; else report FAIL "$what" "$detail"; fi
	fi
}

run_native() { "$native" "$game" "$@" 2> /dev/null; }
run_wbx() { box "$wbx" "$core" "$game" "$@" 2> /dev/null; }

# the baseline every test below is compared with, made once and not broken
run_native --frames 300 --digest-every 30 "${route[@]}" > "$work/n.out"
run_wbx --frames 300 --digest-every 30 "${route[@]}" > "$work/w.out"

tests=(equal repeat stall rerecord session input settings clock sound save pokes refusal unimplemented bigpics ownfiles zipchoice zipnogame standin nolibrary extunused extcalled extstandin engine)
for t in "${tests[@]}"; do
	if [ "$breaking" = 1 ]; then BREAK="$t"; else BREAK=""; fi
	case "$t" in
	# ------------------------------------------------ the sandbox is the native run
	equal)
		run_wbx --frames 300 --digest-every 30 "${route[@]}" $(sabotage equal) > "$work/w2.out"
		n="$(digest "$work/n.out" | wc -l)"
		pictures="$(digest "$work/n.out" | awk '/^frame /{print $4}' | sort -u | wc -l)"
		ok=0; [ "$n" = 11 ] && diff <(digest "$work/n.out") <(digest "$work/w2.out") > /dev/null && ok=1
		verdict equal "$ok" "native == sandbox" "300 steps, clock, picture and sound every 30; $pictures different pictures in 10"
		;;
	# ------------------------------------------------ two runs are one run
	repeat)
		run_native --frames 300 --digest-every 30 "${route[@]}" $(sabotage repeat) > "$work/n2.out"
		ok=0; diff <(digest "$work/n.out" 2>/dev/null) <(digest "$work/n2.out") > /dev/null && [ -s "$work/n2.out" ] && ok=1
		verdict repeat "$ok" "native twice" "the same route twice lands the same"
		;;
	# ------------------------------------------------ the host is not the machine's clock
	stall)
		run_native --frames 300 --digest-every 30 "${route[@]}" --stall 100:700 --stall 200:700 $(sabotage stall) > "$work/ns.out"
		ok=0; diff <(digest "$work/n.out" 2>/dev/null) <(digest "$work/ns.out") > /dev/null && [ -s "$work/ns.out" ] && ok=1
		verdict stall "$ok" "the host stalling 700 ms twice changes nothing" "the game's clock is its own"
		;;
	# ------------------------------------------------ savestates
	rerecord)
		run_wbx --frames 300 --digest-every 30 "${route[@]}" --rerecord $(sabotage rerecord) > "$work/wr.out"
		ok=0; diff <(digest "$work/w.out" 2>/dev/null) <(digest "$work/wr.out") > /dev/null && [ -s "$work/wr.out" ] && ok=1
		verdict rerecord "$ok" "a savestate saved and loaded before every step changes nothing" "300 steps"
		;;
	session)
		run_wbx --frames 300 --digest-every 30 "${route[@]}" --session $(sabotage session) > "$work/ws.out"
		ok=0; diff <(digest "$work/w.out" 2>/dev/null) <(digest "$work/ws.out") > /dev/null && [ -s "$work/ws.out" ] && ok=1
		verdict session "$ok" "the state moved to a fresh host at step 150 lands the same"
		;;
	# ------------------------------------------------ the controls reach the game
	input)
		run_native --frames 300 "${route[@]}" > "$work/in.out"
		run_native --frames 300 $([ "$breaking" = 1 ] && echo "${route[@]}") > "$work/noin.out"
		a="$(digest "$work/in.out" | tail -1 | awk '{print $3,$4}')"
		b="$(digest "$work/noin.out" | tail -1 | awk '{print $3,$4}')"
		ok=0; [ -n "$a" ] && [ "$a" != "$b" ] && ok=1
		verdict input "$ok" "the keys and the pointer are seen" "the picture after the route differs from the untouched game's"
		;;
	# ------------------------------------------------ the settings are the machine's
	settings)
		run_native --frames 120 --setting randomSeed=5 > "$work/s5.out"
		run_native --frames 120 --setting randomSeed=$([ "$breaking" = 1 ] && echo 5 || echo 6) > "$work/s6.out"
		a="$(digest "$work/s5.out" | tail -1 | awk '{print $3,$4}')"
		b="$(digest "$work/s6.out" | tail -1 | awk '{print $3,$4}')"
		ok=0; [ -n "$a" ] && [ "$a" != "$b" ] && ok=1
		verdict settings "$ok" "another random seed plays another game" "randomize() takes its seed from the setting"
		;;
	clock)
		run_native --frames 30 --digest-every 30 --setting startTime=1000000000 > "$work/c.out"
		t="$(digest "$work/c.out" | head -1 | sed 's/.*time_ns=\([0-9]*\).*/\1/')"
		want=1000000000999999990
		[ "$breaking" = 1 ] && want=1262304000999999990
		ok=0; [ "$t" = "$want" ] && ok=1
		verdict clock "$ok" "the game's date starts at startTime and runs with the steps" "30 steps in: $t"
		;;
	# ------------------------------------------------ sound
	sound)
		run_native --frames 60 --digest-every 20 > "$work/snd.n"
		run_wbx --frames 60 --digest-every 20 $(sabotage sound) > "$work/snd.w"
		peaks="$(digest "$work/snd.n" | awk '/^frame /{sub("peak=","",$6); print $6}' | tr '\n' ' ')"
		read -r p20 p40 p60 _ <<< "$peaks"
		ok=0
		if [ "${p20:-0}" -gt 1000 ] && [ "${p40:-9}" -le 1 ] && diff <(digest "$work/snd.n") <(digest "$work/snd.w") > /dev/null; then ok=1; fi
		[ "$breaking" = 1 ] && [ "${p20:-0}" -gt 1000 ] && ok=0
		verdict sound "$ok" "the tone at the start is the same sound in both, then silence" "peaks $peaks"
		;;
	# ------------------------------------------------ save data in and out
	save)
		rm -f "$work"/save-*.zip
		run_native --frames 200 "${route[@]}" --export-save "$work/save-n.zip" > "$work/save.n"
		run_wbx --frames 200 "${route[@]}" --export-save "$work/save-w.zip" $(sabotage save) > "$work/save.w"
		run_native --frames 20 --savedata "$work/save-n.zip" --digest-every 20 > "$work/save2.n"
		run_wbx --frames 20 --savedata "$work/save-n.zip" --digest-every 20 --session > "$work/save2.w"
		run_native --frames 20 --digest-every 20 > "$work/save0.n"
		ok=0
		grep -q '^save log.txt ' "$work/save.n" && grep -q '^save save.ini ' "$work/save.n" \
			&& diff <(grep '^save ' "$work/save.n") <(grep '^save ' "$work/save.w") > /dev/null \
			&& diff <(digest "$work/save2.n") <(digest "$work/save2.w") > /dev/null \
			&& [ "$(digest "$work/save2.n" | tail -1 | awk '{print $3}')" != "$(digest "$work/save0.n" | tail -1 | awk '{print $3}')" ] && ok=1
		verdict save "$ok" "the files the game writes are exported, and a game started from them carries on" "$(grep '^save ' "$work/save.n" | cut -d' ' -f2-3 | tr '\n' ' ')"
		;;
	# ------------------------------------------------ a tool writes into the game's numbers
	pokes)
		poke=(--poke 40:16:777)
		run_native --frames 60 --digest-every 30 "${poke[@]}" > "$work/pk.n"
		run_wbx --frames 60 --digest-every 30 "${poke[@]}" $(sabotage pokes) > "$work/pk.w"
		run_native --frames 60 --digest-every 30 > "$work/pk0.n"
		ok=0
		diff <(digest "$work/pk.n") <(digest "$work/pk.w") > /dev/null \
			&& [ "$(digest "$work/pk.n" | tail -1 | awk '{print $3}')" != "$(digest "$work/pk0.n" | tail -1 | awk '{print $3}')" ] && ok=1
		verdict pokes "$ok" "a value written into Game State (the score) reaches the game, in both" "the picture shows the poked score"
		;;
	# ------------------------------------------------ what is not a game
	refusal)
		printf 'this is not a game' > "$work/notagame.exe"
		out_n="$("$native" "$work/notagame.exe" --frames 1 2>&1 >/dev/null | grep -c 'bad_game')"
		out_w="$(box "$wbx" "$core" "$work/notagame.exe" --frames 1 2>&1 >/dev/null | grep -c 'bad_game')"
		[ "$breaking" = 1 ] && out_n=0
		ok=0; [ "$out_n" -ge 1 ] && [ "$out_w" -ge 1 ] && ok=1
		verdict refusal "$ok" "a file that is not a GameMaker 8 game is refused with its reason, in both"
		;;
	# ------------------------------------------------ a function OpenGMK does not have
	unimplemented)
		# a game that calls show_message at its tenth step; when the test is
		# being broken, the game that calls nothing is run instead
		said="This game called show_message, which OpenGMK does not implement yet."
		ugame="$build/content/unimplemented.exe"
		if [ "$breaking" = 1 ]; then "$mkgame" "$ugame" > /dev/null; else "$mkgame" "$ugame" show_message > /dev/null; fi
		out_n="$("$native" "$ugame" --frames 30 2>&1 >/dev/null | grep -c -F "$said")"
		out_w="$(box "$wbx" "$core" "$ugame" --frames 30 2>&1 >/dev/null | grep -c -F "$said")"
		out_e=""
		run="${chimera_root:+$chimera_root/build/meson-linux/chimera-run}"
		pkg="${chimera_root:+$chimera_root/build/Cores/opengmk.chimeraCore}"
		if [ -n "$run" ] && [ -x "$run" ] && [ -f "$pkg" ]; then
			python3 -c 'import json,sys; n=len(json.load(open(sys.argv[1]))["input"]["buttons"]); print("\n".join("|%5d,%5d,%s|" % (4096, 4096, "." * n) for _ in range(30)))' \
				"$here/waterbox.config" > "$work/unimplemented.movie"
			rm -rf "$work/unimplemented.chimeraProject"
			"$run" --new-project "$work/unimplemented.chimeraProject" "$pkg" "$ugame" "$work/unimplemented.movie" > "$work/unimplemented.make.log" 2>&1
			box "$run" --project "$work/unimplemented.chimeraProject" "$pkg" --files "$build/content" > "$work/unimplemented.out" 2>&1
			out_e="$(grep -c -F "chimera-run: the core stopped: " "$work/unimplemented.out")/$(grep -c -F "It said: $said" "$work/unimplemented.out")"
		fi
		ok=0
		[ "$out_n" -ge 1 ] && [ "$out_w" -ge 1 ] && { [ -z "$out_e" ] || [ "$out_e" = "1/1" ]; } && ok=1
		verdict unimplemented "$ok" "a game that calls show_message, which OpenGMK lacks, gets a sentence naming it" \
			"native $out_n, sandbox $out_w${out_e:+, chimera-run $out_e}"
		;;
	# ------------------------------------------------ a game with a lot of pictures
	bigpics)
		# A game with six sprites of 4096 x 4096 pixels (384 MiB): OpenGMK packs
		# them into one texture of 16384 x 8192 pixels (512 MiB), and that, with the
		# pictures it is made from, did not fit in the 1024 MiB the package had for
		# memory mapped by the game ("Failed to allocate texture on GPU! (OpenGL
		# code 1285)", issue 646). It must start and run, the same as the native
		# run. When the test is being broken, the sandbox is given the 1024 MiB
		# again.
		pgame="$build/content/bigpics.exe"
		"$mkgame" "$pgame" bigpics > /dev/null
		"$native" "$pgame" --frames 30 --digest-every 10 > "$work/bigpics.n" 2> /dev/null
		if [ "$breaking" = 1 ]; then export RUN_WBX_MMAP_MIB=1024; fi
		box "$wbx" "$core" "$pgame" --frames 30 --digest-every 10 > "$work/bigpics.w" 2> "$work/bigpics.w.err"
		unset RUN_WBX_MMAP_MIB
		n="$(digest "$work/bigpics.n" | wc -l)"
		ok=0
		[ "$n" = 4 ] && diff <(digest "$work/bigpics.n") <(digest "$work/bigpics.w") > /dev/null && ok=1
		verdict bigpics "$ok" "a game with 384 MiB of pictures starts and runs, the same as native" \
			"$n digests; $(grep -c 'Failed to allocate texture' "$work/bigpics.w.err") texture failures"
		;;
	# ------------------------------------------------ a game that is a zip of its folder
	ownfiles)
		# A game that loads a sound with sound_add from the Sound folder beside its
		# .exe (Sonic.exe Nightmare Beginning loads 140 this way, and stopped at
		# its first sound_play with "nonexistent asset id -1", issue 646). The
		# game slot takes a zip of the game's folder: with it the sound is loaded
		# (the score is 1000 higher in the picture), the same in both and through a
		# savestate before every step; with the bare .exe sound_add answers -1. The
		# game's .exe and its folder are in another folder of the zip and in other
		# cases (Game.EXE, sound/tone.wav; the game asks for sound/Tone.wav).
		# When the test is being broken, the bare .exe is run.
		fgame="$build/content/ownfiles.exe"
		"$mkgame" "$fgame" ownfiles > /dev/null
		make_tone "$work/tone.wav"
		make_zip "$build/content/ownfiles.zip" "Folder/Game.EXE=$fgame" "Folder/Sound/tone.WAV=$work/tone.wav"
		if [ "$breaking" = 1 ]; then fz="$fgame"; else fz="$build/content/ownfiles.zip"; fi
		"$native" "$fz" --frames 100 --digest-every 20 > "$work/ownfiles.n" 2> /dev/null
		box "$wbx" "$core" "$fz" --frames 100 --digest-every 20 > "$work/ownfiles.w" 2> /dev/null
		box "$wbx" "$core" "$fz" --frames 100 --digest-every 20 --rerecord > "$work/ownfiles.r" 2> /dev/null
		"$native" "$fgame" --frames 100 --digest-every 20 > "$work/ownfiles.plain" 2> /dev/null
		a="$(digest "$work/ownfiles.n" | tail -1 | awk '{print $3,$4}')"
		b="$(digest "$work/ownfiles.plain" | tail -1 | awk '{print $3,$4}')"
		n="$(digest "$work/ownfiles.n" | wc -l)"
		ok=0
		[ "$n" = 6 ] && [ -n "$a" ] && [ "$a" != "$b" ] && diff <(digest "$work/ownfiles.n") <(digest "$work/ownfiles.w") > /dev/null &&
			diff <(digest "$work/ownfiles.n") <(digest "$work/ownfiles.r") > /dev/null && ok=1
		verdict ownfiles "$ok" "a game that is a zip of its folder finds its own files, the same in both and through savestates" \
			"$n digests; the picture shows the sound was loaded"
		;;
	zipchoice)
		# A zip with two .exe files that are not GameMaker games (a setup program, a
		# patcher) beside the game: the game is the one OpenGMK can read, and it
		# runs as in the test above. When the test is being broken the game is left
		# out of the zip.
		fgame="$build/content/ownfiles.exe"
		"$mkgame" "$fgame" ownfiles > /dev/null
		make_tone "$work/tone.wav"
		printf 'MZ this is not a GameMaker game' > "$work/setup.exe"
		printf 'MZ neither is this' > "$work/patch.exe"
		if [ "$breaking" = 1 ]; then ge=("zz/notes.txt=$work/tone.wav"); else ge=("Game/Game.exe=$fgame"); fi
		make_zip "$build/content/zipchoice.zip" "Tools/setup.exe=$work/setup.exe" "patch.EXE=$work/patch.exe" "${ge[@]}" "Game/Sound/tone.wav=$work/tone.wav"
		box "$wbx" "$core" "$build/content/zipchoice.zip" --frames 100 --digest-every 20 > "$work/zipchoice.w" 2> /dev/null
		"$native" "$build/content/zipchoice.zip" --frames 100 --digest-every 20 > "$work/zipchoice.n" 2> /dev/null
		n="$(digest "$work/zipchoice.n" | wc -l)"
		ok=0
		[ "$n" = 6 ] && diff <(digest "$work/zipchoice.n") <(digest "$work/zipchoice.w") > /dev/null &&
			diff <(digest "$work/zipchoice.n") <(digest "$work/ownfiles.n") > /dev/null && ok=1
		verdict zipchoice "$ok" "a zip with two .exe files that are not games and the game runs the game, the same as the zip without them" \
			"$n digests"
		;;
	zipnogame)
		# A zip with no GameMaker game in it is refused with a text that names
		# the .exe files it holds, in both; so is a zip with no .exe at all. When
		# the test is being broken the game is put into the zip.
		fgame="$build/content/ownfiles.exe"
		"$mkgame" "$fgame" ownfiles > /dev/null
		printf 'MZ this is not a GameMaker game' > "$work/setup.exe"
		printf 'MZ neither is this' > "$work/patch.exe"
		if [ "$breaking" = 1 ]; then ge=("Game/Game.exe=$fgame"); else ge=(); fi
		make_zip "$build/content/zipnogame.zip" "Tools/setup.exe=$work/setup.exe" "patch.EXE=$work/patch.exe" "${ge[@]}"
		make_zip "$build/content/zipnoexe.zip" "readme.txt=$work/setup.exe"
		# the core says the text's key and its arguments; the window turns them into
		# the sentence of the player's language
		key="error.zip_no_game"
		names="Tools/setup.exe, patch.EXE"
		key2="error.zip_no_exe"
		out_n="$("$native" "$build/content/zipnogame.zip" --frames 5 2>&1 >/dev/null | grep -F "$key" | grep -c -F "$names")"
		out_w="$(box "$wbx" "$core" "$build/content/zipnogame.zip" --frames 5 2>&1 >/dev/null | grep -F "$key" | grep -c -F "$names")"
		out2_n="$("$native" "$build/content/zipnoexe.zip" --frames 5 2>&1 >/dev/null | grep -c -F "$key2")"
		out2_w="$(box "$wbx" "$core" "$build/content/zipnoexe.zip" --frames 5 2>&1 >/dev/null | grep -c -F "$key2")"
		ok=0; [ "$out_n" -ge 1 ] && [ "$out_w" -ge 1 ] && [ "$out2_n" -ge 1 ] && [ "$out2_w" -ge 1 ] && ok=1
		verdict zipnogame "$ok" "a zip with no GameMaker game is refused naming its .exe files, and one with no .exe says so, in both" \
			"native $out_n/$out2_n, sandbox $out_w/$out2_w"
		;;
	# ------------------------------------------------ a Windows library the game asks for
	standin)
		# A game that defines and calls gmfmodsimple.dll, one of the sound libraries
		# OpenGMK answers itself, silently: it runs, the same in both and through a
		# savestate before every step, and the answer (1) is in its picture (the
		# score is 1000 higher), so the call was made. When the test is being
		# broken the game that calls nothing is run instead.
		sgame="$build/content/standin.exe"
		if [ "$breaking" = 1 ]; then "$mkgame" "$sgame" > /dev/null; else "$mkgame" "$sgame" dll:gmfmodsimple.dll:FMODSoundAdd > /dev/null; fi
		"$native" "$sgame" --frames 100 --digest-every 20 > "$work/standin.n" 2> "$work/standin.n.err"
		box "$wbx" "$core" "$sgame" --frames 100 --digest-every 20 > "$work/standin.w" 2> "$work/standin.w.err"
		box "$wbx" "$core" "$sgame" --frames 100 --digest-every 20 --rerecord > "$work/standin.r" 2> /dev/null
		run_native --frames 100 --digest-every 20 > "$work/standin.plain"
		a="$(digest "$work/standin.n" | tail -1 | awk '{print $3,$4}')"
		b="$(digest "$work/standin.plain" | tail -1 | awk '{print $3,$4}')"
		n="$(digest "$work/standin.n" | wc -l)"
		ok=0
		[ "$n" = 6 ] && [ -n "$a" ] && [ "$a" != "$b" ] && diff <(digest "$work/standin.n") <(digest "$work/standin.w") > /dev/null &&
			diff <(digest "$work/standin.n") <(digest "$work/standin.r") > /dev/null && ok=1
		verdict standin "$ok" "a game that calls gmfmodsimple.dll runs on OpenGMK's silent stand-in, the same in both and through savestates" \
			"$n digests; the picture shows the answer"
		;;
	nolibrary)
		# A game that calls a library nobody can run says so in a sentence naming
		# the library and the function, never with a Rust panic or a missing
		# helper program: asked for at its tenth step (the machine stops with the
		# sentence), and asked for in its first event (the game does not start).
		said="This game calls a Windows library (mystery.dll, function Foo), which this core cannot run."
		lgame="$build/content/nolibrary.exe"
		cgame="$build/content/nolibrary-start.exe"
		if [ "$breaking" = 1 ]; then
			"$mkgame" "$lgame" > /dev/null; "$mkgame" "$cgame" > /dev/null
		else
			"$mkgame" "$lgame" dll:mystery.dll:Foo > /dev/null; "$mkgame" "$cgame" dllc:mystery.dll:Foo > /dev/null
		fi
		"$native" "$lgame" --frames 30 > /dev/null 2> "$work/nolibrary.n"
		box "$wbx" "$core" "$lgame" --frames 30 > /dev/null 2> "$work/nolibrary.w"
		"$native" "$cgame" --frames 30 > /dev/null 2> "$work/nolibrary-start.n"
		box "$wbx" "$core" "$cgame" --frames 30 > /dev/null 2> "$work/nolibrary-start.w"
		out_n="$(grep -c -F "$said" "$work/nolibrary.n")"
		out_w="$(grep -c -F "$said" "$work/nolibrary.w")"
		start_n="$(grep -c -F "error.no_library" "$work/nolibrary-start.n")"
		start_w="$(grep -c -F "error.no_library" "$work/nolibrary-start.w")"
		# no panic text, no helper program, in any of the four
		bad="$(cat "$work"/nolibrary*.n "$work"/nolibrary*.w | grep -c -E "wow64|current_exe|/proc/self")"
		out_e=""
		run="${chimera_root:+$chimera_root/build/meson-linux/chimera-run}"
		pkg="${chimera_root:+$chimera_root/build/Cores/opengmk.chimeraCore}"
		if [ -n "$run" ] && [ -x "$run" ] && [ -f "$pkg" ]; then
			python3 -c 'import json,sys; n=len(json.load(open(sys.argv[1]))["input"]["buttons"]); print("\n".join("|%5d,%5d,%s|" % (4096, 4096, "." * n) for _ in range(30)))' \
				"$here/waterbox.config" > "$work/nolibrary.movie"
			rm -rf "$work/nolibrary.chimeraProject" "$work/nolibrary-start.chimeraProject"
			"$run" --new-project "$work/nolibrary.chimeraProject" "$pkg" "$lgame" "$work/nolibrary.movie" > "$work/nolibrary.make.log" 2>&1
			box "$run" --project "$work/nolibrary.chimeraProject" "$pkg" --files "$build/content" > "$work/nolibrary.out" 2>&1
			"$run" --new-project "$work/nolibrary-start.chimeraProject" "$pkg" "$cgame" "$work/nolibrary.movie" > "$work/nolibrary-start.make.log" 2>&1
			box "$run" --project "$work/nolibrary-start.chimeraProject" "$pkg" --files "$build/content" > "$work/nolibrary-start.out" 2>&1
			out_e="$(grep -c -F "It said: $said" "$work/nolibrary.out")/$(grep -c -F "$said" "$work/nolibrary-start.out")"
			bad=$((bad + $(cat "$work/nolibrary.out" "$work/nolibrary-start.out" | grep -c -E "wow64|current_exe|/proc/self|panicked")))
		fi
		ok=0
		[ "$out_n" -ge 1 ] && [ "$out_w" -ge 1 ] && [ "$start_n" -ge 1 ] && [ "$start_w" -ge 1 ] && [ "$bad" = 0 ] &&
			{ [ -z "$out_e" ] || [ "$out_e" = "1/1" ]; } && ok=1
		verdict nolibrary "$ok" "a game that calls a library the core cannot run gets a sentence naming it, not a panic" \
			"step: native $out_n, sandbox $out_w; start: native $start_n, sandbox $start_w; panic text $bad${out_e:+; chimera-run $out_e}"
		;;
	# ------------------------------------------------ a Windows library inside an extension
	extunused)
		# A game that carries a GameMaker extension holding a Windows library the
		# core cannot run (mystery.dll), and never calls it, starts and runs as
		# the game without the extension does: the same digests in the native
		# reference and the sandbox, and the same as the plain game's (the
		# extension changes nothing it holds), and the log says the function was
		# passed over. When the test is being broken the sandbox is given a
		# different route.
		egame="$build/content/extunused.exe"
		"$mkgame" "$egame" ext:mystery.dll:Foo > /dev/null
		"$native" "$egame" --frames 100 --digest-every 20 > "$work/extunused.n" 2> "$work/extunused.n.err"
		box "$wbx" "$core" "$egame" --frames 100 --digest-every 20 $(sabotage extunused) > "$work/extunused.w" 2> "$work/extunused.w.err"
		run_native --frames 100 --digest-every 20 > "$work/extunused.plain"
		n="$(digest "$work/extunused.n" | wc -l)"
		warned="$(grep -c -F "WARNING: extension function ext_Foo" "$work/extunused.n")"
		said="$(cat "$work"/extunused.*.err | grep -c -E "Windows library|panicked|wow64|current_exe")"
		ok=0
		[ "$n" = 6 ] && [ "$warned" -ge 1 ] && [ "$said" = 0 ] && diff <(digest "$work/extunused.n") <(digest "$work/extunused.w") > /dev/null &&
			diff <(digest "$work/extunused.n") <(digest "$work/extunused.plain") > /dev/null && ok=1
		verdict extunused "$ok" "a game with an extension holding a library the core cannot run, never calling it, starts and runs as before" \
			"$n digests; passed over with a warning: $warned; sentences or panics: $said"
		;;
	extcalled)
		# The same game, calling the library's function at its tenth step: the
		# machine stops with the sentence naming the library and the function
		# (the function's name in the library, not the one GML calls it by), in
		# the native reference, the sandbox and chimera-run, with no panic or
		# helper-program text. When the test is being broken the game does not
		# call it.
		said="This game calls a Windows library (mystery.dll, function Foo), which this core cannot run."
		cgame="$build/content/extcalled.exe"
		if [ "$breaking" = 1 ]; then "$mkgame" "$cgame" ext:mystery.dll:Foo > /dev/null; else "$mkgame" "$cgame" extcall:mystery.dll:Foo > /dev/null; fi
		"$native" "$cgame" --frames 60 > "$work/extcalled.n.out" 2> "$work/extcalled.n"
		box "$wbx" "$core" "$cgame" --frames 60 > "$work/extcalled.w.out" 2> "$work/extcalled.w"
		out_n="$(grep -c -F "$said" "$work/extcalled.n")"
		out_w="$(grep -c -F "$said" "$work/extcalled.w")"
		bad="$(cat "$work"/extcalled.n "$work"/extcalled.w | grep -c -E "wow64|current_exe|/proc/self|panicked")"
		out_e=""
		run="${chimera_root:+$chimera_root/build/meson-linux/chimera-run}"
		pkg="${chimera_root:+$chimera_root/build/Cores/opengmk.chimeraCore}"
		if [ -n "$run" ] && [ -x "$run" ] && [ -f "$pkg" ]; then
			python3 -c 'import json,sys; n=len(json.load(open(sys.argv[1]))["input"]["buttons"]); print("\n".join("|%5d,%5d,%s|" % (4096, 4096, "." * n) for _ in range(60)))' \
				"$here/waterbox.config" > "$work/extcalled.movie"
			rm -rf "$work/extcalled.chimeraProject"
			"$run" --new-project "$work/extcalled.chimeraProject" "$pkg" "$cgame" "$work/extcalled.movie" > "$work/extcalled.make.log" 2>&1
			box "$run" --project "$work/extcalled.chimeraProject" "$pkg" --files "$build/content" > "$work/extcalled.out" 2>&1
			out_e="$(grep -c -F "It said: $said" "$work/extcalled.out")"
			bad=$((bad + $(grep -c -E "wow64|current_exe|/proc/self|panicked" "$work/extcalled.out")))
		fi
		ok=0
		[ "$out_n" -ge 1 ] && [ "$out_w" -ge 1 ] && [ "$bad" = 0 ] && { [ -z "$out_e" ] || [ "$out_e" = 1 ]; } && ok=1
		verdict extcalled "$ok" "a call into a library the core cannot run, inside an extension, stops the game with a sentence naming it" \
			"native $out_n, sandbox $out_w; panic text $bad${out_e:+; chimera-run $out_e}"
		;;
	extstandin)
		# An extension whose library is one of OpenGMK's silent stand-ins
		# (gmfmodsimple.dll) is run on the stand-in: the game calls it, the answer
		# (1) shows in the picture, and the native reference, the sandbox and a
		# savestate before every step agree. When the test is being broken the
		# game does not call it.
		sgame="$build/content/extstandin.exe"
		if [ "$breaking" = 1 ]; then "$mkgame" "$sgame" ext:gmfmodsimple.dll:FMODSoundAdd > /dev/null; else "$mkgame" "$sgame" extcall:gmfmodsimple.dll:FMODSoundAdd > /dev/null; fi
		"$native" "$sgame" --frames 100 --digest-every 20 > "$work/extstandin.n" 2> "$work/extstandin.n.err"
		box "$wbx" "$core" "$sgame" --frames 100 --digest-every 20 > "$work/extstandin.w" 2> "$work/extstandin.w.err"
		box "$wbx" "$core" "$sgame" --frames 100 --digest-every 20 --rerecord > "$work/extstandin.r" 2> /dev/null
		run_native --frames 100 --digest-every 20 > "$work/extstandin.plain"
		a="$(digest "$work/extstandin.n" | tail -1 | awk '{print $3,$4}')"
		b="$(digest "$work/extstandin.plain" | tail -1 | awk '{print $3,$4}')"
		n="$(digest "$work/extstandin.n" | wc -l)"
		said="$(cat "$work"/extstandin.*.err | grep -c -E "Windows library|panicked")"
		ok=0
		[ "$n" = 6 ] && [ -n "$a" ] && [ "$a" != "$b" ] && [ "$said" = 0 ] && diff <(digest "$work/extstandin.n") <(digest "$work/extstandin.w") > /dev/null &&
			diff <(digest "$work/extstandin.n") <(digest "$work/extstandin.r") > /dev/null && ok=1
		verdict extstandin "$ok" "an extension whose library has a stand-in runs on it, the same in both and through savestates" \
			"$n digests; the picture shows the answer; sentences or panics: $said"
		;;
	# ------------------------------------------------ the package, through the engine
	engine)
		run="${chimera_root:+$chimera_root/build/meson-linux/chimera-run}"
		pkg="${chimera_root:+$chimera_root/build/Cores/opengmk.chimeraCore}"
		if [ "$breaking" = 1 ]; then continue; fi
		if [ -n "$run" ] && [ -x "$run" ] && [ -f "$pkg" ]; then
			python3 - "$work/route.movie" "$here" <<'PY'
import json, sys
path, here = sys.argv[1], sys.argv[2]
cfg = json.load(open(here + "/waterbox.config"))
inp = cfg["input"]
buttons, mn = inp["buttons"], inp["mnemonics"]
sched = {"Key Space": [(10, 13), (140, 141)], "Key Right": [(30, 80)], "Mouse Left Button": [(50, 120)]}
mouse = [(0, 4096, 4096), (50, 2000 * 8192 // 8192, 3000), (90, 6000, 1000)]
def held(name, f):
    return any(a <= f < b for a, b in sched.get(name, []))
def axes(f):
    cur = mouse[0]
    for m in mouse:
        if m[0] <= f:
            cur = m
    return cur[1], cur[2]
out = []
for f in range(300):
    ax, ay = axes(f)
    row = "".join((mn.get(b, "?")[:1] if held(b, f) else ".") for b in buttons)
    out.append("|%5d,%5d,%s|" % (ax, ay, row))
open(path, "w").write("\n".join(out) + "\n")
PY
			"$run" --new-project "$work/route.chimeraProject" "$pkg" "$game" "$work/route.movie" > "$work/route.make.log" 2>&1
			box "$run" --project "$work/route.chimeraProject" "$pkg" --files "$build/content" --screenshot 299="$work/route-engine.tga" > "$work/engine.out" 2>&1
			run_wbx --frames 300 "${route[@]}" --screenshot 299="$work/route-wbx.tga" > /dev/null
			a="$(ffmpeg -loglevel error -i "$work/route-engine.tga" -f rawvideo -pix_fmt rgb24 - 2>/dev/null | sha1sum)"
			b="$(ffmpeg -loglevel error -i "$work/route-wbx.tga" -f rawvideo -pix_fmt rgb24 - 2>/dev/null | sha1sum)"
			if [ "$a" = "$b" ] && [ -s "$work/route-engine.tga" ]; then
				report PASS "the engine: the package plays the route" "its picture is run-wbx's, byte for byte"
			else
				report FAIL "the engine: the package plays the route" "see $work/engine.out"
			fi
		else
			report SKIP "the engine" "no chimera checkout with chimera-run and the package (-r)"
		fi
		;;
	esac
done

echo
echo "test script: $pass passed, $fail failed, $skip skipped"
[ "$fail" = 0 ]
