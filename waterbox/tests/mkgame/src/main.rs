//! mkgame: writes the core's test game, a GameMaker 8.1 executable.
//!
//! usage: mkgame OUT.exe
//!
//! The container is what a GameMaker 8.1 runner makes (a Windows executable
//! with the game's data after the program, scrambled twice), without the
//! program: nothing here runs on Windows, the file only has to be one OpenGMK
//! reads. The game is one object in one room: a ball that bounces, is nudged
//! by the arrow keys, follows the left mouse button, picks its colours from
//! random(), draws text and shapes, and plays a tone when the space bar is
//! pressed. The tone is a sine wave made here.

use byteorder::{WriteBytesExt, LE};
use flate2::{write::ZlibEncoder, Compression};
use gm8exe::{
    asset::{
        code_action::PARAM_COUNT, object::Object, room, room::Room, sound::SoundFX, Asset, CodeAction, PascalString,
        sprite::{CollisionMap, Frame},
        Sound, SoundKind, Sprite,
    },
    GameVersion,
};
use std::io::Write;

const VERSION: GameVersion = GameVersion::GameMaker8_1;

fn pas(s: &str) -> PascalString {
    PascalString::from(s)
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(data).unwrap();
    z.finish().unwrap()
}

/// An "execute a piece of code" action of the GameMaker editor.
fn code(gml: &str) -> CodeAction {
    let mut param_strings: [PascalString; PARAM_COUNT] = Default::default();
    param_strings[0] = pas(gml);
    let mut param_types = [0u32; PARAM_COUNT];
    param_types[0] = 1;
    CodeAction {
        id: 603,
        applies_to: -1, // the instance itself
        is_condition: false,
        invert_condition: false,
        is_relative: false,
        lib_id: 1,
        action_kind: 7,
        execution_type: 2,
        can_be_relative: 0,
        applies_to_something: true,
        fn_name: pas(""),
        fn_code: pas(""),
        param_count: 1,
        param_types,
        param_strings,
    }
}

const CREATE: &str = r#"
randomize();
x = 160; y = 120; hs = 3; vs = 2;
score = 0; ang = 0; col = c_yellow;
px = 0; py = 0; presses = 0;
if (file_exists("save.ini")) { ini_open("save.ini"); score = ini_read_real("game", "score", 0); ini_close(); }
sound_play(snd_tone);
"#;

const STEP: &str = r#"
x += hs + (keyboard_check(vk_right) - keyboard_check(vk_left)) * 2;
y += vs + (keyboard_check(vk_down) - keyboard_check(vk_up)) * 2;
if (x < 12 || x > 308) hs = -hs;
if (y < 12 || y > 228) vs = -vs;
x = clamp(x, 12, 308); y = clamp(y, 12, 228);
ang += 4;
if (keyboard_check_pressed(vk_space)) {
    score += 10; presses += 1; sound_play(snd_tone);
    ini_open("save.ini"); ini_write_real("game", "score", score); ini_close();
    f = file_text_open_write("log.txt"); file_text_write_string(f, "presses " + string(presses)); file_text_close(f);
}
if (mouse_check_button(mb_left)) { px = mouse_x; py = mouse_y; }
col = make_color_hsv(random(255), 255, 255);
if (irandom(30) == 0) score += 1;
"#;

const DRAW: &str = r#"
draw_set_color(c_white);
draw_text(8, 8, "score " + string(score) + "  seed " + string(random_get_seed()));
draw_text(8, 24, "mouse " + string(mouse_x) + "," + string(mouse_y) + " ang " + string(sin(degtorad(ang))));
draw_text(8, 40, string(current_year) + "-" + string(current_month) + "-" + string(current_day) + " " + string(current_hour) + ":" + string(current_minute) + ":" + string(current_second));
draw_set_color(col);
draw_circle(x, y, 8 + 4 * sin(degtorad(ang)), false);
draw_set_color(c_aqua);
draw_line(160, 120, x, y);
draw_rectangle(px - 4, py - 4, px + 4, py + 4, true);
draw_set_color(c_red);
draw_triangle(300, 200, 310, 230, 290, 230, false);
draw_set_color(c_white);
"#;

