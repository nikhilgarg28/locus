//! Standalone/build-script entry point. Checking always precedes emission;
//! receipts describe artifacts and never authorize skipped proof checking.
use super::{
    Checked, Error, Loaded,
    cargo::{self, CargoOptions},
};
use crate::{
    diagnostic::Diagnostic,
    elab,
    source::{SourceMap, Span},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Build {
    pub entry: PathBuf,
    pub out_dir: Option<PathBuf>,
    pub name: String,
    pub cargo: CargoOptions,
    pub options: elab::Options,
    pub use_proofs: bool,
    pub write_proofs: bool,
    pub locked_proofs: bool,
    pub search_proofs: bool,
}
#[derive(Clone, Debug)]
pub struct Built {
    pub rust: PathBuf,
    pub receipt: PathBuf,
    pub inputs: Vec<PathBuf>,
}
impl Built {
    /// Call from build.rs. All loaded modules, manifests and lockfiles are inputs.
    pub fn cargo_rerun_directives(&self) {
        for path in &self.inputs {
            println!("cargo:rerun-if-changed={}", path.display());
        }
        for name in ["TARGET", "CARGO_ENCODED_RUSTFLAGS"] {
            println!("cargo:rerun-if-env-changed={name}");
        }
    }
}
impl Build {
    pub fn new(entry: impl Into<PathBuf>) -> Self {
        Self {
            entry: entry.into(),
            out_dir: None,
            name: "locus".into(),
            cargo: CargoOptions::default(),
            options: elab::Options::default(),
            use_proofs: true,
            write_proofs: false,
            locked_proofs: false,
            search_proofs: true,
        }
    }
    pub fn out_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.out_dir = Some(path.into());
        self
    }
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
    pub fn manifest_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.cargo.manifest_path = Some(path.into());
        self
    }
    pub fn offline(mut self, yes: bool) -> Self {
        self.cargo.offline = yes;
        self
    }
    pub fn locked(mut self, yes: bool) -> Self {
        self.cargo.locked = yes;
        self
    }
    fn load(&self) -> Result<Loaded, Error> {
        let workspace = cargo::discover(&self.entry, &self.cargo)
            .map_err(|e| driver("L0505", &self.entry, e))?;
        if let Some(workspace) = workspace {
            super::load::load_cargo(&self.entry, workspace)
        } else {
            super::load(&self.entry)
        }
    }
    pub fn check(&self) -> Result<Checked, Error> {
        let loaded = self.load()?;
        if !self.use_proofs {
            return super::check::check_loaded(loaded, &self.options);
        }
        let directory = loaded
            .cargo
            .as_ref()
            .map(|w| w.packages[w.host].manifest.parent().unwrap())
            .unwrap_or_else(|| loaded.entry.parent().unwrap_or(Path::new(".")));
        let lock_path = directory.join(crate::store::FILE_NAME);
        let key = loaded
            .entry
            .strip_prefix(directory)
            .unwrap_or(&loaded.entry)
            .to_string_lossy()
            .replace('\\', "/");
        if std::fs::metadata(&lock_path).is_ok_and(|m| m.len() > crate::store::MAX_FILE as u64) {
            return Err(driver(
                "L0505",
                &lock_path,
                "proof lockfile exceeds MAX_FILE".into(),
            ));
        }
        let text = if lock_path.is_file() {
            std::fs::read_to_string(&lock_path)
                .map_err(|e| driver("L0505", &lock_path, e.to_string()))?
        } else {
            String::new()
        };
        let mut lock = if text.is_empty() {
            crate::store::Lockfile::new()
        } else {
            crate::store::Lockfile::parse(&text)
                .map_err(|e| driver("L0505", &lock_path, e))?
                .0
        };
        let store = lock
            .take(&key)
            .locked(self.locked_proofs)
            .searching(!self.locked_proofs && self.search_proofs);
        let (result, store) =
            crate::store::with_store(store, || super::check::check_loaded(loaded, &self.options));
        let checked = result?;
        if self.write_proofs && !self.locked_proofs {
            lock.put(&key, &store);
            let new = lock.render();
            if new != text && (!lock.is_empty() || !text.is_empty()) {
                atomic_write(&lock_path, new.as_bytes())
                    .map_err(|e| driver("L0505", &lock_path, e.to_string()))?;
            }
        }
        Ok(checked)
    }
    pub fn rust(&self) -> Result<String, Error> {
        super::rust(self.check()?)
    }
    pub fn generate(&self) -> Result<Built, Error> {
        cargo::identifier(&self.name).map_err(|e| driver("L0505", &self.entry, e))?;
        let output = self
            .out_dir
            .clone()
            .or_else(|| std::env::var_os("OUT_DIR").map(PathBuf::from))
            .ok_or_else(|| {
                driver(
                    "L0505",
                    &self.entry,
                    "set an output directory or call from Cargo build.rs with OUT_DIR".into(),
                )
            })?;
        let checked = self.check()?;
        let inputs = checked.loaded.inputs.clone();
        let configuration = self.configuration(&checked.loaded);
        let source = super::rust(checked)?;
        let rust = output.join(format!("{}.rs", self.name));
        let receipt = output.join(format!("{}.locus.json", self.name));
        if rust.exists() && !owned_receipt(&receipt, &self.name) {
            return Err(driver("L0506",&rust,"refusing to overwrite an existing file without a matching Locus build receipt; choose a fresh output name/directory".into()));
        }
        let input_hashes: BTreeMap<_, _> = inputs
            .iter()
            .map(|(p, s)| (p.to_string_lossy().into_owned(), hash(s.as_bytes())))
            .collect();
        let record = serde_json::json!({"format":1,"name":self.name,"compiler":compiler_id(),"configuration":configuration,"inputs":input_hashes,"outputs":{format!("{}.rs",self.name):hash(source.as_bytes())}});
        let receipt_text = serde_json::to_string_pretty(&record).expect("JSON record") + "\n";
        std::fs::create_dir_all(&output).map_err(|e| driver("L0506", &output, e.to_string()))?;
        // Write the receipt last: an interrupted build cannot leave a matching
        // receipt for a partially installed output. No unrelated files are removed.
        write_changed(&rust, source.as_bytes())
            .map_err(|e| driver("L0506", &rust, e.to_string()))?;
        write_changed(&receipt, receipt_text.as_bytes())
            .map_err(|e| driver("L0506", &receipt, e.to_string()))?;
        Ok(Built {
            rust,
            receipt,
            inputs: inputs.into_keys().collect(),
        })
    }
    pub fn is_current(&self) -> Result<bool, Error> {
        let output = self
            .out_dir
            .clone()
            .or_else(|| std::env::var_os("OUT_DIR").map(PathBuf::from))
            .ok_or_else(|| driver("L0505", &self.entry, "no output directory".into()))?;
        let Ok(text) = std::fs::read(output.join(format!("{}.locus.json", self.name))) else {
            return Ok(false);
        };
        let Ok(record) = serde_json::from_slice::<serde_json::Value>(&text) else {
            return Ok(false);
        };
        if record["compiler"] != compiler_id()
            || compiler_id() == "unavailable"
            || record["format"] != 1
            || record["name"] != self.name
        {
            return Ok(false);
        }
        let loaded = self.load()?;
        if record["configuration"] != self.configuration(&loaded) {
            return Ok(false);
        }
        let inputs: BTreeMap<_, _> = loaded
            .inputs
            .iter()
            .map(|(p, s)| (p.to_string_lossy().into_owned(), hash(s.as_bytes())))
            .collect();
        if record["inputs"] != serde_json::json!(inputs) {
            return Ok(false);
        }
        let Ok(source) = std::fs::read(output.join(format!("{}.rs", self.name))) else {
            return Ok(false);
        };
        Ok(record["outputs"][format!("{}.rs", self.name)] == hash(&source))
    }
    fn configuration(&self, loaded: &Loaded) -> serde_json::Value {
        serde_json::json!({"entry":loaded.entry.canonicalize().unwrap_or_else(|_|loaded.entry.clone()),"previews":self.options.previews.iter().map(|p|p.name()).collect::<Vec<_>>(),"cargo":loaded.cargo.as_ref().map(|w|&w.selection),"check_moves":self.options.check_moves})
    }
}
fn owned_receipt(path: &Path, name: &str) -> bool {
    std::fs::read(path)
        .ok()
        .and_then(|t| serde_json::from_slice::<serde_json::Value>(&t).ok())
        .is_some_and(|v| {
            v["format"] == 1
                && v["name"] == name
                && v["outputs"]
                    .as_object()
                    .is_some_and(|o| o.len() == 1 && o.contains_key(&format!("{name}.rs")))
        })
}
pub(super) fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn compiler_id() -> &'static str {
    crate::bench::source_identity()
}
fn write_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if std::fs::read(path).ok().as_deref() == Some(bytes) {
        Ok(())
    } else {
        atomic_write(path, bytes)
    }
}
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temporary = path.with_extension(format!(
        "locus-tmp-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
pub(super) fn driver(code: &'static str, path: &Path, message: String) -> Error {
    let mut sources = SourceMap::default();
    let file = sources.add(path.display().to_string(), "");
    Error {
        sources,
        diagnostics: vec![Diagnostic::error(code, message, Span::new(file, 0, 0))],
    }
}
