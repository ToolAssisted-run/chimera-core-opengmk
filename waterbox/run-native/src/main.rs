//! run-native: the OpenGMK core's native reference.
//!
//! Runs a game the way the engine runs core.wbx - settings, then a step at a
//! time with that step's input - but on the host, where a debugger works.
//!
//! usage: run-native <game.exe> [options]
//!   --frames N              steps to run (default 600)
//!   --setting KEY=VALUE     a setting, as waterbox.config names it (repeatable)
//!   --press F:BUTTON        from step F, that button (its number in the
//!                           config's list) is held
//!   --release F:BUTTON      from step F, it is let go
//!   --poke F:OFFSET:VALUE   before step F, write the 32-bit VALUE at OFFSET of the
//!                           domain "Game State", as a tool does (16 is the score)
//!   --mouse F:X:Y           from step F, the pointer is at X,Y (0..8191 each)
//!   --screenshot F=PATH     write step F's picture as a TGA
//!   --digest-every N        print machine time and a picture hash every N steps
//!   --savedata ZIP          start from this save data (what --export-save wrote)
//!   --export-save ZIP       at the end, write what the game saved, as the
//!                           frontend's Export Save Data does
//!   --stall F:MS            before step F, keep the host busy for MS ms: the
//!                           machine must not notice (its clock is its own)
//!
//! At the end it prints one line the gate compares between flavors:
//!   frames=N time_ns=T video=<hash of the last picture> WxH
//!   audio=<hash of every step's sound> running=<0|1>

use opengmk_guest as core;
use std::collections::BTreeMap;

fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn video() -> (&'static [u8], usize, usize) {
    let w = core::GetVideoWidth() as usize;
    let h = core::GetVideoHeight() as usize;
    let p = core::GetVideoBgra();
    (unsafe { std::slice::from_raw_parts(p, w * h * 4) }, w, h)
}

fn write_tga(path: &str, bgra: &[u8], w: usize, h: usize) {
    let mut out = vec![0u8; 18];
    out[2] = 2; // uncompressed true colour
    out[12..14].copy_from_slice(&(w as u16).to_le_bytes());
    out[14..16].copy_from_slice(&(h as u16).to_le_bytes());
    out[16] = 32;
    out[17] = 0x28; // top-left origin, 8 alpha bits
    out.extend_from_slice(bgra);
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{path}: {e}"));
}

/// What Export Save Data would write: every file the core lists, in a zip,
/// every entry dated the same so the file is a function of the save.
fn write_save_zip(path: &str) {
    let n = core::GetSaveDataFileCount();
    let entry = |i: i32| {
        let name = unsafe { std::ffi::CStr::from_ptr(core::GetSaveDataFileName(i) as *const _) }
            .to_string_lossy()
            .into_owned();
        let size = core::GetSaveDataFileSize(i) as usize;
        let data = unsafe { std::slice::from_raw_parts(core::GetSaveDataFileBuffer(i), size) };
        (name, data)
    };
    for i in 0..n {
        let (name, data) = entry(i);
        println!("save {} {} {:016x}", name, data.len(), fnv64(data));
    }
    let mut out = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::from_date_and_time(2026, 10, 2, 0, 0, 0).unwrap());
        for i in 0..n {
            let (name, data) = entry(i);
            z.start_file(name, options).unwrap();
            std::io::Write::write_all(&mut z, data).unwrap();
        }
        z.finish().unwrap();
    }
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{path}: {e}"));
    eprintln!("run-native: exported {n} save files to {path}");
}

