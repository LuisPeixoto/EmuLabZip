//! `cargo run --release -p emulabzip-heap --example analyze -- app.hprof [--all-heaps]`
//!
//! Prints what the leak finder sees in a heap dump, as JSON.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: analyze <file.hprof> [--all-heaps]");
        std::process::exit(2);
    };
    let bytes = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("could not read {path}: {e}");
        std::process::exit(1);
    });
    let options = emulabzip_heap::Options { all_heaps: args.iter().any(|a| a == "--all-heaps"), ..Default::default() };
    let started = std::time::Instant::now();
    match emulabzip_heap::analyze(&bytes, &options) {
        Ok(a) => {
            println!("{}", serde_json::to_string_pretty(&a).unwrap());
            eprintln!("analyzed {} MB in {:.1} s", bytes.len() / (1024 * 1024), started.elapsed().as_secs_f32());
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
