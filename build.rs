//! Record the toolchain/settings that built this binary, not the later shell.
use std::{
    env,
    fmt::Write as _,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
fn output(command: &mut Command) -> String {
    command
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().into())
        .unwrap_or_else(|| "unavailable".into())
}
fn sources(path: &Path, paths: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            sources(&entry, paths);
        }
    } else {
        paths.push(path.to_owned());
    }
}
fn main() {
    for path in [
        "build.rs",
        "src",
        "docs/diagnostics",
        "tools/bench.py",
        "Cargo.toml",
        "Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let mut paths = vec![
        PathBuf::from("build.rs"),
        PathBuf::from("Cargo.toml"),
        PathBuf::from("Cargo.lock"),
    ];
    sources(Path::new("src"), &mut paths);
    sources(Path::new("docs/diagnostics"), &mut paths);
    paths.push(PathBuf::from("tools/bench.py"));
    paths.sort();
    let mut raw = Vec::new();
    for path in paths {
        raw.extend_from_slice(path.to_string_lossy().as_bytes());
        raw.push(0);
        raw.extend(fs::read(path).unwrap());
        raw.push(0);
    }
    let source = Command::new("git")
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
        .and_then(|mut child| {
            child.stdin.take()?.write_all(&raw).ok()?;
            let result = child.wait_with_output().ok()?;
            result
                .status
                .success()
                .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
        })
        .unwrap_or_else(|| "unavailable".into());
    let mut values = vec![
        (
            "TOOLCHAIN",
            output(Command::new(env::var_os("RUSTC").unwrap()).arg("-Vv")),
        ),
        ("SOURCE_GIT_BLOB", source),
    ];
    for name in [
        "PROFILE",
        "OPT_LEVEL",
        "DEBUG",
        "TARGET",
        "HOST",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        values.push((name, env::var(name).unwrap_or_default()));
    }
    let mut code = String::new();
    for (name, value) in values {
        writeln!(code, "pub const {name}: &str = {value:?};").unwrap();
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("bench_build.rs"),
        code,
    )
    .unwrap();
}
