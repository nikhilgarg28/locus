//! Cargo owns package resolution. Locus consumes Cargo's resolved graph and
//! package metadata; it never searches registry/cache directory layouts.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, Default)]
pub struct CargoOptions {
    pub manifest_path: Option<PathBuf>,
    pub offline: bool,
    pub locked: bool,
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
    pub target: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Target {
    pub name: String,
    pub entry: PathBuf,
    pub rust_module: String,
}
#[derive(Clone, Debug)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub manifest: PathBuf,
    pub library: Option<PathBuf>,
    pub targets: Vec<Target>,
    pub dependencies: BTreeMap<String, usize>,
}
#[derive(Clone, Debug)]
pub struct Workspace {
    pub packages: Vec<Package>,
    pub host: usize,
    pub root: PathBuf,
    pub inputs: BTreeMap<PathBuf, String>,
    pub selection: Value,
}

pub fn manifest_for(entry: &Path, explicit: Option<&Path>) -> Result<Option<PathBuf>, String> {
    if let Some(path) = explicit {
        return path
            .canonicalize()
            .map(Some)
            .map_err(|e| format!("cannot open manifest {}: {e}", path.display()));
    }
    let path = if entry.is_dir() {
        entry
    } else {
        entry
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    };
    let path = path
        .canonicalize()
        .map_err(|e| format!("cannot locate source directory {}: {e}", path.display()))?;
    Ok(path
        .ancestors()
        .map(|p| p.join("Cargo.toml"))
        .find(|p| p.is_file()))
}

pub fn discover(entry: &Path, options: &CargoOptions) -> Result<Option<Workspace>, String> {
    let Some(manifest) = manifest_for(entry, options.manifest_path.as_deref())? else {
        return Ok(None);
    };
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(&manifest);
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
        command.arg("--filter-platform").arg(target);
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot invoke cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("invalid Cargo metadata: {e}"))?;
    let raw = json["packages"]
        .as_array()
        .ok_or("Cargo metadata has no packages")?;
    let mut packages = Vec::new();
    let mut indices = BTreeMap::new();
    let mut inputs = BTreeMap::new();
    for p in raw {
        let id = string(p, "id")?;
        let name = string(p, "name")?;
        let manifest = PathBuf::from(string(p, "manifest_path")?);
        let directory = manifest.parent().ok_or("manifest has no parent")?;
        let metadata = &p["metadata"]["locus"];
        if let Some(table) = metadata.as_object() {
            for key in table.keys() {
                if !["lib", "targets"].contains(&key.as_str()) {
                    return Err(format!(
                        "{}: unknown package.metadata.locus key `{key}`",
                        manifest.display()
                    ));
                }
            }
        } else if !metadata.is_null() {
            return Err(format!(
                "{}: package.metadata.locus must be a table",
                manifest.display()
            ));
        }
        let library = optional_string(metadata, "lib")?
            .map(|path| source_path(directory, &path))
            .transpose()?;
        let mut targets = Vec::new();
        if !metadata["targets"].is_null() {
            for target in metadata["targets"]
                .as_array()
                .ok_or("locus.targets must be an array of tables")?
            {
                let object = target
                    .as_object()
                    .ok_or("each locus target must be a table")?;
                for key in object.keys() {
                    if !["name", "entry", "rust-module"].contains(&key.as_str()) {
                        return Err(format!("unknown Locus target key `{key}`"));
                    }
                }
                let name = string(target, "name")?;
                identifier(&name)?;
                if targets.iter().any(|t: &Target| t.name == name) {
                    return Err(format!("duplicate Locus target `{name}`"));
                }
                let entry = source_path(directory, &string(target, "entry")?)?;
                let rust_module =
                    optional_string(target, "rust-module")?.unwrap_or_else(|| name.clone());
                for part in rust_module.split("::").filter(|p| !p.is_empty()) {
                    identifier(part)?;
                }
                if !rust_module.is_empty() && rust_module.split("::").any(str::is_empty) {
                    return Err("rust-module must be a relative Rust module path or empty for the crate root".into());
                }
                targets.push(Target {
                    name,
                    entry,
                    rust_module,
                });
            }
        }
        inputs.insert(
            manifest.clone(),
            std::fs::read_to_string(&manifest).map_err(|e| e.to_string())?,
        );
        indices.insert(id.clone(), packages.len());
        packages.push(Package {
            id,
            name,
            manifest,
            library,
            targets,
            dependencies: BTreeMap::new(),
        });
    }
    let root = PathBuf::from(string(&json, "workspace_root")?);
    for path in [root.join("Cargo.toml"), root.join("Cargo.lock")] {
        if path.is_file() {
            inputs.insert(
                path.clone(),
                std::fs::read_to_string(path).map_err(|e| e.to_string())?,
            );
        }
    }
    let nodes = json["resolve"]["nodes"]
        .as_array()
        .ok_or("Cargo did not return a resolved dependency graph")?;
    for node in nodes {
        let Some(&index) = indices.get(&string(node, "id")?) else {
            continue;
        };
        for dep in node["deps"]
            .as_array()
            .ok_or("Cargo node has no dependency list")?
        {
            // Build/dev dependencies are not a source package's ordinary imports.
            if !dep["dep_kinds"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|k| k["kind"].is_null()))
            {
                continue;
            }
            let alias = string(dep, "name")?;
            let id = string(dep, "pkg")?;
            let dependency = *indices
                .get(&id)
                .ok_or("Cargo dependency references a missing package")?;
            if packages[index]
                .dependencies
                .insert(alias.clone(), dependency)
                .is_some_and(|old| old != dependency)
            {
                return Err(format!(
                    "dependency alias `{alias}` resolves to multiple packages; select a target"
                ));
            }
        }
    }
    let host = packages
        .iter()
        .position(|p| p.manifest == manifest)
        .or_else(|| {
            let source = entry.canonicalize().ok()?;
            packages
                .iter()
                .enumerate()
                .filter(|(_, p)| source.starts_with(p.manifest.parent().unwrap()))
                .max_by_key(|(_, p)| p.manifest.components().count())
                .map(|(i, _)| i)
        })
        .ok_or("the manifest is a virtual workspace; select a member package's Cargo.toml")?;
    let selection = serde_json::json!({"resolve":json["resolve"],"target":options.target,"features":options.features,"all_features":options.all_features,"no_default_features":options.no_default_features});
    Ok(Some(Workspace {
        packages,
        host,
        root,
        inputs,
        selection,
    }))
}
fn string(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("expected a string for `{key}`"))
}
fn optional_string(v: &Value, key: &str) -> Result<Option<String>, String> {
    if v[key].is_null() {
        Ok(None)
    } else {
        string(v, key).map(Some)
    }
}
fn source_path(root: &Path, value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(format!(
            "Locus source path `{value}` must stay inside its Cargo package"
        ));
    }
    let joined = root.join(path);
    if let Ok(canonical) = joined.canonicalize()
        && !canonical.starts_with(root.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err(format!(
            "Locus source path `{value}` follows a symlink outside its Cargo package"
        ));
    }
    Ok(joined)
}
pub(crate) fn identifier(value: &str) -> Result<(), String> {
    let mut sources = crate::source::SourceMap::default();
    let id = sources.add("identifier", value);
    let tokens = crate::lexer::lex(sources.get(id));
    if tokens.diagnostics.is_empty()
        && tokens.tokens.len() == 2
        && tokens.tokens[0].kind == crate::lexer::TokenKind::Name
        && !value.starts_with("__locus_")
    {
        Ok(())
    } else {
        Err(format!("`{value}` is not a valid unreserved identifier"))
    }
}
