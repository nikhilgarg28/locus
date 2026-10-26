//! Spike: a fake `locus`, run by the driver as a separate process. It does not
//! link rustc.
//!
//! - `locus lower ROOT OUT`: lowers every `.lc` file under ROOT's directory to
//!   a `.rs` file under OUT, blanking the spike's Locus lines, which start with
//!   `contract ` or `uses ` (or `//@ contract `, `//@ uses `). A file is
//!   rewritten only when its text changes.
//! - `locus check --crate NAME [--src ROOT] --facts FILE [--iface OUT] [--dep NAME=IFACE]...`:
//!   the fake logical pass. It reads each dependency's interface, fails when a
//!   `uses DEP::NAME: VERSION` line of the crate does not match a
//!   `contract NAME: VERSION` line there, and writes the crate's interface: its
//!   own `contract` lines.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("lower") if args.len() == 3 => lower(Path::new(&args[1]), Path::new(&args[2])),
        Some("check") => check(&args[1..]),
        _ => {
            eprintln!("usage: locus lower ROOT OUT | locus check --crate NAME ...");
            exit(2)
        }
    }
}

fn lc_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            lc_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "lc") {
            out.push(path);
        }
    }
}

/// A spike Locus line. It may also be written after `//@`, which plain rustc
/// reads as a comment.
fn locus_line(line: &str) -> Option<(&'static str, &str)> {
    let t = line.trim_start();
    let t = t.strip_prefix("//@").map_or(t, str::trim_start);
    ["contract ", "uses "]
        .into_iter()
        .find_map(|k| t.strip_prefix(k).map(|rest| (k.trim(), rest.trim())))
}

fn write_if_changed(path: &Path, text: &str) {
    if fs::read_to_string(path).ok().as_deref() != Some(text) {
        fs::create_dir_all(path.parent().unwrap()).expect("create directory");
        fs::write(path, text).expect("write file");
    }
}

fn lower(root: &Path, out: &Path) {
    let src = root.parent().expect("root has a directory");
    let mut files = Vec::new();
    lc_files(src, &mut files);
    for file in files {
        let text = fs::read_to_string(&file).expect("read .lc file");
        let mut lowered = String::new();
        for line in text.lines() {
            lowered.push_str(if locus_line(line).is_some() { "" } else { line });
            lowered.push('\n');
        }
        let dest = out.join(file.strip_prefix(src).unwrap()).with_extension("rs");
        write_if_changed(&dest, &lowered);
    }
}

fn check(args: &[String]) {
    let (mut krate, mut src, mut facts, mut iface) = (String::new(), None, None, None);
    let mut deps = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().expect("option value").clone();
        match arg.as_str() {
            "--crate" => krate = value(),
            "--src" => src = Some(PathBuf::from(value())),
            "--facts" => facts = Some(PathBuf::from(value())),
            "--iface" => iface = Some(PathBuf::from(value())),
            "--dep" => {
                let v = value();
                let (name, path) = v.split_once('=').expect("--dep NAME=PATH");
                deps.push((name.to_string(), PathBuf::from(path)));
            }
            other => panic!("unknown option {other}"),
        }
    }

    let (mut contracts, mut uses) = (Vec::new(), Vec::new());
    if let Some(root) = &src {
        let mut files = Vec::new();
        lc_files(root.parent().unwrap(), &mut files);
        files.sort();
        for file in files {
            for line in fs::read_to_string(&file).unwrap().lines() {
                match locus_line(line) {
                    Some(("contract", rest)) => contracts.push(rest.to_string()),
                    Some((_, rest)) => uses.push(rest.to_string()),
                    None => {}
                }
            }
        }
    }

    let mut errors = 0;
    let mut offered: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, path) in &deps {
        match fs::read_to_string(path) {
            Ok(text) => {
                let lines = text.lines().filter_map(|l| l.strip_prefix("contract ").map(str::to_string));
                offered.insert(name.clone(), lines.collect());
            }
            Err(e) => {
                eprintln!("locus: error: no interface for dependency `{name}` at {}: {e}", path.display());
                errors += 1;
            }
        }
    }
    for expected in &uses {
        let (dep, contract) = expected.split_once("::").expect("uses DEP::NAME: VERSION");
        let name = contract.split(':').next().unwrap();
        let theirs = offered.get(dep).into_iter().flatten();
        if !theirs.clone().any(|c| c == contract) {
            let found: Vec<_> = theirs.filter(|c| c.split(':').next() == Some(name)).collect();
            eprintln!(
                "locus: error: `{krate}` relies on `{dep}::{contract}`, but the interface of `{dep}` states {found:?}"
            );
            errors += 1;
        }
    }

    let observations = facts
        .as_ref()
        .and_then(|f| fs::read_to_string(f).ok())
        .map_or(0, |t| t.lines().filter(|l| l.contains(r#""kind":"observation""#)).count());
    let verdict = if errors == 0 { "ok" } else { "FAILED" };
    log(&format!(
        "locus check {krate}: {observations} observations, {} dependency interfaces, {} expectations: {verdict}",
        deps.len(),
        uses.len()
    ));
    if errors > 0 {
        exit(1);
    }
    if let Some(path) = iface {
        let mut text = format!("crate {krate}\n");
        for c in &contracts {
            text.push_str(&format!("contract {c}\n"));
        }
        write_if_changed(&path, &text);
    }
}

fn log(line: &str) {
    use std::io::Write;
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() % 100_000;
    if let Ok(dir) = std::env::var("LOCUS_SPIKE_DIR")
        && let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(Path::new(&dir).join("log"))
    {
        // One write per line, as in the driver.
        let _ = f.write_all(format!("[{ms:05}] {line}\n").as_bytes());
    }
}
