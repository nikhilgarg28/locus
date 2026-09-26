//! Versioned, compiler-owned view of Rust metadata. Unsupported entities remain
//! present; only normalized safe signatures may cross into execution checking.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const INTERFACE_VERSION: u64 = 1;
pub const RUSTDOC_VERSION: u64 = 56;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Locus,
    Rust { package: String, item: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhysicalType {
    Scalar(String),
    Tuple(Vec<PhysicalType>),
}
impl PhysicalType {
    pub fn spelling(&self) -> String {
        match self {
            Self::Scalar(s) => s.clone(),
            Self::Tuple(fields) => format!(
                "({})",
                fields
                    .iter()
                    .map(|t| format!("{},", t.spelling()))
                    .collect::<String>()
            ),
        }
    }
    pub fn json(&self) -> Value {
        match self {
            Self::Scalar(s) => json!({"scalar":s}),
            Self::Tuple(fields) => {
                json!({"tuple":fields.iter().map(Self::json).collect::<Vec<_>>()})
            }
        }
    }
    pub(crate) fn read(v: &Value) -> Result<Self, String> {
        if let Some(p) = v.get("primitive").and_then(Value::as_str) {
            if matches!(p, "bool" | "usize" | "isize")
                || crate::kernel::MachineInt::from_name(p).is_some()
            {
                return Ok(Self::Scalar(p.into()));
            }
            return Err(format!(
                "Rust primitive `{p}` is not supported by native calls yet"
            ));
        }
        if let Some(fields) = v.get("tuple").and_then(Value::as_array) {
            return fields
                .iter()
                .map(Self::read)
                .collect::<Result<_, _>>()
                .map(Self::Tuple);
        }
        let kind = v
            .as_object()
            .and_then(|o| o.keys().next())
            .map(String::as_str)
            .unwrap_or("malformed");
        let description = match kind {
            "borrowed_ref" => "borrowed reference",
            "resolved_path" => "named type",
            "generic" => "generic type parameter",
            "raw_pointer" => "raw pointer",
            "function_pointer" => "function pointer",
            "dyn_trait" => "trait object",
            "impl_trait" => "opaque return type",
            other => other,
        };
        Err(format!(
            "native calls do not yet support this Rust type: {description}; use `locus import --json` to inspect the full signature"
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub inputs: Vec<PhysicalType>,
    pub output: PhysicalType,
}
impl Signature {
    pub fn json(&self) -> Value {
        json!({"inputs": self.inputs.iter().map(PhysicalType::json).collect::<Vec<_>>(), "output":self.output.json()})
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub trait_interface: Option<super::traits::Interface>,
    pub name: String,
    pub kind: String,
    pub origin: Origin,
    pub signature: Option<Signature>,
    pub unavailable: Option<String>,
    /// Upstream detail is retained for inspection only, never used as a
    /// callable signature. Its schema is explicitly the pinned rustdoc one.
    pub rustdoc: Value,
}
impl Entity {
    fn read(id: &str, v: &Value, package: &str) -> Result<Self, String> {
        let object = v
            .get("inner")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("item {id} has no inner object"))?;
        if object.len() != 1 {
            return Err(format!("item {id} must have one item kind"));
        }
        let (kind, body) = object.iter().next().unwrap();
        let name = v
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let mut entity = Self {
            trait_interface: None,
            name,
            kind: kind.clone(),
            origin: Origin::Rust {
                package: package.into(),
                item: id.into(),
            },
            signature: None,
            unavailable: None,
            rustdoc: v.clone(),
        };
        entity.unavailable=Some(match kind.as_str() {
            "module" => return Ok(entity),
            "function" => match read_signature(body) { Ok(sig)=>{entity.signature=Some(sig);return Ok(entity)}, Err(why)=>why },
            "trait" => "this imported Rust trait is outside the supported interface subset".into(),
            "struct"|"enum"|"union"|"type_alias" => "opaque Rust types are retained, but native type instantiation and methods are not supported yet".into(),
            "constant"|"static" => "native constants and statics are retained, but using their values is not supported yet".into(),
            "macro"|"proc_macro" => "Rust macro expansion is not supported in Locus".into(),
            _ => format!("Rust item kind `{kind}` is retained for inspection but is not supported in Locus yet"),
        });
        Ok(entity)
    }
    pub fn json(&self) -> Value {
        let origin = match &self.origin {
            Origin::Locus => json!({"language":"locus"}),
            Origin::Rust { package, item } => {
                json!({"language":"rust","package":package,"item":item})
            }
        };
        json!({"name":self.name,"kind":self.kind,"origin":origin,"signature":self.signature.as_ref().map(Signature::json),"trait_interface":self.trait_interface.as_ref().map(super::traits::Interface::json),"unavailable":self.unavailable,"rustdoc":self.rustdoc})
    }
}
fn read_signature(body: &Value) -> Result<Signature, String> {
    let header = body
        .get("header")
        .and_then(Value::as_object)
        .ok_or("malformed Rust function header")?;
    for flag in ["is_async", "is_unsafe", "is_const"] {
        if !header.get(flag).is_some_and(Value::is_boolean) {
            return Err(format!(
                "malformed Rust function header: missing boolean {flag}"
            ));
        }
    }
    if header["is_async"] == true {
        return Err(
            "async Rust functions are retained, but Locus has no async/await execution yet".into(),
        );
    }
    if header["is_unsafe"] == true {
        return Err("unsafe Rust functions require an unsafe-call boundary, which Locus does not support yet".into());
    }
    if header.get("abi") != Some(&json!("Rust")) {
        return Err("non-Rust calling conventions are not supported yet".into());
    }
    let generics = &body["generics"];
    if generics["params"].as_array().is_none_or(|p| !p.is_empty())
        || generics["where_predicates"]
            .as_array()
            .is_none_or(|p| !p.is_empty())
    {
        return Err("generic Rust functions and trait bounds are retained, but native specialization is not supported yet".into());
    }
    let sig = &body["sig"];
    if sig["is_c_variadic"] != false {
        return Err("variadic or malformed Rust signatures are not supported".into());
    }
    let inputs = sig["inputs"]
        .as_array()
        .ok_or("malformed Rust parameter list")?
        .iter()
        .map(|arg| {
            let pair = arg
                .as_array()
                .filter(|a| a.len() == 2 && a[0].is_string())
                .ok_or("malformed Rust parameter")?;
            PhysicalType::read(&pair[1])
        })
        .collect::<Result<_, String>>()?;
    let output = if sig.get("output").is_some_and(Value::is_null) {
        PhysicalType::Tuple(vec![])
    } else {
        PhysicalType::read(&sig["output"])?
    };
    Ok(Signature { inputs, output })
}

#[derive(Clone, Debug)]
pub struct Interface {
    pub root: String,
    pub entities: BTreeMap<String, Entity>,
    pub paths: Value,
    pub external_crates: Value,
    pub target: Value,
    pub context: Value,
}
fn id(v: &Value) -> Option<String> {
    v.as_u64().map(|n| n.to_string())
}
impl Interface {
    pub fn read(bytes: &[u8], package: &str, context: Value) -> Result<Self, String> {
        if bytes.len() > crate::limits::MAX_RUSTDOC_BYTES {
            return Err("rustdoc JSON exceeds the 128 MiB import limit".into());
        }
        let v: Value = serde_json::from_slice(bytes)
            .map_err(|e| format!("rustdoc produced invalid JSON: {e}"))?;
        let version = v["format_version"]
            .as_u64()
            .ok_or("rustdoc JSON has no format_version")?;
        if version != RUSTDOC_VERSION {
            return Err(format!(
                "unsupported rustdoc JSON format {version}; this Locus adapter accepts {RUSTDOC_VERSION}. Update Locus for this Rust toolchain; no metadata was imported."
            ));
        }
        if v["includes_private"] != false {
            return Err("rustdoc JSON must exclude private items".into());
        }
        let root = id(&v["root"]).ok_or("rustdoc JSON has no valid root ID")?;
        let raw = v["index"]
            .as_object()
            .ok_or("rustdoc JSON has no item index")?;
        let mut entities = raw
            .iter()
            .map(|(key, value)| {
                if id(&value["id"]).as_ref() != Some(key) {
                    return Err(format!("rustdoc item {key} has inconsistent identity"));
                }
                Ok((key.clone(), Entity::read(key, value, package)?))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let normalized: Vec<_> = entities
            .iter()
            .filter(|(_, e)| e.kind == "trait")
            .map(|(id, e)| (id.clone(), super::traits::Interface::read(e, &entities)))
            .collect();
        for (id, result) in normalized {
            let e = entities.get_mut(&id).unwrap();
            match result {
                Ok(interface) => {
                    e.trait_interface = Some(interface);
                    e.unavailable = None;
                }
                Err(why) => e.unavailable = Some(why),
            }
        }
        // rustdoc uses external references for re-exports. Keep those
        // names rather than silently dropping them or inventing a body.
        if let Some(paths) = v["paths"].as_object() {
            for (key, summary) in paths {
                if entities.contains_key(key) {
                    continue;
                }
                let parts = summary["path"]
                    .as_array()
                    .ok_or("invalid external Rust path")?;
                let name = parts
                    .last()
                    .and_then(Value::as_str)
                    .ok_or("empty external Rust path")?
                    .to_owned();
                let kind = summary["kind"]
                    .as_str()
                    .ok_or("external Rust path has no kind")?
                    .to_owned();
                entities.insert(key.clone(),Entity {
                    name,kind,trait_interface:None,
                    origin:Origin::Rust{package:format!("{}:external:{}",package,summary["crate_id"]),item:key.clone()},
                    signature:None,
                    unavailable:Some(format!("this public re-export refers to another Rust crate ({summary}); its defining metadata is not loaded yet")),
                    rustdoc:json!({"external_reference":summary}),
                });
            }
        }
        if entities.get(&root).is_none_or(|e| e.kind != "module") {
            return Err("rustdoc root is missing or is not a module".into());
        }
        for field in ["paths", "external_crates", "target"] {
            if !v[field].is_object() {
                return Err(format!("rustdoc JSON has no {field} object"));
            }
        }
        Ok(Self {
            root,
            entities,
            paths: v["paths"].clone(),
            external_crates: v["external_crates"].clone(),
            target: v["target"].clone(),
            context,
        })
    }
    pub fn json(&self) -> Value {
        json!({"interface_version":INTERFACE_VERSION,"rustdoc_version":RUSTDOC_VERSION,"root":self.root,"entities":self.entities.iter().map(|(id,e)|(id.clone(),e.json())).collect::<BTreeMap<_,_>>(),"paths":self.paths,"external_crates":self.external_crates,"target":self.target,"context":self.context})
    }
    pub fn children(&self, parent: &str) -> Result<BTreeSet<(String, String)>, String> {
        self.children_inner(parent, &mut BTreeSet::new())
    }
    fn children_inner(
        &self,
        parent: &str,
        seen: &mut BTreeSet<String>,
    ) -> Result<BTreeSet<(String, String)>, String> {
        if !seen.insert(parent.into()) {
            return Ok(BTreeSet::new());
        }
        let entity = self.entities.get(parent).ok_or_else(|| {
            format!("Rust metadata for referenced item {parent} is not available in this crate")
        })?;
        let mut out = BTreeSet::new();
        let body = &entity.rustdoc["inner"]["module"];
        let items = body["items"]
            .as_array()
            .ok_or_else(|| format!("`{}` is a {}, not a Rust module", entity.name, entity.kind))?;
        for child in items {
            let child = id(child).ok_or("invalid Rust module child ID")?;
            let e = self
                .entities
                .get(&child)
                .ok_or_else(|| format!("Rust module references missing item {child}"))?;
            if e.kind == "use" {
                let u = &e.rustdoc["inner"]["use"];
                // Built-in macro re-exports may have no target ID. Retain the
                // declaration as an unavailable entity instead of losing the module.
                let target = id(&u["id"]).unwrap_or_else(|| child.clone());
                if u["is_glob"] == true {
                    for (name, id) in self.children_inner(&target, seen)? {
                        out.insert((name, id));
                    }
                } else {
                    let name = u["name"].as_str().ok_or("invalid Rust re-export name")?;
                    out.insert((name.into(), target));
                }
            } else if e.rustdoc["visibility"] == "public" {
                out.insert((e.name.clone(), child));
            }
        }
        seen.remove(parent);
        Ok(out)
    }
    pub fn find(&self, segments: &[&str]) -> Result<String, String> {
        let mut item = self.root.clone();
        for (index, segment) in segments.iter().enumerate() {
            let children = self.children(&item)?;
            let found: Vec<_> = children
                .iter()
                .filter(|(name, id)| {
                    name == segment
                        && (index + 1 == segments.len()
                            || self.entities.get(id).is_some_and(|e| e.kind == "module"))
                })
                .map(|(_, id)| id.clone())
                .collect();
            item = match found.as_slice() {
                [id] => id.clone(),
                [] => {
                    return Err(format!(
                        "Rust item `{segment}` was not found in `{}` for the selected target/features. Available public names: {}",
                        self.entities[&item].name,
                        children
                            .iter()
                            .map(|(name, _)| name.as_str())
                            .take(20)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                _ => {
                    return Err(format!(
                        "Rust name `{segment}` exists in several namespaces; import its containing module and select the member in a type or value context"
                    ));
                }
            };
        }
        Ok(item)
    }
}