enum Change {
    /// a 32-bit value written into the "Game State" domain, as a tool would
    Poke(usize, i32),
    Press(i32),
    Release(i32),
    Mouse(i32, i32),
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut game = None;
    let mut frames: u64 = 600;
    let mut settings: BTreeMap<String, String> = BTreeMap::new();
    let mut changes: BTreeMap<u64, Vec<Change>> = BTreeMap::new();
    let mut shots: BTreeMap<u64, String> = BTreeMap::new();
    let mut digest_every: u64 = 0;
    let mut stall: Option<(u64, u64)> = None;
    let mut savedata: Option<String> = None;
    let mut export_save: Option<String> = None;
    let num = |s: &str| -> i64 { s.parse().unwrap_or_else(|_| panic!("not a number: {s}")) };
    while let Some(a) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--frames" => frames = num(&value()) as u64,
            "--setting" => {
                let v = value();
                let (k, v) = v.split_once('=').expect("--setting KEY=VALUE");
                settings.insert(k.to_string(), v.to_string());
            }
            "--press" | "--release" => {
                let v = value();
                let p: Vec<&str> = v.split(':').collect();
                assert!(p.len() == 2, "{a} F:BUTTON");
                let c = if a == "--press" { Change::Press(num(p[1]) as i32) } else { Change::Release(num(p[1]) as i32) };
                changes.entry(num(p[0]) as u64).or_default().push(c);
            }
            "--poke" => {
                let v = value();
                let p: Vec<&str> = v.split(':').collect();
                assert!(p.len() == 3, "--poke F:OFFSET:VALUE");
                changes.entry(num(p[0]) as u64).or_default().push(Change::Poke(num(p[1]) as usize, num(p[2]) as i32));
            }
            "--mouse" => {
                let v = value();
                let p: Vec<&str> = v.split(':').collect();
                assert!(p.len() == 3, "--mouse F:X:Y");
                changes.entry(num(p[0]) as u64).or_default().push(Change::Mouse(num(p[1]) as i32, num(p[2]) as i32));
            }
            "--screenshot" => {
                let v = value();
                let (f, path) = v.split_once('=').expect("--screenshot F=PATH");
                shots.insert(num(f) as u64, path.to_string());
            }
            "--savedata" => savedata = Some(value()),
            "--export-save" => export_save = Some(value()),
            "--digest-every" => digest_every = num(&value()) as u64,
            "--stall" => {
                let v = value();
                let (f, ms) = v.split_once(':').expect("--stall F:MS");
                stall = Some((num(f) as u64, num(ms) as u64));
            }
            other if !other.starts_with("--") && game.is_none() => game = Some(other.to_string()),
            other => panic!("unknown option {other}"),
        }
    }
    let game = game.expect("usage: run-native <game.exe> [options]");

    // the mounts the engine makes: the game under its own name, the settings
    let dir = std::env::temp_dir().join(format!("opengmk-run-native-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let name = std::path::Path::new(&game).file_name().unwrap().to_string_lossy().into_owned();
    std::fs::copy(&game, dir.join(&name)).unwrap();
    std::fs::write(dir.join("rom.name"), format!("/{name}")).unwrap();
    let json: BTreeMap<&String, &String> = settings.iter().collect();
    std::fs::write(dir.join("settings"), serde_like(&json)).unwrap();
    if let Some(zip) = &savedata {
        // a project: each file under its own name, the slot map saying which
        let zname = std::path::Path::new(zip).file_name().unwrap().to_string_lossy().into_owned();
        std::fs::copy(zip, dir.join(&zname)).unwrap();
        std::fs::write(dir.join("slots"), format!("{{\"game\":[\"{name}\"],\"savedata\":[\"{zname}\"]}}")).unwrap();
    }
    std::env::set_current_dir(&dir).unwrap();

    if core::Init() != 1 {
        let e = unsafe { std::ffi::CStr::from_ptr(core::GetLoadError() as *const _) };
        eprintln!("run-native: Init failed: {}", e.to_string_lossy());
        std::process::exit(1);
    }
    let mut audio_hash: u64 = 0xcbf29ce484222325;
    let mut peak = 0i32;
    for frame in 0..frames {
        if let Some((f, ms)) = stall {
            if f == frame {
                std::thread::sleep(std::time::Duration::from_millis(ms));
            }
        }
        for c in changes.get(&frame).into_iter().flatten() {
            match c {
                Change::Poke(offset, value) => unsafe {
                    let base = core::GetMemoryDomainPtr(0) as *mut u8;
                    std::ptr::copy_nonoverlapping(value.to_le_bytes().as_ptr(), base.add(*offset), 4);
                },
                Change::Press(b) => core::SetButton(*b, 1),
                Change::Release(b) => core::SetButton(*b, 0),
                Change::Mouse(x, y) => {
                    core::SetAxis(0, *x);
                    core::SetAxis(1, *y);
                }
            }
        }
        core::FrameAdvance(0);
        let n = core::GetAudioSampleCount() as usize * 2;
        let audio = unsafe { std::slice::from_raw_parts(core::GetAudio(), n) };
        for s in audio {
            for b in (*s as u16).to_le_bytes() {
                audio_hash ^= b as u64;
                audio_hash = audio_hash.wrapping_mul(0x100000001b3);
            }
            peak = peak.max((*s as i32).abs());
        }
        let (px, w, h) = video();
        if let Some(path) = shots.get(&frame) {
            write_tga(path, px, w, h);
        }
        if digest_every > 0 && (frame + 1) % digest_every == 0 {
            println!(
                "frame {} time_ns={} video={:016x} {}x{} peak={}",
                frame + 1,
                core::GetMachineTimeNs(),
                fnv64(px),
                w,
                h,
                peak
            );
            peak = 0;
        }
        if core::IsRunning() == 0 {
            break;
        }
    }
    let (px, w, h) = video();
    println!(
        "frames={} time_ns={} video={:016x} {}x{} audio={:016x} running={}",
        core::GetFrameCount(),
        core::GetMachineTimeNs(),
        fnv64(px),
        w,
        h,
        audio_hash,
        core::IsRunning()
    );
    if let Some(zip) = &export_save {
        write_save_zip(zip);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The settings as the flat JSON object the engine mounts.
fn serde_like(map: &BTreeMap<&String, &String>) -> String {
    let items: Vec<String> = map.iter().map(|(k, v)| format!("\"{k}\":\"{v}\"")).collect();
    format!("{{{}}}", items.join(","))
}
