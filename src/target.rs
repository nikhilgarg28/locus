//! Target layout is a build input. Querying rustc's cfg does not require the
//! target standard library, and does not execute target code.
use crate::kernel::PointerWidth;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug)]
pub struct TargetLayout {
    pub triple: String,
    pub pointer_width: PointerWidth,
    pub compiler: String,
    pub inputs: BTreeMap<PathBuf, String>,
}
impl TargetLayout {
    pub fn receipt(&self) -> serde_json::Value {
        serde_json::json!({"target":self.triple,"pointer_width":self.pointer_width.bits(),"rustc":self.compiler})
    }
    /// Resolve from the same host-package directory used for native extraction.
    /// Explicit selection and Cargo's environment override configuration files.
    pub fn discover(cwd: &Path, explicit: Option<&str>) -> Result<Self, String> {
        let mut inputs = BTreeMap::new();
        let mut configured = explicit
            .map(str::to_owned)
            .or_else(|| std::env::var("TARGET").ok())
            .or_else(|| std::env::var("CARGO_BUILD_TARGET").ok());
        for dir in cwd.ancestors() {
            read_config(&dir.join(".cargo"), &mut inputs, &mut configured)?;
        }
        let cargo_home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cargo")));
        if let Some(home) = cargo_home {
            read_config(&home, &mut inputs, &mut configured)?;
        }
        let target = configured;
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compiler = output(
            Command::new(&rustc).arg("-vV").current_dir(cwd),
            "query Rust compiler version",
        )?;
        let triple = match target {
            Some(t) if !t.is_empty() => t,
            Some(_) => return Err("Rust target must not be empty".into()),
            None => compiler
                .lines()
                .find_map(|l| l.strip_prefix("host: "))
                .map(str::to_owned)
                .ok_or("rustc -vV did not report its host target")?,
        };
        // The JSON target specification itself participates in receipt freshness.
        if triple.ends_with(".json") {
            let path = Path::new(&triple);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            };
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read target specification {}: {e}", path.display()))?;
            inputs.insert(path, text);
        }
        let cfg = output(
            Command::new(&rustc)
                .args(["--print", "cfg", "--target", &triple])
                .current_dir(cwd),
            "query target layout",
        )?;
        let bits = cfg
            .lines()
            .find_map(|line| {
                line.strip_prefix("target_pointer_width=\"")
                    .and_then(|s| s.strip_suffix('"'))
            })
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or("rustc did not report target_pointer_width")?;
        let pointer_width=PointerWidth::from_bits(bits).ok_or_else(||format!("Locus supports 32-bit and 64-bit pointers; Rust target `{triple}` has {bits}-bit pointers"))?;
        Ok(Self {
            triple,
            pointer_width,
            compiler,
            inputs,
        })
    }
}
fn output(command: &mut Command, what: &str) -> Result<String, String> {
    let out = command
        .output()
        .map_err(|e| format!("cannot {what}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cannot {what}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout)
        .map_err(|_| format!("cannot {what}: rustc returned non-UTF-8 output"))
}
fn read_config(
    dir: &Path,
    inputs: &mut BTreeMap<PathBuf, String>,
    selected: &mut Option<String>,
) -> Result<(), String> {
    // Cargo prefers the legacy name if both exist.
    let old = dir.join("config");
    let new = dir.join("config.toml");
    let path = if old.is_file() { old } else { new };
    if !path.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read Cargo configuration {}: {e}", path.display()))?;
    let config = toml::from_str::<toml::Table>(&text)
        .map_err(|e| format!("invalid Cargo configuration {}: {e}", path.display()))?;
    if selected.is_none()
        && let Some(target) = config.get("build").and_then(|b| b.get("target"))
    {
        *selected=Some(target.as_str().ok_or_else(||format!("Cargo configuration {} selects multiple or non-string targets; select one explicitly with --target",path.display()))?.into());
    }
    inputs.insert(path, text);
    Ok(())
}
