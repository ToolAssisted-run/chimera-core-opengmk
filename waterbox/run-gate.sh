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

tests=(equal repeat stall rerecord session input settings clock sound save pokes refusal unimplemented engine)
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
