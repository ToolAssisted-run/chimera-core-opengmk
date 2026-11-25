//! The OpenGMK core: the exports the Chimera engine calls, over OpenGMK's
//! GameMaker 8 runner (extern/OpenGMK, patched: no window, no sound device,
//! Mesa's softpipe for the picture) driven one step of the game at a time.
//!
//! The same code is the native reference: waterbox/run-native links this crate
//! for the host and calls these functions in the order the engine does, so the
//! two flavors differ only in where they run.

#![allow(non_snake_case)]
#![allow(clippy::missing_safety_doc)]

pub mod input_table;
pub mod mathlib;

use gm8emulator::{game::GameClock, Game, PlayType};
use input_table::{Btn, BUTTONS, BUTTON_COUNT};
use serde_json::Value;
use std::ffi::CString;
use std::path::PathBuf;

/// The sound is mixed at this rate, in stereo (OpenGMK's mixer, a step at a time).
pub const AUDIO_RATE: u32 = gm8emulator::game::audio::MIX_RATE;
/// The most one step can hold: a game at 1 step a second.
pub const AUDIO_MAX_FRAMES: usize = AUDIO_RATE as usize;

/// The game's own numbers in one block of the core's memory, laid out as
/// the table GetGameProperties describes: what RAM Watch, RAM Search and the
/// Hex Editor see as the domain "Game State". Copied from the game after
/// every step; a value a tool changes is copied back before the next.
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq)]
struct State {
    steps: u64,
    room: i32,
    room_speed: i32,
    score: i32,
    lives: i32,
    health: f64,
    random_seed: i32,
    instances: i32,
    mouse_x: i32,
    mouse_y: i32,
    _pad: [i32; 3],
}
static mut STATE: State = State {
    steps: 0,
    room: 0,
    room_speed: 0,
    score: 0,
    lives: 0,
    health: 0.0,
    random_seed: 0,
    instances: 0,
    mouse_x: 0,
    mouse_y: 0,
    _pad: [0; 3],
};
/// the block as the game last made it
static mut PUBLISHED: State = State {
    steps: 0,
    room: 0,
    room_speed: 0,
    score: 0,
    lives: 0,
    health: 0.0,
    random_seed: 0,
    instances: 0,
    mouse_x: 0,
    mouse_y: 0,
    _pad: [0; 3],
};

#[allow(static_mut_refs)]
fn state() -> &'static mut State {
    unsafe { &mut STATE }
}

/// What a tool wrote into the block since the game made it, into the game.
fn state_to_game(game: &mut Game) {
    #[allow(static_mut_refs)]
    let (now, then) = unsafe { (STATE, PUBLISHED) };
    if now.score != then.score {
        game.score = now.score;
    }
    if now.lives != then.lives {
        game.lives = now.lives;
    }
    if now.health != then.health {
        game.health = now.health.into();
    }
    if now.random_seed != then.random_seed {
        game.rand.set_seed(now.random_seed);
    }
}

/// What the game has now, into the block.
fn game_to_state(game: &Game, steps: u64) {
    let s = State {
        steps,
        room: game.room.id,
        room_speed: game.room.speed as i32,
        score: game.score,
        lives: game.lives,
        health: game.health.into(),
        random_seed: game.rand.seed(),
        instances: game.room.instance_list.count_all() as i32,
        mouse_x: game.get_mouse_in_room().0,
        mouse_y: game.get_mouse_in_room().1,
        _pad: [0; 3],
    };
    unsafe {
        STATE = s;
        PUBLISHED = s;
    }
}

struct Core {
    /// Why Init refused, NUL-terminated.
    load_error: Vec<u8>,
    game: Option<Box<Game>>,
    running: bool,
    frame: u64,
    /// held now, and as the game was last told, one bit per button
    buttons: [u64; 2],
    held: [u64; 2],
    axes: [i32; 2],
    /// the picture of the last step, BGRA
    video: Vec<u8>,
    video_w: u32,
    video_h: u32,
    /// the last step's sound, interleaved stereo
    audio: Vec<i16>,
    /// samples owed to the next step: the step's length is not always a whole
    /// number of samples
    audio_fraction: u64,
    /// the steps' rate, now
    speed: u32,
    /// the save data listed by the last GetSaveDataFileCount, names
    /// NUL-terminated
    saves: Vec<(Vec<u8>, Vec<u8>)>,
}

static mut CORE: Core = Core {
    load_error: Vec::new(),
    game: None,
    running: false,
    frame: 0,
    buttons: [0; 2],
    held: [0; 2],
    axes: [4096, 4096],
    video: Vec::new(),
    video_w: 0,
    video_h: 0,
    audio: Vec::new(),
    audio_fraction: 0,
    speed: 30,
    saves: Vec::new(),
};

