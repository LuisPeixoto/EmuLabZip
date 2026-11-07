//! Builds a tiny program with debug info, makes its `.dSYM`, and looks an address up with the real `atos`.
//! It needs a Mac with Xcode's command line tools, and skips itself anywhere else.

#![cfg(target_os = "macos")]

use std::path::Path;
use std::process::Command;

use emulabzip_symbols::dsym::Dsym;

fn tool(args: &[&str], dir: &Path) -> Option<String> {
    let out = Command::new(args[0]).args(&args[1..]).current_dir(dir).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn names_a_function_and_a_line_from_a_dsym() {
    let dir = std::env::temp_dir().join(format!("emulabzip-dsym-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("prog.c"),
        "#include <stdio.h>\n__attribute__((noinline)) int explode(int n) {\n  volatile int x = n * 2;\n  return x + 1;\n}\nint main(void) {\n  printf(\"%d\\n\", explode(3));\n  return 0;\n}\n",
    )
    .unwrap();
    // The object file stays next to the program: dsymutil reads the debug information from it.
    let built = tool(&["clang", "-g", "-O0", "-c", "-o", "prog.o", "prog.c"], &dir).is_some()
        && tool(&["clang", "-o", "prog", "prog.o"], &dir).is_some()
        && tool(&["dsymutil", "prog"], &dir).is_some();
    if !built || tool(&["atos", "-h"], &dir).is_none() && Command::new("atos").arg("-h").output().is_err() {
        eprintln!("skipped: no compiler, dsymutil or atos here");
        return;
    }
    let dsym = Dsym::open(&dir.join("prog.dSYM")).unwrap();
    let uuids = dsym.uuids().unwrap();
    assert!(!uuids.is_empty());
    let (uuid, arch) = &uuids[0];
    assert!(dsym.is_for(&uuid.to_uppercase()).unwrap(), "any case is the same UUID");
    assert!(!dsym.is_for("00000000-0000-0000-0000-000000000000").unwrap());

    // The address of `explode` in the image: ask the linker's symbol table for it.
    let nm = tool(&["nm", "-arch", arch, "prog"], &dir).expect("nm");
    let address = nm
        .lines()
        .find(|l| l.ends_with(" _explode"))
        .and_then(|l| u64::from_str_radix(l.split_whitespace().next()?, 16).ok())
        .expect("the address of explode");
    // The program is not loaded: atos takes the load address as 0x100000000 for an executable.
    let base = 0x1_0000_0000u64;
    let found = dsym.symbolicate(arch, base, &[address + 4, 0]).unwrap();
    let symbol = found[0].as_ref().expect("atos names the address");
    assert_eq!(symbol.function, "explode");
    assert_eq!(symbol.file.as_deref(), Some("prog.c"));
    assert!(matches!(symbol.line, Some(2..=4)), "{:?}", symbol.line);
    let _ = std::fs::remove_dir_all(dir);
}
