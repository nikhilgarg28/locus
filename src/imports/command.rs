//! Inspection command; generating an interface never creates a proof contract.
use super::{Extraction, model};
use crate::project::cargo::{self, CargoOptions};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
pub fn run(arguments: &[OsString]) -> Result<String, String> {
    let mut path = None;
    let mut out = None;
    let mut json = false;
    let mut options = CargoOptions::default();
    let mut args = arguments.iter();
    while let Some(arg) = args.next() {
        let value = arg.to_str().ok_or("import arguments must be UTF-8")?;
        match value {
            "--manifest-path" => {
                options.manifest_path = Some(
                    args.next()
                        .ok_or("--manifest-path takes Cargo.toml")?
                        .into(),
                )
            }
            "--offline" => options.offline = true,
            "--locked" => options.locked = true,
            "--no-default-features" => options.no_default_features = true,
            "--all-features" => options.all_features = true,
            "--features" => {
                options.features = args
                    .next()
                    .ok_or("--features takes a comma-separated list")?
                    .to_string_lossy()
                    .split(',')
                    .map(str::to_owned)
                    .collect()
            }
            "--target" => {
                options.target = Some(
                    args.next()
                        .ok_or("--target takes a Rust target")?
                        .to_string_lossy()
                        .into_owned(),
                )
            }
            "--out" => {
                out = Some(PathBuf::from(
                    args.next().ok_or("--out takes a JSON filename")?,
                ))
            }
            "--json" => json = true,
            _ if value.starts_with('-') => return Err(format!("unknown import option `{value}`")),
            _ if path.is_none() => path = Some(value.to_owned()),
            _ => return Err("import accepts one Rust path".into()),
        }
    }
    let path=path.ok_or("usage: locus import dependency::item [--manifest-path Cargo.toml] [--json | --out interface.json]")?;
    let parts: Vec<_> = path.split("::").collect();
    if parts.iter().any(|p| {
        p.is_empty()
            || !p.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
            || p.starts_with(|c: char| c.is_ascii_digit())
    }) {
        return Err("import expects a Rust module/item path, without generic arguments".into());
    }
    Extraction::preflight()?;
    let workspace = cargo::discover(Path::new("."), &options)?
        .ok_or("native imports need a Cargo.toml; pass --manifest-path")?;
    let package = if parts[0] == "crate" {
        workspace.host
    } else {
        *workspace.packages[workspace.host]
            .dependencies
            .get(parts[0])
            .ok_or_else(|| {
                format!(
                    "`{}` is not an active ordinary Cargo dependency; use its Cargo alias",
                    parts[0]
                )
            })?
    };
    let mut extraction = Extraction::new(&workspace, &options)?;
    let native = extraction.get(&workspace, package)?;
    let id = native.interface.find(&parts[1..])?;
    let selected = native
        .interface
        .entities
        .get(&id)
        .ok_or("selected Rust entity has no metadata")?;
    let mut value = native.interface.json();
    value["selection"] = serde_json::json!({"path":path,"id":id});
    let encoded = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    if let Some(output) = out {
        // Do not clobber user-maintained source or an unrelated JSON file.
        if output.exists() {
            let old: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&output).map_err(|e| e.to_string())?)
                    .map_err(|_| "refusing to replace an existing non-interface file")?;
            if old["interface_version"] != model::INTERFACE_VERSION
                || old["selection"]["path"] != path
            {
                return Err(
                    "refusing to replace an unrelated interface; choose a different --out filename"
                        .into(),
                );
            }
        }
        std::fs::write(&output, format!("{encoded}\n"))
            .map_err(|e| format!("cannot write {}: {e}", output.display()))?;
        return Ok(format!(
            "Imported {} Rust entities; saved {}\n",
            native.interface.entities.len(),
            output.display()
        ));
    }
    if json {
        return Ok(format!("{encoded}\n"));
    }
    let mut text = format!(
        "{path}: Rust {}\n{}\n",
        selected.kind,
        selected
            .unavailable
            .as_deref()
            .unwrap_or("physical interface; no behavioral proofs")
    );
    if selected.kind == "module" && selected.unavailable.is_none() {
        for (name, id) in native.interface.children(&id)? {
            let e = &native.interface.entities[&id];
            text.push_str(&format!(
                "  {:<14} {:<24} {}\n",
                e.kind,
                name,
                e.unavailable
                    .as_deref()
                    .unwrap_or(if e.signature.is_some() {
                        "callable"
                    } else {
                        "namespace"
                    })
            ));
        }
    }
    text.push_str(
        "Use --json or --out FILE to inspect all retained items and compiler configuration.\n",
    );
    Ok(text)
}