#[allow(static_mut_refs)]
fn core() -> &'static mut Core {
    unsafe { &mut CORE }
}

/// A text a person reads: a key of languages/en.json, each of its arguments
/// after a 0x1F byte (chimera docs/translations.md).
fn say(key: &str, args: &[&str]) -> String {
    let mut out = key.to_string();
    for arg in args {
        out.push('\u{1f}');
        out.push_str(arg);
    }
    out
}

/// The English of a text of languages/en.json, its %1 filled in. The sandbox's
/// last words for a stop are one plain line of English: the window shows what
/// the machine said as it is, and does not look it up in the package's tables.
fn english(key: &str, args: &[&str]) -> String {
    let table: Value = serde_json::from_str(include_str!("../../languages/en.json")).unwrap_or(Value::Null);
    match table["texts"][key].as_str() {
        Some(text) => {
            let mut text = text.to_string();
            for (i, arg) in args.iter().enumerate() {
                text = text.replace(&format!("%{}", i + 1), arg);
            }
            text
        }
        None => format!("{key} {}", args.join(" ")),
    }
}

/// OpenGMK does not implement every GameMaker function: where it has none it
/// panics with "Called unimplemented kernel function <name>". The panic's own
/// text (a source file and a line of Rust) means nothing to a player, so the
/// hook says what happened in one sentence naming the function. The machine
/// then stops as any panic stops it. Any other panic keeps its own text.
fn say_unimplemented_functions() {
    const MARK: &str = "Called unimplemented kernel function ";
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let text = info.to_string();
        if let Some((library, function)) = missing_library(&text) {
            eprintln!("{}", english("error.no_library", &[&library, &function]));
        } else if let Some(at) = text.find(MARK) {
            let name: String =
                text[at + MARK.len()..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            eprintln!("{}", english("error.unimplemented", &[&name]));
        } else {
            previous(info);
        }
    }));
}

/// What OpenGMK (patch 0008) says when a game asks for a Windows library the core
/// cannot run: this marker, then the library and the function, each after a
/// 0x1F byte. None for any other text.
const NO_LIBRARY: &str = "chimera-no-library";

fn missing_library(text: &str) -> Option<(String, String)> {
    let at = text.find(NO_LIBRARY)? + NO_LIBRARY.len();
    let mut parts = text[at..].split('\u{1f}').skip(1);
    Some((parts.next()?.to_string(), parts.next()?.to_string()))
}

/// Why a game could not start, as a text of the tables.
fn launch_error(e: &dyn std::fmt::Display) -> String {
    let text = e.to_string();
    match missing_library(&text) {
        Some((library, function)) => say("error.no_library", &[&library, &function]),
        None => say("error.launch", &[&text]),
    }
}

fn fail(why: String) -> i32 {
    eprintln!("opengmk core: {why}");
    core().load_error = CString::new(why.replace('\0', " ")).unwrap().into_bytes_with_nul();
    0
}

// ---- Settings -----------------------------------------------------------------

/// The settings the engine mounts ("settings", a JSON object, every declared
/// setting present).
pub struct Settings {
    /// Seconds since the Unix epoch when the game starts: the date GameMaker's
    /// date functions answer from.
    pub clock_start: u64,
    /// The first of the numbers `randomize()` draws from, and the seed a game
    /// that never sets one starts with.
    pub seed: u64,
}

impl Settings {
    pub fn parse(json: &str) -> Result<Settings, String> {
        let mut s = Settings { clock_start: 1_262_304_000, seed: 0 }; // 2010-01-01
        let root: Value = if json.trim().is_empty() {
            Value::Object(Default::default())
        } else {
            serde_json::from_str(json).map_err(|e| say("error.settings_not_json", &[&e.to_string()]))?
        };
        let Value::Object(map) = root else {
            return Err(say("error.settings_not_object", &[]));
        };
        for (key, value) in &map {
            let text = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            match key.as_str() {
                "startTime" => {
                    s.clock_start = text.parse::<u64>().map_err(|_| say("error.bad_clock", &[&text]))?
                }
                "randomSeed" => s.seed = text.parse::<u64>().map_err(|_| say("error.bad_seed", &[&text]))?,
                _ => {} // a setting this build does not know is not an error
            }
        }
        Ok(s)
    }
}

// ---- Starting ------------------------------------------------------------------

