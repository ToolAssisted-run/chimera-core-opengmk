// The native reference draws with the host build of the core's own Mesa
// (waterbox/setup-mesa.sh -n), never with whatever OpenGL the host has: the
// OpenGL is part of the machine, and the reference must be the same machine
// as the sandbox.
//
// llvmpipe turns shaders into machine code with LLVM, which is built static
// and without PIC (build-llvm.sh), so Mesa's shared libOSMesa.so cannot link:
// the Mesa archives, its OSMesa target object and LLVM's archives go into the
// program, as they go into the sandbox build.
use std::path::{Path, PathBuf};
use std::process::Command;

fn archives(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                archives(&p, out);
            } else if p.extension().map_or(false, |x| x == "a") {
                out.push(p);
            }
        }
    }
}

fn find_target(dir: &Path) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find_target(&p) {
                return Some(f);
            }
        } else if p.file_name().map_or(false, |n| n == "target.c.o") {
            return Some(p);
        }
    }
    None
}

fn main() {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = here.join("../..");
    let mesa = std::env::var("MESA_NATIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("build/mesa/build-native"));
    let llvm = std::env::var("LLVM_NATIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("build/llvm-native"));
    let target = find_target(&mesa.join("src/gallium/targets/osmesa"))
        .unwrap_or_else(|| panic!("no host Mesa at {} - run waterbox/setup-mesa.sh -n first", mesa.display()));
    let mut mesa_libs = Vec::new();
    archives(&mesa, &mut mesa_libs);
    mesa_libs.sort();
    let mut llvm_libs = Vec::new();
    archives(&llvm.join("lib"), &mut llvm_libs);
    llvm_libs.sort();
    if llvm_libs.is_empty() {
        panic!("no LLVM in {} - run waterbox/build-llvm.sh native first", llvm.display());
    }
    // what LLVM asks the dynamic loader for, answered by the core (jit-symbols.c)
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let jit_o = out.join("jit-symbols.o");
    let st = Command::new("gcc")
        .args(["-O2", "-c", "-o"])
        .arg(&jit_o)
        .arg(here.join("../jit-symbols.c"))
        .status()
        .expect("gcc");
    assert!(st.success());

    println!("cargo:rustc-link-arg=-no-pie");
    println!("cargo:rustc-link-arg={}", jit_o.display());
    println!("cargo:rustc-link-arg={}", target.display());
    println!("cargo:rustc-link-arg=-Wl,--start-group");
    for l in mesa_libs.iter().chain(llvm_libs.iter()) {
        println!("cargo:rustc-link-arg={}", l.display());
    }
    println!("cargo:rustc-link-arg=-Wl,--end-group");
    for w in ["dlopen", "dlsym", "dlclose"] {
        println!("cargo:rustc-link-arg=-Wl,--wrap={}", w);
    }
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rerun-if-env-changed=MESA_NATIVE_DIR");
    println!("cargo:rerun-if-env-changed=LLVM_NATIVE_DIR");
    println!("cargo:rerun-if-changed={}", here.join("../jit-symbols.c").display());
}