fn tone() -> Vec<u8> {
    // 440 Hz, 0.4 s, 16-bit mono at 22050 Hz, as a WAV file
    let rate = 22050u32;
    let n = rate * 4 / 10;
    let mut pcm = Vec::new();
    for i in 0..n {
        let t = i as f64 / rate as f64;
        let env = 1.0 - i as f64 / n as f64;
        let s = (t * 440.0 * std::f64::consts::TAU).sin() * env * 0.5;
        pcm.write_i16::<LE>((s * 32767.0) as i16).unwrap();
    }
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.write_u32::<LE>(36 + pcm.len() as u32).unwrap();
    w.extend_from_slice(b"WAVEfmt ");
    w.write_u32::<LE>(16).unwrap();
    w.write_u16::<LE>(1).unwrap(); // PCM
    w.write_u16::<LE>(1).unwrap(); // mono
    w.write_u32::<LE>(rate).unwrap();
    w.write_u32::<LE>(rate * 2).unwrap();
    w.write_u16::<LE>(2).unwrap();
    w.write_u16::<LE>(16).unwrap();
    w.extend_from_slice(b"data");
    w.write_u32::<LE>(pcm.len() as u32).unwrap();
    w.extend_from_slice(&pcm);
    w
}

/// One asset as the data file holds it: a flag that it exists, then its
/// bytes, deflated.
fn chunk(asset: &impl Asset) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.write_u32::<LE>(1).unwrap();
    asset.serialize_exe(&mut raw, VERSION).unwrap();
    zlib(&raw)
}

fn list(out: &mut Vec<u8>, header: u32, chunks: &[Vec<u8>]) {
    out.write_u32::<LE>(header).unwrap();
    out.write_u32::<LE>(chunks.len() as u32).unwrap();
    for c in chunks {
        out.write_u32::<LE>(c.len() as u32).unwrap();
        out.extend_from_slice(c);
    }
}

fn settings() -> Vec<u8> {
    let mut s = Vec::new();
    let mut put = |v: u32| s.write_u32::<LE>(v).unwrap();
    put(0); // fullscreen
    put(0); // interpolate pixels
    put(0); // no border
    put(1); // cursor
    put(100); // scaling: fixed, 100%
    put(0); // resizable
    put(0); // on top
    put(0x00404040); // outside colour
    put(0); // set resolution
    put(0); // colour depth
    put(0); // resolution
    put(0); // frequency
    put(0); // no buttons
    put(0); // vsync flags
    put(0); // screensaver
    put(0); // f4
    put(0); // f1
    put(1); // esc ends the game
    put(0); // f5/f6
    put(0); // f9
    put(0); // close as esc
    put(0); // priority
    put(0); // freeze
    put(0); // loading bar: none
    put(0); // custom load image: none
    put(0); // transparent
    put(255); // translucency
    put(0); // scale progress bar
    put(1); // show errors
    put(0); // log errors
    put(0); // always abort
    put(1); // zero uninitialised variables
    put(0); // (webgl)
    put(0); // swap creation events
    s
}

fn help() -> Vec<u8> {
    let mut h = Vec::new();
    h.write_u32::<LE>(0xFFFFFF).unwrap();
    h.write_u32::<LE>(0).unwrap();
    h.write_u32::<LE>(0).unwrap();
    h.extend_from_slice(b"");
    // caption (empty pascal string), then position, size, flags, info text
    for v in [0i32, 0, 0, 0, 0, 0, 0, 0] {
        h.write_i32::<LE>(v).unwrap();
    }
    h.write_u32::<LE>(0).unwrap();
    zlib(&h)
}

/// The part of the data GameMaker scrambles with a table of its own.
/// GML that defines `func` of the Windows library `lib` (cdecl, one number in, a
/// number out), calls it and adds what it answered, times 1000, to the score.
fn library_call(lib: &str, func: &str, when: &str) -> String {
    format!("{when}{{ lib_id = external_define(\"{lib}\", \"{func}\", 0, 0, 1, 0); score += 1000 * external_call(lib_id, 1); }}\n")
}

/// A length-prefixed string, as the data file holds one.
fn put_str(out: &mut Vec<u8>, s: &str) {
    out.write_u32::<LE>(s.len() as u32).unwrap();
    out.extend_from_slice(s.as_bytes());
}

