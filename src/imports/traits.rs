//! Normalize the supported Rust trait subset without admitting any proof law.
use super::model::{Entity, PhysicalType};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Physical(PhysicalType),
    SelfType,
    Associated(String),
    Tuple(Vec<Ty>),
    Reference(bool, Box<Ty>),
}
impl Ty {
    fn read(v: &Value, interface: &Value) -> Result<Self, String> {
        if v.get("generic").and_then(Value::as_str) == Some("Self") {
            return Ok(Self::SelfType);
        }
        if let Some(r) = v.get("borrowed_ref") {
            if !r["lifetime"].is_null() {
                return Err("named lifetimes on imported trait members are deferred".into());
            }
            return Ok(Self::Reference(
                r["is_mutable"]
                    .as_bool()
                    .ok_or("malformed reference mutability")?,
                Box::new(Self::read(&r["type"], interface)?),
            ));
        }
        if let Some(q) = v.get("qualified_path")
            && q["self_type"]["generic"] == "Self"
            && (q["trait"].is_null()
                || (q["trait"]["id"] == *interface && q["trait"]["args"].is_null()))
            && q["args"].is_null()
        {
            return Ok(Self::Associated(identifier(&q["name"])?));
        }
        if let Some(ts) = v.get("tuple").and_then(Value::as_array) {
            return ts
                .iter()
                .map(|v| Self::read(v, interface))
                .collect::<Result<_, _>>()
                .map(Self::Tuple);
        }
        PhysicalType::read(v).map(Self::Physical)
    }
    pub fn spelling(&self) -> String {
        match self {
            Self::Physical(t) => t.spelling(),
            Self::SelfType => "Self".into(),
            Self::Associated(n) => format!("Self::{n}"),
            Self::Tuple(ts) => format!(
                "({})",
                ts.iter()
                    .map(|t| format!("{},", t.spelling()))
                    .collect::<String>()
            ),
            Self::Reference(m, t) => format!("&{}{}", if *m { "mut " } else { "" }, t.spelling()),
        }
    }
    fn zero(&self) -> Result<String, String> {
        Ok(match self {
            Self::Physical(PhysicalType::Scalar(s)) if s == "bool" => "false".into(),
            Self::Physical(PhysicalType::Scalar(_)) => "0".into(),
            Self::Associated(_) => "()".into(),
            Self::Tuple(ts) => format!(
                "({})",
                ts.iter()
                    .map(|t| t.zero().map(|s| format!("{s},")))
                    .collect::<Result<String, _>>()?
            ),
            _ => return Err("imported associated constant type is not supported yet".into()),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Member {
    Type(String),
    Constant(String, Ty),
    Function {
        name: String,
        receiver: Option<String>,
        inputs: Vec<Ty>,
        output: Ty,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interface {
    pub members: Vec<Member>,
}
fn identifier(v: &Value) -> Result<String, String> {
    let n = v.as_str().ok_or("missing Rust item name")?;
    crate::project::cargo::identifier(n)
        .map_err(|_| format!("Rust identifier `{n}` has no supported Locus spelling"))?;
    Ok(n.into())
}
fn no_generics(v: &Value) -> Result<(), String> {
    if v["params"].as_array().is_none_or(|v| !v.is_empty())
        || v["where_predicates"]
            .as_array()
            .is_none_or(|v| !v.is_empty())
    {
        return Err(
            "generic Rust traits/methods and bounds are retained but their use is deferred".into(),
        );
    }
    Ok(())
}
impl Interface {
    pub fn read(entity: &Entity, entities: &BTreeMap<String, Entity>) -> Result<Self, String> {
        let b = &entity.rustdoc["inner"]["trait"];
        if b["is_unsafe"] != false || b["is_auto"] != false {
            return Err("unsafe and auto Rust traits require compiler integration and cannot be implemented yet".into());
        }
        no_generics(&b["generics"])?;
        if b["bounds"].as_array().is_none_or(|v| !v.is_empty()) {
            return Err("Rust supertraits are retained but their use is deferred".into());
        }
        let mut members = Vec::new();
        for id in b["items"].as_array().ok_or("malformed trait items")? {
            let id = id.as_u64().ok_or("malformed trait item ID")?.to_string();
            let e = entities.get(&id).ok_or("missing trait member metadata")?;
            let n = identifier(&json!(e.name))?;
            let d = &e.rustdoc["inner"][&e.kind];
            let member = match e.kind.as_str() {
                "assoc_type" => {
                    no_generics(&d["generics"])?;
                    if d["bounds"].as_array().is_none_or(|v| !v.is_empty()) || !d["type"].is_null()
                    {
                        return Err(
                            "associated bounds/defaults on imported traits are deferred".into()
                        );
                    }
                    Member::Type(n)
                }
                "assoc_const" => {
                    let t = Ty::read(&d["type"], &entity.rustdoc["id"])?;
                    t.zero()?;
                    Member::Constant(n, t)
                }
                "function" => {
                    no_generics(&d["generics"])?;
                    let h = &d["header"];
                    if h["is_async"] != false
                        || h["is_unsafe"] != false
                        || h["is_const"] != false
                        || h["abi"] != "Rust"
                    {
                        return Err(format!(
                            "Rust trait method `{n}` is async, unsafe, const or uses an unsupported ABI"
                        ));
                    }
                    let sig = &d["sig"];
                    if sig["is_c_variadic"] != false {
                        return Err("variadic trait signatures are unsupported".into());
                    }
                    let mut receiver = None;
                    let mut inputs = Vec::new();
                    for (i, arg) in sig["inputs"]
                        .as_array()
                        .ok_or("malformed trait parameters")?
                        .iter()
                        .enumerate()
                    {
                        let t = Ty::read(&arg[1], &entity.rustdoc["id"])?;
                        if i == 0 && arg[0] == "self" {
                            receiver = Some(
                                match t {
                                    Ty::SelfType => "self",
                                    Ty::Reference(false, t) if *t == Ty::SelfType => "&self",
                                    Ty::Reference(true, t) if *t == Ty::SelfType => "&mut self",
                                    _ => {
                                        return Err("arbitrary Rust self types are deferred".into());
                                    }
                                }
                                .into(),
                            );
                        } else {
                            inputs.push(t);
                        }
                    }
                    let raw_output = sig.get("output").ok_or("malformed trait output")?;
                    let output = if raw_output.is_null() {
                        Ty::Tuple(vec![])
                    } else {
                        Ty::read(raw_output, &entity.rustdoc["id"])?
                    };
                    Member::Function {
                        name: n,
                        receiver,
                        inputs,
                        output,
                    }
                }
                _ => return Err(format!("unsupported Rust trait member kind `{}`", e.kind)),
            };
            members.push(member);
        }
        Ok(Self { members })
    }
    /// Rust probe bodies are never executed. Locus declarations have no bodies:
    /// a local implementation must supply even Rust's defaulted methods.
    pub fn declarations(&self, probe: bool) -> String {
        let mut out = String::new();
        for m in &self.members {
            let s = match m {
                Member::Type(n) => format!("type {n}{};", if probe { " = ()" } else { "" }),
                Member::Constant(n, t) => format!(
                    "const {n}: {}{};",
                    t.spelling(),
                    if probe {
                        format!(" = {}", t.zero().expect("validated constant"))
                    } else {
                        String::new()
                    }
                ),
                Member::Function {
                    name,
                    receiver,
                    inputs,
                    output,
                } => {
                    let mut params = receiver.iter().cloned().collect::<Vec<_>>();
                    params.extend(
                        inputs
                            .iter()
                            .enumerate()
                            .map(|(i, t)| format!("arg{i}: {}", t.spelling())),
                    );
                    format!(
                        "fn {name}({})->{}{}",
                        params.join(","),
                        output.spelling(),
                        if probe { "{panic!()}" } else { ";" }
                    )
                }
            };
            out.push_str(&s);
            out.push('\n');
        }
        out
    }
    pub fn json(&self) -> Value {
        json!({"declarations":self.declarations(false)})
    }
}