/// The files of a save-data zip (what Export Save Data writes): the files the
/// game wrote, by the names it gave them.
pub fn read_save_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| say("error.save_not_zip", &[&e.to_string()]))?;
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| say("error.save_not_zip", &[&e.to_string()]))?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().to_string();
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| say("error.save_entry", &[&name, &e.to_string()]))?;
        out.push((name, data));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Make the game from a GameMaker 8 executable. Nothing runs until the first
/// step.
pub fn start(exe: Vec<u8>, name: &str, settings: &Settings) -> Result<(), String> {
    gm8emulator::gml::rand::set_machine_seed(settings.seed);
    let mut file = exe;
    // threads: none (the machine is one thread)
    let assets = gm8exe::reader::from_exe(&mut file, None::<fn(&str)>, false, false)
        .map_err(|e| say("error.bad_game", &[&e.to_string()]))?;
    drop(file);
    // the game's place: one folder, which a machine does not change
    let path = PathBuf::from("/").join(name);
    let mut game = Game::launch(
        assets,
        path,
        vec![name.to_string()],
        Some(PathBuf::new()),
        encoding_rs::SHIFT_JIS,
        false,
        0,
        false,
        PlayType::Normal,
    )
    .map_err(|e| launch_error(&e))?;
    game.clock = GameClock::SpoofedNanos(settings.clock_start as u128 * 1_000_000_000);
    game.chimera_start().map_err(|e| launch_error(&e))?;
    let c = core();
    c.speed = game.room.speed.max(1);
    game_to_state(&game, 0);
    c.game = Some(Box::new(game));
    c.running = true;
    Ok(())
}

fn read_mount(name: &str) -> Result<Vec<u8>, String> {
    std::fs::read(name).map_err(|e| say("error.not_read", &[name, &e.to_string()]))
}

/// The first file of slot `id` in the project's slot map ("slots": a JSON
/// object of slot id to file names, each file mounted under its own name).
/// None without a project or without that slot.
fn slot_first(id: &str) -> Option<String> {
    let text = std::fs::read("slots").ok()?;
    let map: Value = serde_json::from_slice(&text).ok()?;
    map.get(id)?.get(0)?.as_str().map(String::from)
}

#[no_mangle]
pub extern "C" fn Init() -> i32 {
    say_unimplemented_functions();
    let settings = match read_mount("settings")
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .and_then(|s| Settings::parse(&s))
    {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    // The game: a project names it in its "game" slot; a file opened on its
    // own arrives with its name in rom.name (and mounted under it too).
    let name = match slot_first("game") {
        Some(n) => n,
        None => match read_mount("rom.name") {
            Ok(n) => String::from_utf8_lossy(&n).trim().to_string(),
            Err(_) => return fail(say("error.no_file", &[])),
        },
    };
    let base = name.rsplit('/').next().unwrap_or(&name).to_string();
    let path = if std::path::Path::new(&base).is_file() { base } else { name };
    let exe = match read_mount(&path) {
        Ok(b) => b,
        Err(e) => return fail(e),
    };
    let short = path.rsplit('/').next().unwrap_or(&path).to_string();
    // What the game saved before: the .zip Export Save Data writes, put where
    // the game finds its files.
    if let Some(zip_name) = slot_first("savedata") {
        let zip_base = zip_name.rsplit('/').next().unwrap_or(&zip_name).to_string();
        match read_mount(&zip_base).and_then(|b| read_save_zip(&b)) {
            Ok(files) => {
                for (name, data) in files {
                    gm8emulator::memfs::put(&name, data);
                }
            }
            Err(e) => return fail(e),
        }
    }
    match start(exe, &short, &settings) {
        Ok(()) => 1,
        Err(e) => fail(e),
    }
}

#[no_mangle]
pub extern "C" fn GetLoadError() -> *const u8 {
    let e = &core().load_error;
    if e.is_empty() {
        b"\0".as_ptr()
    } else {
        e.as_ptr()
    }
}

// ---- A step -----------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn SetButton(index: i32, state: i32) {
    if (0..BUTTON_COUNT as i32).contains(&index) {
        let (word, bit) = (index as usize / 64, 1u64 << (index as usize % 64));
        let c = core();
        c.buttons[word] = if state != 0 { c.buttons[word] | bit } else { c.buttons[word] & !bit };
    }
}

#[no_mangle]
pub extern "C" fn SetAxis(index: i32, value: i32) {
    if let Some(axis) = core().axes.get_mut(index as usize) {
        *axis = value;
    }
}