/// The extensions list of the data file: one extension holding one Windows
/// library (`lib`, its contents are not a real library, nothing here ever loads
/// it) that declares one function, `func`, which GML calls `ext_<func>` (cdecl,
/// one number in, a number out). The inverse of gm8exe's `Extension::read`.
fn extensions(lib: &str, func: &str) -> Vec<u8> {
    let mut e = Vec::new();
    e.write_u32::<LE>(700).unwrap(); // the extension
    put_str(&mut e, "ext_test");
    put_str(&mut e, "ext_test");
    e.write_u32::<LE>(1).unwrap(); // files
    e.write_u32::<LE>(700).unwrap(); // the library
    put_str(&mut e, lib);
    e.write_u32::<LE>(1).unwrap(); // kind: a dynamic library
    put_str(&mut e, ""); // no initializer
    put_str(&mut e, ""); // no finalizer
    e.write_u32::<LE>(1).unwrap(); // functions
    e.write_u32::<LE>(700).unwrap();
    put_str(&mut e, &format!("ext_{func}"));
    put_str(&mut e, func);
    e.write_u32::<LE>(12).unwrap(); // cdecl
    e.write_u32::<LE>(0).unwrap(); // id
    e.write_i32::<LE>(1).unwrap(); // one argument
    for _ in 0..17 {
        e.write_u32::<LE>(2).unwrap(); // a number
    }
    e.write_u32::<LE>(2).unwrap(); // returns a number
    e.write_u32::<LE>(0).unwrap(); // no constants
    // the contents: the file deflated behind its length, scrambled with the table
    // of seed 0 (the reader's: all but the first byte go through the table)
    let (seed1, seed2) = (0u32, 6u32);
    let mut table = [0u8; 0x200];
    for (i, v) in table.iter_mut().enumerate() {
        *v = (i % 256) as u8;
    }
    for i in 1..0x2711u32 {
        let idx = (((i * seed2 + seed1) % 0xFE) + 1) as usize;
        table.swap(idx, idx + 1);
    }
    let mut encrypt = [0u8; 256]; // plain byte -> scrambled byte
    for i in 0..0x100usize {
        encrypt[(i as u8).wrapping_add(1) as usize] = table[i + 1];
    }
    let z = zlib(b"not a Windows library: the core never loads this");
    let mut data = Vec::new();
    data.write_u32::<LE>(z.len() as u32).unwrap();
    data.extend_from_slice(&z);
    for b in data.iter_mut().skip(1) {
        *b = encrypt[*b as usize];
    }
    e.write_u32::<LE>(data.len() as u32 + 4).unwrap();
    e.write_u32::<LE>(0).unwrap(); // seed 0
    e.extend_from_slice(&data);
    let mut d = Vec::new();
    d.write_u32::<LE>(700).unwrap();
    d.write_u32::<LE>(1).unwrap();
    d.extend_from_slice(&e);
    d
}

