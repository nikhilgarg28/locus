//! Kernel terms and types in surface syntax, for diagnostics. A term is
//! shown with the names the programmer wrote; the result of a call is shown
//! as the call.

use crate::kernel::{Mode, Prim, Term, Type, infer_term};

use super::env::Env;

/// Binding strength, loosest first.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Level {
    Implies,
    Or,
    And,
    Compare,
    Prefix,
    Postfix,
}

impl Env<'_> {
    pub fn show_type(&mut self, ty: &Type) -> String {
        self.type_at(ty, &mut Vec::new())
    }

    pub fn show(&mut self, term: &Term) -> String {
        self.term_at(term, Level::Implies, &mut Vec::new())
    }

    fn type_at(&mut self, ty: &Type, bound: &mut Vec<String>) -> String {
        match ty {
            Type::Bool => "bool".into(),
            Type::U8 => "u8".into(),
            Type::Nat => "Nat".into(),
            Type::Int => "Int".into(),
            Type::Prop => "Prop".into(),
            Type::Proof(claim) => match &**claim {
                Term::PropApp(..) | Term::Call(..) | Term::Free(_) | Term::Bound(_)
                    if !self.is_operator(claim) =>
                {
                    format!("@{}", self.term_at(claim, Level::Postfix, bound))
                }
                claim => format!("@[{}]", self.term_at(claim, Level::Implies, bound)),
            },
            Type::Tuple(fields) => self.telescope_at(fields, bound, true),
            Type::Struct(id) => self
                .struct_by_id(*id)
                .map_or_else(|| "struct".into(), |info| info.name.clone()),
            Type::Enum(id) => self
                .enum_by_id(*id)
                .map_or_else(|| "enum".into(), |info| info.name.clone()),
            Type::Fn(params, result) => {
                let depth = bound.len();
                let shown = self.telescope_at(params, bound, false);
                // The result is under all the parameters.
                for index in 0..params.len() {
                    bound.push(format!("x{}", depth + index));
                }
                let result = self.type_at(result, bound);
                bound.truncate(depth);
                format!("math fn{shown} -> {result}")
            }
        }
    }

    fn telescope_at(&mut self, fields: &[Type], bound: &mut Vec<String>, tuple: bool) -> String {
        let depth = bound.len();
        let mut shown = Vec::new();
        for (index, field) in fields.iter().enumerate() {
            let name = format!("x{}", depth + index);
            let text = self.type_at(field, bound);
            // Name a field only when a later one speaks of it.
            let mentioned = fields[index + 1..]
                .iter()
                .enumerate()
                .any(|(offset, later)| format!("{later:?}").contains(&format!("Bound({offset})")));
            shown.push(if mentioned {
                format!("{name}: {text}")
            } else {
                text
            });
            bound.push(name);
        }
        bound.truncate(depth);
        if tuple && shown.len() == 1 {
            format!("({},)", shown[0])
        } else {
            format!("({})", shown.join(", "))
        }
    }

    fn is_operator(&self, term: &Term) -> bool {
        match term {
            Term::Call(callee, _) => {
                matches!(&**callee, Term::Fn(id) if *id == self.prelude.u8_le || *id == self.prelude.u8_lt || *id == self.prelude.nat_le || *id == self.prelude.nat_lt)
            }
            Term::PropApp(id, _) => [
                self.prelude.and,
                self.prelude.or,
                self.prelude.truth,
                self.prelude.falsehood,
            ]
            .contains(id),
            _ => false,
        }
    }

    fn binary(
        &mut self,
        op: &str,
        level: Level,
        at: Level,
        left: &Term,
        right: &Term,
        bound: &mut Vec<String>,
    ) -> String {
        // Comparisons do not chain, so both sides bind tighter.
        let (l, r) = match level {
            Level::Implies => (Level::Or, Level::Implies),
            Level::Or => (Level::Or, Level::And),
            Level::And => (Level::And, Level::Compare),
            _ => (Level::Prefix, Level::Prefix),
        };
        let text = format!(
            "{} {op} {}",
            self.term_at(left, l, bound),
            self.term_at(right, r, bound)
        );
        if at > level {
            format!("({text})")
        } else {
            text
        }
    }

    fn list(&mut self, terms: &[Term], bound: &mut Vec<String>) -> String {
        terms
            .iter()
            .map(|term| self.term_at(term, Level::Implies, bound))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn term_at(&mut self, term: &Term, at: Level, bound: &mut Vec<String>) -> String {
        let prelude = self.prelude;
        match term {
            Term::Free(id) => self.labels.get(id).cloned().unwrap_or_else(|| "_".into()),
            Term::Bound(index) => bound
                .len()
                .checked_sub(1 + *index as usize)
                .and_then(|position| bound.get(position).cloned())
                .unwrap_or_else(|| "_".into()),
            Term::Bool(value) => value.to_string(),
            Term::U8(value) => value.to_string(),
            Term::Nat(value) => format!("{value}"),
            Term::Int(value) => format!("{value}"),
            Term::Prim(prim, operands) => match (prim, operands.as_slice()) {
                (Prim::U8Eq, [a, b]) => self.binary("==", Level::Compare, at, a, b, bound),
                (Prim::U8Lt, [a, b]) => self.binary("<", Level::Compare, at, a, b, bound),
                (Prim::U8Le, [a, b]) => self.binary("<=", Level::Compare, at, a, b, bound),
                (_, [receiver, rest @ ..]) => format!(
                    "{}.{}({})",
                    self.term_at(receiver, Level::Postfix, bound),
                    prim.name(),
                    self.list(rest, bound)
                ),
                _ => prim.name().to_string(),
            },
            // A fact about a test reads as the test.
            Term::Eq(Type::Bool, tested, outcome) if matches!(**outcome, Term::Bool(_)) => {
                if **outcome == Term::Bool(true) {
                    self.term_at(tested, at, bound)
                } else {
                    format!("!({})", self.term_at(tested, Level::Implies, bound))
                }
            }
            Term::Eq(_, left, right) => self.binary("==", Level::Compare, at, left, right, bound),
            Term::Implies(premise, conclusion) if **conclusion == prelude.falsehood_prop() => {
                match &**premise {
                    Term::Eq(ty, left, right) if !matches!(ty, Type::Bool) => {
                        self.binary("!=", Level::Compare, at, left, right, bound)
                    }
                    premise => format!("!({})", self.term_at(premise, Level::Implies, bound)),
                }
            }
            Term::Implies(premise, conclusion) => {
                self.binary("=>", Level::Implies, at, premise, conclusion, bound)
            }
            Term::Forall(ty, body) | Term::Exists(ty, body) => {
                let word = if matches!(term, Term::Forall(..)) {
                    "forall"
                } else {
                    "exists"
                };
                let name = format!("x{}", bound.len());
                let ty = self.type_at(ty, bound);
                bound.push(name.clone());
                let body = self.term_at(body, Level::Implies, bound);
                bound.pop();
                format!("{word} ({name}: {ty}) {{ {body} }}")
            }
            Term::Tuple(_, values) if values.len() == 1 => {
                format!("({},)", self.list(values, bound))
            }
            Term::Tuple(_, values) => format!("({})", self.list(values, bound)),
            Term::Struct(id, values) => match self.struct_by_id(*id) {
                Some(info) => {
                    let fields: Vec<String> = info
                        .fields
                        .iter()
                        .zip(values)
                        .map(|(field, value)| {
                            format!(
                                "{}: {}",
                                field.name,
                                self.term_at(value, Level::Implies, bound)
                            )
                        })
                        .collect();
                    format!("{} {{ {} }}", info.name, fields.join(", "))
                }
                None => format!("struct {{ {} }}", self.list(values, bound)),
            },
            Term::Proj(target, index) => {
                let field = match infer_term(&mut self.ctx, target, Mode::Logical) {
                    Ok(Type::Struct(id)) => self
                        .struct_by_id(id)
                        .and_then(|info| info.fields.get(*index).map(|field| field.name.clone())),
                    _ => None,
                };
                format!(
                    "{}.{}",
                    self.term_at(target, Level::Postfix, bound),
                    field.unwrap_or_else(|| index.to_string())
                )
            }
            Term::Proof(_) => "_".into(),
            Term::Fn(id) => {
                let named = [
                    (prelude.u8_le, "u8_le"),
                    (prelude.u8_lt, "u8_lt"),
                    (prelude.nat_le, "nat_le"),
                    (prelude.nat_lt, "nat_lt"),
                ];
                match self.fn_by_id(*id) {
                    Some(info) => info.name.clone(),
                    None => named
                        .iter()
                        .find(|(known, _)| known == id)
                        .map_or_else(|| "lemma".to_string(), |(_, name)| name.to_string()),
                }
            }
            Term::Call(callee, arguments) => match (&**callee, arguments.as_slice()) {
                (Term::Fn(id), [a, b]) if *id == prelude.u8_le || *id == prelude.nat_le => {
                    self.binary("<=", Level::Compare, at, a, b, bound)
                }
                (Term::Fn(id), [a, b]) if *id == prelude.u8_lt || *id == prelude.nat_lt => {
                    self.binary("<", Level::Compare, at, a, b, bound)
                }
                _ => format!(
                    "{}({})",
                    self.term_at(callee, Level::Postfix, bound),
                    self.list(arguments, bound)
                ),
            },
            Term::Variant(id, index, payload) => {
                let name = self.enum_by_id(*id).map_or_else(
                    || format!("enum::{index}"),
                    |info| format!("{}::{}", info.name, info.variants[*index].0),
                );
                if payload.is_empty() {
                    name
                } else {
                    format!("{name}({})", self.list(payload, bound))
                }
            }
            Term::PropApp(id, arguments) => match arguments.as_slice() {
                [] if *id == prelude.truth => "true".into(),
                [] if *id == prelude.falsehood => "false".into(),
                [p, q] if *id == prelude.and => self.binary("&&", Level::And, at, p, q, bound),
                [p, q] if *id == prelude.or => self.binary("||", Level::Or, at, p, q, bound),
                _ => {
                    let name = self
                        .prop_by_id(*id)
                        .map_or_else(|| "prop".into(), |info| info.name.clone());
                    if arguments.is_empty() {
                        return name;
                    }
                    format!("{name}({})", self.list(arguments, bound))
                }
            },
            Term::Case {
                scrutinee, arms, ..
            } => {
                let scrutinee_text = self.term_at(scrutinee, Level::Implies, bound);
                match (
                    arms.as_slice(),
                    infer_term(&mut self.ctx, scrutinee, Mode::Logical),
                ) {
                    ([otherwise, then], Ok(Type::Bool) | Err(_))
                        if then.binders == 0 && otherwise.binders == 0 =>
                    {
                        format!(
                            "if {scrutinee_text} {{ {} }} else {{ {} }}",
                            self.term_at(&then.body, Level::Implies, bound),
                            self.term_at(&otherwise.body, Level::Implies, bound)
                        )
                    }
                    _ => format!("match {scrutinee_text} {{ ... }}"),
                }
            }
            Term::Absurd(..) => "unreachable".into(),
            Term::For(_) => "for ... { ... }".into(),
        }
    }
}