/// Tells the game what changed since its last step: the keys and mouse buttons
/// that went down or up, and where the pointer is.
fn apply_input(input: &mut gm8emulator::input::Input, buttons: [u64; 2], held: [u64; 2], axes: [i32; 2], size: (u32, u32)) {
    for (i, btn) in BUTTONS.iter().enumerate() {
        let (word, bit) = (i / 64, 1u64 << (i % 64));
        let (now, before) = (buttons[word] & bit != 0, held[word] & bit != 0);
        if now == before {
            continue;
        }
        match (*btn, now) {
            (Btn::Vk(vk), true) => input.button_press(vk, true),
            (Btn::Vk(vk), false) => input.button_release(vk, true),
            (Btn::Mouse(b), true) => input.mouse_press(b, true),
            (Btn::Mouse(b), false) => input.mouse_release(b, true),
        }
    }
    // the pointer: 0..8191 across the window, as the config declares
    let x = (axes[0].clamp(0, 8191) as i64 * size.0 as i64 / 8192) as i32;
    let y = (axes[1].clamp(0, 8191) as i64 * size.1 as i64 / 8192) as i32;
    input.mouse_move_to((x, y));
}

/// One step. The controls were set by SetButton and SetAxis (there are more
/// than 64 buttons, so the argument cannot carry them).
#[no_mangle]
pub extern "C" fn FrameAdvance(_buttons: u64) {
    let c = core();
    if !c.running {
        return;
    }
    let Some(game) = c.game.as_mut() else { return };
    state_to_game(game);
    let (buttons, held, axes) = (c.buttons, c.held, c.axes);
    let size = game.window_inner_size;
    let result = game.chimera_step(|input| apply_input(input, buttons, held, axes, size));
    c.held = c.buttons;
    c.frame += 1;
    match result {
        Ok(true) => {}
        Ok(false) => {
            c.running = false;
            eprintln!("opengmk core: the game ended");
        }
        Err(e) => {
            c.running = false;
            let text = e.to_string();
            if missing_library(&text).is_some() {
                // a game that asks for a library the core cannot run stops the machine with a
                // sentence naming it, as a function OpenGMK lacks does (the hook says it)
                panic!("{text}");
            }
            eprintln!("opengmk core: the game stopped: {e}");
        }
    }
    c.speed = game.room.speed.max(1);
    game_to_state(game, c.frame);
    capture_picture(c);
    capture_sound(c);
}

fn capture_picture(c: &mut Core) {
    let (rgba, w, h) = gm8emulator::render::osmesa_screen();
    c.video.resize(rgba.len(), 0);
    for (dst, src) in c.video.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = 0xff;
    }
    c.video_w = w;
    c.video_h = h;
}

fn capture_sound(c: &mut Core) {
    // this step's share of a second of samples, with the remainder carried
    let total = c.audio_fraction + AUDIO_RATE as u64;
    let frames = (total / c.speed as u64) as usize;
    c.audio_fraction = total % c.speed as u64;
    let frames = frames.min(AUDIO_MAX_FRAMES);
    let mut mix = vec![0f32; frames * 2];
    if let Some(game) = c.game.as_mut() {
        game.audio.mix(&mut mix);
    }
    c.audio.clear();
    c.audio.extend(mix.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16));
}

#[no_mangle]
pub extern "C" fn IsRunning() -> i32 {
    core().running as i32
}

#[no_mangle]
pub extern "C" fn GetFrameCount() -> u64 {
    core().frame
}

/// The game's own clock, nanoseconds: the date it was started at and the
/// length of every step since.
#[no_mangle]
pub extern "C" fn GetMachineTimeNs() -> u64 {
    match core().game.as_ref().map(|g| &g.clock) {
        Some(GameClock::SpoofedNanos(t)) => *t as u64,
        _ => 0,
    }
}

// ---- What the step produced --------------------------------------------------------

#[no_mangle]
pub extern "C" fn GetVideoBgra() -> *const u8 {
    let c = core();
    if c.video.is_empty() {
        BLACK.as_ptr()
    } else {
        c.video.as_ptr()
    }
}
/// Before the first picture.
static BLACK: [u8; 640 * 480 * 4] = [0; 640 * 480 * 4];

#[no_mangle]
pub extern "C" fn GetVideoWidth() -> i32 {
    match core().video_w {
        0 => 640,
        w => w as i32,
    }
}

#[no_mangle]
pub extern "C" fn GetVideoHeight() -> i32 {
    match core().video_h {
        0 => 480,
        h => h as i32,
    }
}

#[no_mangle]
pub extern "C" fn GetVsyncNumerator() -> i32 {
    core().speed as i32
}