fn data_block(calls: &Option<String>) -> Vec<u8> {
    let mut d = Vec::new();
    d.write_u32::<LE>(0).unwrap(); // no garbage words
    d.write_u32::<LE>(1).unwrap(); // pro
    d.write_u32::<LE>(424242).unwrap(); // game id
    for g in [1u32, 2, 3, 4] {
        d.write_u32::<LE>(g).unwrap();
    }
    let call = calls.as_deref().map(|c| c.splitn(3, ':').collect::<Vec<_>>());
    match call.as_deref() {
        Some(["ext" | "extcall", lib, func]) => d.extend_from_slice(&extensions(lib, func)),
        _ => {
            d.write_u32::<LE>(700).unwrap(); // extensions: none
            d.write_u32::<LE>(0).unwrap();
        }
    }
    list(&mut d, 800, &[]); // triggers
    d.write_u32::<LE>(800).unwrap(); // constants
    d.write_u32::<LE>(0).unwrap();
    let snd = Sound {
        name: pas("snd_tone"),
        source: pas("tone.wav"),
        extension: pas(".wav"),
        data: Some(tone().into_boxed_slice()),
        kind: SoundKind::Normal,
        volume: 1.0,
        pan: 0.0,
        preload: true,
        fx: SoundFX { chorus: false, echo: false, flanger: false, gargle: false, reverb: false },
    };
    list(&mut d, 800, &[chunk(&snd)]);
    // "bigpics": six sprites of 4096 x 4096 pixels (64 MiB each, 384 MiB in all),
    // the pictures of a game that has a lot of them; they do not fit in one
    // 8192 x 8192 texture, so the game needs one of 16384 x 8192 (512 MiB)
    let sprites: Vec<Vec<u8>> = if let Some(["bigpics"]) = call.as_deref() {
        (0..6)
            .map(|i| {
                chunk(&Sprite {
                    name: pas(&format!("spr_big{i}")),
                    origin_x: 0,
                    origin_y: 0,
                    frames: vec![Frame { width: 4096, height: 4096, data: vec![0x80u8; 4096 * 4096 * 4].into_boxed_slice() }],
                    colliders: vec![CollisionMap {
                        width: 1,
                        height: 1,
                        bbox_left: 0,
                        bbox_right: 0,
                        bbox_top: 0,
                        bbox_bottom: 0,
                        data: vec![true].into_boxed_slice(),
                    }],
                    per_frame_colliders: false,
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    list(&mut d, 800, &sprites);
    list(&mut d, 800, &[]); // backgrounds
    list(&mut d, 800, &[]); // paths
    list(&mut d, 800, &[]); // scripts
    list(&mut d, 800, &[]); // fonts
    list(&mut d, 800, &[]); // timelines
    let mut events: Vec<Vec<(u32, Vec<CodeAction>)>> = (0..12).map(|_| Vec::new()).collect();
    let (mut create, mut step) = (CREATE.to_string(), STEP.to_string());
    match call.as_deref() {
        Some(["extcall", _, func]) => step += &format!("if (ang == 40) score += 1000 * ext_{func}(1);\n"),
        Some(["dll", lib, func]) => step += &library_call(lib, func, "if (ang == 40) "),
        Some(["dllc", lib, func]) => create += &library_call(lib, func, ""),
        Some(["bigpics"]) => {}
        Some([name]) => step += &format!("if (ang == 40) {name}(\"hello\");\n"),
        _ => {}
    }
    events[0] = vec![(0, vec![code(&create)])];
    events[3] = vec![(0, vec![code(&step)])];
    events[8] = vec![(0, vec![code(DRAW)])];
    let obj = Object {
        name: pas("obj_ball"),
        sprite_index: -1,
        solid: false,
        visible: true,
        depth: 0,
        persistent: false,
        parent_index: -1,
        mask_index: -1,
        events,
    };
    list(&mut d, 800, &[chunk(&obj)]);
    let room = Room {
        name: pas("room0"),
        caption: pas("OpenGMK test game"),
        width: 320,
        height: 240,
        speed: 30,
        persistent: false,
        bg_colour: gm8exe::Colour::new(40, 60, 120, 0),
        clear_screen: true,
        clear_region: true,
        creation_code: pas(""),
        backgrounds: Vec::new(),
        views_enabled: false,
        views: Vec::new(),
        instances: vec![room::Instance {
            x: 0,
            y: 0,
            object: 0,
            id: 100001,
            creation_code: pas(""),
            xscale: 1.0,
            yscale: 1.0,
            blend: 0xFFFFFFFF,
            angle: 0.0,
        }],
        tiles: Vec::new(),
        uses_810_features: false,
        uses_811_features: false,
    };
    list(&mut d, 800, &[chunk(&room)]);
    d.write_i32::<LE>(100001).unwrap(); // last instance id
    d.write_i32::<LE>(10000000).unwrap(); // last tile id
    list(&mut d, 800, &[]); // included files
    d.write_u32::<LE>(800).unwrap(); // help dialog
    let h = help();
    d.write_u32::<LE>(h.len() as u32).unwrap();
    d.extend_from_slice(&h);
    d.write_u32::<LE>(500).unwrap(); // action library initialisation code
    d.write_u32::<LE>(0).unwrap();
    d.write_u32::<LE>(700).unwrap(); // room order
    d.write_u32::<LE>(1).unwrap();
    d.write_i32::<LE>(0).unwrap();
    d
}

/// The inverse of gm8exe's `gm80::decrypt`: scrambles `data` with `table`.
fn scramble(data: &[u8], table: &[u8; 256]) -> Vec<u8> {
    let len = data.len();
    let mut x = data.to_vec();
    // undo the second pass: the same swaps, in the other order
    for i in 0..len {
        let b = (i as i64 - table[i & 0xFF] as i64).max(0) as usize;
        x.swap(i, b);
    }
    // undo the first pass
    let mut e = vec![0u8; len];
    for j in 0..len {
        if j == 0 {
            e[0] = x[0];
        } else {
            let v = x[j].wrapping_add(e[j - 1]).wrapping_add(j as u8);
            e[j] = table[v as usize];
        }
    }
    e
}

fn gm81_xor(buf: &mut [u8], seed1: u32, seed2: u32) {
    let (mut s1, mut s2) = (seed1, seed2);
    for chunk in buf.chunks_exact_mut(4) {
        s1 = (0xFFFF & s1).wrapping_mul(0x9069).wrapping_add(s1 >> 16);
        s2 = (0xFFFF & s2).wrapping_mul(0x4650).wrapping_add(s2 >> 16);
        let mask = (s1 << 16).wrapping_add(s2 & 0xFFFF);
        let v = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) ^ mask;
        chunk.copy_from_slice(&v.to_le_bytes());
    }
}

/// GameMaker 8.1's own checksum of the key text: CRC-32 of the text as
/// UTF-16, without the final inversion.
fn seed2_of(key: u32) -> u32 {
    let text: Vec<u8> = format!("_MJD{key}#RWK").bytes().flat_map(|c| [c, 0]).collect();
    let mut h = crc32_table_free(&text);
    h ^= 0xFFFF_FFFF;
    h
}
fn crc32_table_free(data: &[u8]) -> u32 {
    // standard reflected CRC-32 with the final xor left to the caller
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("usage: mkgame OUT.exe [FUNCTION]");
    // A second argument names a GameMaker function the game calls at its
    // tenth step, to test what the core says about one OpenGMK has not
    // implemented (show_message, for one). "dll:LIBRARY:FUNCTION" makes the game
    // define that function of a Windows library and call it at its tenth step
    // (its answer is added to the score, so the picture shows it); "dllc:..."
    // does the same in the first event, before the game has started.
    // "ext:LIBRARY:FUNCTION" makes the game carry a GameMaker extension that holds
    // that library and function and never calls it; "extcall:..." calls it at the
    // tenth step too (the answer is added to the score as for "dll").
    // "bigpics" gives the game six very large sprites (see data_block).
    let calls = args.next();
    let mut exe = vec![0u8; 3_800_004];
    // MZ, and a PE header with one section
    exe[0] = b'M';
    exe[1] = b'Z';
    let pe = 0x80usize;
    exe[0x3C..0x40].copy_from_slice(&(pe as u32).to_le_bytes());
    exe[pe..pe + 6].copy_from_slice(b"PE\0\0\x4C\x01");
    exe[pe + 6..pe + 8].copy_from_slice(&1u16.to_le_bytes());
    let optional = 0xE0u16;
    exe[pe + 20..pe + 22].copy_from_slice(&optional.to_le_bytes());
    let section = pe + 24 + optional as usize;
    exe[section..section + 5].copy_from_slice(b".text");
    exe[section + 8..section + 12].copy_from_slice(&0x1000u32.to_le_bytes());
    exe[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
    exe[section + 16..section + 20].copy_from_slice(&0x1000u32.to_le_bytes());
    exe[section + 20..section + 24].copy_from_slice(&0x400u32.to_le_bytes());

    // the key: any whose checksum puts the encryption's start within 10 bytes
    let mut key = 0u32;
    let seed2 = loop {
        let s = seed2_of(key);
        if s & 0xFF <= 10 {
            break s;
        }
        key += 1;
    };
    let pad = (seed2 & 0xFF) as usize;
    let seed1 = 0x1234_5678u32;

    // the magic: 0xF7140067 spread over two words
    let value = 0xF714_0067u32;
    let xor = 0u32;
    let d1 = (value & 0xFF00_FF00) | (xor & 0x00FF_00FF);
    let d2 = (value & 0x00FF_00FF) | (xor & 0xFF00_FF00);
    exe.extend_from_slice(&d1.to_le_bytes());
    exe.extend_from_slice(&d2.to_le_bytes());
    // from here (P): key, seed1, then 20 bytes the reader skips, then the settings
    let p = exe.len();
    exe.extend_from_slice(&key.to_le_bytes());
    exe.extend_from_slice(&seed1.to_le_bytes());
    exe.extend_from_slice(&[0u8; 20]);
    let mut tail = Vec::new();
    let s = zlib(&settings());
    tail.write_u32::<LE>(s.len() as u32).unwrap();
    tail.extend_from_slice(&s);
    tail.write_u32::<LE>(0).unwrap(); // the DirectX library's name: empty
    tail.write_u32::<LE>(0).unwrap(); // and its data
    // the table-scrambled block
    let table: [u8; 256] = {
        let mut t = [0u8; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = (i as u8).wrapping_mul(7).wrapping_add(13); // a permutation: 7 is odd
        }
        t
    };
    tail.write_u32::<LE>(0).unwrap(); // no garbage before the table
    tail.write_u32::<LE>(0).unwrap(); // and none after
    tail.extend_from_slice(&table);
    let block = data_block(&calls);
    tail.write_u32::<LE>(block.len() as u32).unwrap();
    tail.extend_from_slice(&scramble(&block, &table));
    while tail.len() % 4 != 0 {
        tail.push(0);
    }
    exe.extend_from_slice(&tail);
    // the encryption covers everything from `pad` bytes past the skipped part
    let start = p + 8 + pad + 10;
    let region = &mut exe[start..];
    let n = region.len() / 4 * 4;
    gm81_xor(&mut region[..n], seed1, seed2);
    std::fs::write(&out, &exe).unwrap();
    println!("wrote {out}: {} bytes", exe.len());
}
