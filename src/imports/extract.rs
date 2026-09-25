//! Cargo supplies native cfg/extern arguments. The extractor does not resolve
//! dependency features itself and never enables unstable Rust source in Cargo.
use super::model::Interface;
use crate::project::cargo::{CargoOptions, Workspace};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!(
            "locus-native-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut b = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        b.create(&root)
            .map_err(|e| format!("cannot create import workspace: {e}"))?;
        Ok(Self(root))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub struct Invocation {
    pub compiler: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}
impl std::fmt::Debug for Invocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invocation")
            .field("compiler", &self.compiler)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field("environment_entries", &self.env.len())
            .finish_non_exhaustive()
    }
}
impl Invocation {
    fn read(path: &Path) -> Result<Self, String> {
        let data = fs::read(path).map_err(|e| format!("cannot read compiler capture: {e}"))?;
        if data.len() > crate::limits::MAX_RUSTC_CAPTURE_BYTES {
            return Err("compiler capture exceeds the 16 MiB limit".into());
        }
        let mut input = data.as_slice();
        fn n(input: &mut &[u8]) -> Result<usize, String> {
            let b = input.get(..4).ok_or("truncated compiler capture")?;
            let n = u32::from_le_bytes(b.try_into().unwrap()) as usize;
            *input = &input[4..];
            Ok(n)
        }
        fn strings(input: &mut &[u8]) -> Result<Vec<String>, String> {
            let count = n(input)?;
            if count > crate::limits::MAX_RUSTC_CAPTURE_STRINGS {
                return Err("too many compiler arguments".into());
            }
            (0..count)
                .map(|_| {
                    let len = n(input)?;
                    let s = input.get(..len).ok_or("truncated compiler argument")?;
                    let s = std::str::from_utf8(s)
                        .map_err(|_| "non-UTF-8 compiler argument")?
                        .to_owned();
                    *input = &input[len..];
                    Ok(s)
                })
                .collect()
        }
        let mut args = strings(&mut input)?;
        let cwd = strings(&mut input)?;
        let env = strings(&mut input)?;
        if args.is_empty() || cwd.len() != 1 || env.len() % 2 != 0 || !input.is_empty() {
            return Err("malformed compiler capture".into());
        }
        let compiler = args.remove(0);
        Ok(Self {
            compiler,
            args,
            cwd: PathBuf::from(&cwd[0]),
            env: env
                .chunks_exact(2)
                .map(|p| (p[0].clone(), p[1].clone()))
                .collect(),
        })
    }
    fn option(&self, name: &str) -> Option<&str> {
        self.args.iter().enumerate().find_map(|(i, a)| {
            if a == name {
                self.args.get(i + 1).map(String::as_str)
            } else {
                a.strip_prefix(&format!("{name}="))
            }
        })
    }
    fn source(&self) -> Option<PathBuf> {
        self.args.iter().find(|a| a.ends_with(".rs")).map(|a| {
            self.cwd
                .join(a)
                .canonicalize()
                .unwrap_or_else(|_| self.cwd.join(a))
        })
    }
    fn flags(&self) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.args.len() {
            let a = &self.args[i];
            if [
                "--crate-type",
                "--crate-name",
                "--edition",
                "--cfg",
                "--check-cfg",
                "--extern",
                "-L",
                "-C",
                "--target",
                "--cap-lints",
                "--sysroot",
                "-l",
            ]
            .contains(&a.as_str())
            {
                out.push(a.clone());
                i += 1;
                out.push(
                    self.args
                        .get(i)
                        .ok_or("incomplete captured compiler flag")?
                        .clone(),
                );
            } else if a.ends_with(".rs")
                || [
                    "--edition=",
                    "--cap-lints=",
                    "--target=",
                    "--cfg=",
                    "--check-cfg=",
                    "--extern=",
                    "-L",
                    "-C",
                ]
                .iter()
                .any(|prefix| a.starts_with(prefix))
            {
                out.push(a.clone());
            } else if a.starts_with("-Z") {
                return Err("native imports do not support unstable rustc flags; select a stable native compilation configuration".into());
            }
            i += 1;
        }
        Ok(out)
    }
}
#[derive(Debug)]
pub struct Native {
    pub interface: Interface,
    pub metadata: Option<PathBuf>,
    pub invocation: usize,
}
#[derive(Debug)]
pub struct Extraction {
    temp: Temporary,
    pub invocations: Vec<Invocation>,
    artifacts: Vec<Value>,
    pub interfaces: BTreeMap<usize, Native>,
    pub context: Value,
}
fn output(command: &mut Command, what: &str) -> Result<Output, String> {
    let result = command
        .output()
        .map_err(|e| format!("cannot run {what}: {e}"))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        // Cargo JSON diagnostics are on stdout. Keep their rendered explanation.
        let rendered = String::from_utf8_lossy(&result.stdout)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter_map(|v| v["message"]["rendered"].as_str().map(str::to_owned))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!(
            "{what} failed ({}):\n{}\n{}",
            result.status,
            excerpt(&stderr),
            excerpt(&rendered)
        ));
    }
    Ok(result)
}
fn excerpt(s: &str) -> &str {
    if s.len() <= 16_384 {
        s
    } else {
        let mut start = s.len() - 16_384;
        while !s.is_char_boundary(start) {
            start += 1;
        }
        &s[start..]
    }
}
fn version(exe: impl AsRef<std::ffi::OsStr>, cwd: &Path) -> Result<String, String> {
    let out = output(
        Command::new(exe).arg("-vV").current_dir(cwd),
        "toolchain version query",
    )?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Extraction {
    pub fn preflight() -> Result<(), String> {
        if std::env::var_os("LOCUS_IMPORT_ACTIVE").is_some() {
            return Err("recursive native import extraction from Cargo build.rs is not supported yet. Move the Rust interface to a package that builds independently, or generate Locus output before the consuming Cargo build; extraction was stopped to avoid a Cargo lock/dependency cycle.".into());
        }
        for name in ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"] {
            if std::env::var_os(name).is_some_and(|s| !s.is_empty()) {
                return Err(format!(
                    "native import capture cannot yet compose with {name}; unset it for the import build so the compiler configuration can be recorded exactly"
                ));
            }
        }
        Ok(())
    }
    pub fn new(workspace: &Workspace, options: &CargoOptions) -> Result<Self, String> {
        Self::preflight()?;
        let temp = Temporary::new()?;
        let capture = temp.0.join("capture");
        fs::create_dir(&capture).map_err(|e| e.to_string())?;
        let helper = temp.0.join("capture.rs");
        fs::write(&helper, include_str!("capture.rs")).map_err(|e| e.to_string())?;
        let executable = temp
            .0
            .join(format!("capture-driver{}", std::env::consts::EXE_SUFFIX));
        let host = &workspace.packages[workspace.host];
        let cwd = host.manifest.parent().unwrap();
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        output(
            Command::new(&rustc)
                .current_dir(cwd)
                .arg("--edition=2024")
                .arg(&helper)
                .arg("-o")
                .arg(&executable)
                .env_remove("RUSTC_BOOTSTRAP"),
            "compiling the import capture helper",
        )?;
        let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        command
            .current_dir(cwd)
            .args(["check", "--message-format=json", "--manifest-path"])
            .arg(&host.manifest)
            .arg("--package")
            .arg(&host.id)
            .arg("--target-dir")
            .arg(temp.0.join("target"))
            .env("RUSTC_WRAPPER", &executable)
            .env("LOCUS_RUSTC_CAPTURE", &capture)
            .env("LOCUS_IMPORT_ACTIVE", "1")
            .env_remove("RUSTC_BOOTSTRAP");
        if options.offline {
            command.arg("--offline");
        }
        if options.locked {
            command.arg("--locked");
        }
        if options.no_default_features {
            command.arg("--no-default-features");
        }
        if options.all_features {
            command.arg("--all-features");
        }
        if !options.features.is_empty() {
            command.arg("--features").arg(options.features.join(","));
        }
        if let Some(target) = &options.target {
            command.arg("--target").arg(target);
        }
        let result = output(&mut command, "Cargo native interface build")?;
        let artifacts = String::from_utf8_lossy(&result.stdout)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|v| v["reason"] == "compiler-artifact")
            .collect();
        let mut invocations = Vec::new();
        let mut paths = fs::read_dir(&capture)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        paths.sort();
        for path in paths {
            if path.join("complete").is_file() {
                invocations.push(Invocation::read(&path.join("invocation.bin"))?);
            }
        }
        let inputs: BTreeMap<_, _> = workspace
            .inputs
            .iter()
            .map(|(path, text)| (path.display().to_string(), hash(text.as_bytes())))
            .collect();
        let context = json!({"cargo_selection":workspace.selection,"cargo_inputs":inputs,"host_package":host.id,"adapter":1});
        Ok(Self {
            temp,
            invocations,
            artifacts,
            interfaces: BTreeMap::new(),
            context,
        })
    }
    pub fn get(&mut self, workspace: &Workspace, package: usize) -> Result<&Native, String> {
        if self.interfaces.contains_key(&package) {
            return Ok(&self.interfaces[&package]);
        }
        let p = &workspace.packages[package];
        let artifacts: Vec<_> = self
            .artifacts
            .iter()
            .filter(|a| {
                a["package_id"] == p.id
                    && a["target"]["kind"].as_array().is_some_and(|k| {
                        k.iter()
                            .any(|v| v == "lib" || v == "rlib" || v == "proc-macro")
                    })
            })
            .collect();
        let artifact = match artifacts.as_slice() {
            [a] => *a,
            [] => {
                return Err(format!(
                    "Cargo did not compile a library for `{}` in this target/feature selection; inactive, binary-only and build-only dependencies cannot be imported",
                    p.name
                ));
            }
            _ => {
                return Err(format!(
                    "Cargo produced multiple native library instances for `{}`. Host/target feature-split selection is ambiguous; no interface was chosen.",
                    p.name
                ));
            }
        };
        let source = PathBuf::from(
            artifact["target"]["src_path"]
                .as_str()
                .ok_or("Cargo library has no source path")?,
        )
        .canonicalize()
        .map_err(|e| e.to_string())?;
        let name = artifact["target"]["name"]
            .as_str()
            .ok_or("Cargo library has no name")?;
        let metadata = artifact["filenames"]
            .as_array()
            .and_then(|v| {
                v.iter()
                    .filter_map(Value::as_str)
                    .find(|s| s.ends_with(".rmeta"))
            })
            .map(PathBuf::from);
        let artifact_path = metadata
            .clone()
            .or_else(|| {
                artifact["filenames"]
                    .as_array()?
                    .first()?
                    .as_str()
                    .map(PathBuf::from)
            })
            .ok_or("Cargo library artifact has no output path")?;
        let matches: Vec<_> = self
            .invocations
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                i.source().as_ref() == Some(&source)
                    && i.option("--crate-name") == Some(name)
                    && i.option("--out-dir")
                        .is_some_and(|dir| artifact_path.starts_with(dir))
            })
            .collect();
        let (index, inv) = match matches.as_slice() {
            [one] => *one,
            _ => {
                return Err(format!(
                    "could not uniquely match Cargo's native compiler invocation for `{}`; refusing to guess its feature/target configuration",
                    p.name
                ));
            }
        };
        let rustdoc = std::env::var_os("RUSTDOC").unwrap_or_else(|| {
            Path::new(&inv.compiler)
                .parent()
                .unwrap_or(Path::new(""))
                .join(format!("rustdoc{}", std::env::consts::EXE_SUFFIX))
                .into_os_string()
        });
        let compiler_version = version(&inv.compiler, &inv.cwd)?;
        let doc_version = version(&rustdoc, &inv.cwd)?;
        let commit = |s: &str| {
            s.lines()
                .find_map(|l| l.strip_prefix("commit-hash: "))
                .map(str::to_owned)
        };
        if commit(&compiler_version).is_none() || commit(&compiler_version) != commit(&doc_version)
        {
            return Err("rustc and rustdoc are from different toolchains; use matching tools for native imports".into());
        }
        let doc_dir = self.temp.0.join(format!("doc-{package}"));
        fs::create_dir(&doc_dir).map_err(|e| e.to_string())?;
        let flags = inv.flags()?;
        let mut command = Command::new(&rustdoc);
        command
            .current_dir(&inv.cwd)
            .env_clear()
            .envs(&inv.env)
            .env_remove("CARGO_MAKEFLAGS")
            .env_remove("MAKEFLAGS")
            .env("RUSTC_BOOTSTRAP", "1")
            .args(&flags)
            .args([
                "--output-format",
                "json",
                "-Z",
                "unstable-options",
                "--document-hidden-items",
                "-o",
            ])
            .arg(&doc_dir);
        output(
            &mut command,
            "rustdoc JSON extraction (installed toolchain; no nightly installation)",
        )?;
        let json_path = doc_dir.join(format!("{name}.json"));
        if fs::metadata(&json_path)
            .map_err(|e| format!("rustdoc did not produce {}: {e}", json_path.display()))?
            .len()
            > crate::limits::MAX_RUSTDOC_BYTES as u64
        {
            return Err("rustdoc JSON exceeds the import size limit".into());
        }
        let bytes = fs::read(&json_path).map_err(|e| e.to_string())?;
        let stable_flags: Vec<_> = flags
            .iter()
            .map(|a| a.replace(self.temp.0.to_str().unwrap_or(""), "<import-build>"))
            .collect();
        let context = json!({"build":self.context,"package":p.id,"compiler":compiler_version,"rustdoc":doc_version,"native_flags":stable_flags,"features":artifact["features"],"native_metadata_sha256":hash(&fs::read(&artifact_path).map_err(|e|e.to_string())?)});
        let interface = Interface::read(&bytes, &p.id, context)?;
        self.interfaces.insert(
            package,
            Native {
                interface,
                metadata,
                invocation: index,
            },
        );
        Ok(&self.interfaces[&package])
    }
    /// Check a used callable against Cargo's native metadata, not rustdoc's
    /// cfg(doc) view. No proof is accepted from this attestation.
    pub fn validate_function(
        &self,
        package: usize,
        path: &[String],
        signature: &super::model::Signature,
    ) -> Result<(), String> {
        let native = self
            .interfaces
            .get(&package)
            .ok_or("missing native interface")?;
        let inv = &self.invocations[native.invocation];
        let metadata = native.metadata.as_ref().ok_or(
            "this Rust library has no callable native type metadata; only inspection is supported",
        )?;
        let dir = self
            .temp
            .0
            .join(format!("probe-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
        let source = dir.join("probe.rs");
        let path = path
            .iter()
            .map(|p| format!("r#{p}"))
            .collect::<Vec<_>>()
            .join("::");
        let params = signature
            .inputs
            .iter()
            .map(|t| t.spelling())
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            &source,
            format!(
                "const _: fn({params}) -> {} = __locus_native::{path};\n",
                signature.output.spelling()
            ),
        )
        .map_err(|e| e.to_string())?;
        let mut command = Command::new(&inv.compiler);
        command
            .current_dir(&inv.cwd)
            .env_clear()
            .envs(&inv.env)
            .env_remove("RUSTC_BOOTSTRAP")
            .env_remove("CARGO_MAKEFLAGS")
            .env_remove("MAKEFLAGS")
            .args([
                "--edition=2024",
                "--crate-type=lib",
                "--crate-name=locus_import_probe",
                "--emit=metadata",
            ])
            .arg(&source)
            .arg("--extern")
            .arg(format!("__locus_native={}", metadata.display()))
            .arg("-o")
            .arg(dir.join("probe.rmeta"));
        let mut i = 0;
        while i < inv.args.len() {
            let a = &inv.args[i];
            if ["--target", "--sysroot", "-L"].contains(&a.as_str()) {
                command.arg(a);
                i += 1;
                command.arg(
                    inv.args
                        .get(i)
                        .ok_or("missing native linker/target argument")?,
                );
            } else if a.starts_with("--target=")
                || a.starts_with("--sysroot=")
                || a.starts_with("-L")
            {
                command.arg(a);
            }
            i += 1;
        }
        output(
            &mut command,
            "native Rust signature validation; the item may be unavailable outside cfg(doc), unstable, or different in this build",
        )?;
        Ok(())
    }
}