#[no_mangle]
pub extern "C" fn GetVsyncDenominator() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn GetAudio() -> *const i16 {
    let c = core();
    if c.audio.is_empty() {
        SILENCE.as_ptr()
    } else {
        c.audio.as_ptr()
    }
}
static SILENCE: [i16; 2] = [0; 2];

#[no_mangle]
pub extern "C" fn GetAudioSampleCount() -> i32 {
    (core().audio.len() / 2) as i32
}

// ---- Memory (RAM Watch, RAM Search, Hex Editor) and the game's properties ------------

/// One domain: the game's own numbers (see State).
#[no_mangle]
pub extern "C" fn GetMemoryDomainCount() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn GetMemoryDomainName(i: i32) -> *const u8 {
    match i {
        0 => b"Game State\0".as_ptr(),
        _ => b"\0".as_ptr(),
    }
}

#[no_mangle]
pub extern "C" fn GetMemoryDomainPtr(i: i32) -> *const u8 {
    match i {
        0 => state() as *mut State as *const u8,
        _ => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn GetMemoryDomainSize(i: i32) -> i64 {
    match i {
        0 => std::mem::size_of::<State>() as i64,
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn GetMemoryDomainWritable(i: i32) -> i32 {
    (i == 0) as i32
}

/// The table of the block above, in the shape docs/game-cores.md in Chimera
/// describes.
#[no_mangle]
pub extern "C" fn GetGameProperties() -> *const u8 {
    concat!(
        "{\"properties\":[",
        "{\"name\":\"Steps\",\"domain\":\"Game State\",\"offset\":0,\"type\":\"u64\",\"writable\":false,\"description\":\"Steps of the game run so far\"},",
        "{\"name\":\"Room\",\"domain\":\"Game State\",\"offset\":8,\"type\":\"s32\",\"writable\":false,\"description\":\"The number of the room being played\"},",
        "{\"name\":\"Room Speed\",\"domain\":\"Game State\",\"offset\":12,\"type\":\"s32\",\"writable\":false,\"description\":\"Steps a second of the room\"},",
        "{\"name\":\"Score\",\"domain\":\"Game State\",\"offset\":16,\"type\":\"s32\",\"description\":\"The game's built-in score variable\"},",
        "{\"name\":\"Lives\",\"domain\":\"Game State\",\"offset\":20,\"type\":\"s32\",\"description\":\"The game's built-in lives variable\"},",
        "{\"name\":\"Health\",\"domain\":\"Game State\",\"offset\":24,\"type\":\"f64\",\"description\":\"The game's built-in health variable\"},",
        "{\"name\":\"Random Seed\",\"domain\":\"Game State\",\"offset\":32,\"type\":\"s32\",\"description\":\"The seed random() and irandom() draw from next\"},",
        "{\"name\":\"Instances\",\"domain\":\"Game State\",\"offset\":36,\"type\":\"s32\",\"writable\":false,\"description\":\"Instances in the room\"},",
        "{\"name\":\"Mouse.X\",\"domain\":\"Game State\",\"offset\":40,\"type\":\"s32\",\"writable\":false,\"group\":\"Mouse\",\"description\":\"The pointer in the room\"},",
        "{\"name\":\"Mouse.Y\",\"domain\":\"Game State\",\"offset\":44,\"type\":\"s32\",\"writable\":false,\"group\":\"Mouse\",\"description\":\"The pointer in the room\"}",
        "]}\0"
    )
    .as_ptr()
}

// ---- Save data export (Emulator > Export Save Data...) --------------------------------

/// The files the game has written, now: the frontend writes them as a zip,
/// which the project's save data slot takes back.
#[no_mangle]
pub extern "C" fn GetSaveDataFileCount() -> i32 {
    let c = core();
    c.saves = gm8emulator::memfs::written()
        .into_iter()
        .map(|(name, bytes)| (CString::new(name).unwrap_or_default().into_bytes_with_nul(), bytes))
        .collect();
    c.saves.len() as i32
}

#[no_mangle]
pub extern "C" fn GetSaveDataFileName(i: i32) -> *const u8 {
    match core().saves.get(i as usize) {
        Some((name, _)) => name.as_ptr(),
        None => b"\0".as_ptr(),
    }
}

#[no_mangle]
pub extern "C" fn GetSaveDataFileSize(i: i32) -> i64 {
    core().saves.get(i as usize).map_or(0, |(_, b)| b.len() as i64)
}

#[no_mangle]
pub extern "C" fn GetSaveDataFileBuffer(i: i32) -> *const u8 {
    match core().saves.get(i as usize) {
        Some((_, b)) => b.as_ptr(),
        None => std::ptr::null(),
    }
}
