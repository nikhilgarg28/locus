//! Kernel terms and types in surface syntax, for diagnostics. A term is
//! shown with the names the programmer wrote; the result of a call is shown
//! as the call.

use crate::kernel::{CmpOp, Integer, Mode, Op, Prim, Term, Type, infer_term};

use super::env::Env;

/// Binding strength, loosest first.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Level {
    Implies,
    Or,
    And,
    Compare,
    Sum,
    Product,
    Prefix,
    Postfix,
}

/// A machine literal, or the view of one, as its type and number.
fn literal_of(term: &Term) -> Option<(crate::kernel::MachineInt, Integer)> {
    match term {
        Term::Prim(Prim::View(_), operand) => match operand.as_slice() {
            [inner] => inner.machine_value(),
            _ => None,
        },
        other => other.machine_value(),
    }
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
            Type::Instance(base, args) => {
                let base = self.type_at(base, bound);
                let args: Vec<_> = args
                    .iter()
                    .map(|t| self.term_at(t, Level::Implies, bound))
                    .collect();
                format!("{base}[{}]", args.join(", "))
            }
            Type::Boxed(element) => format!("Box<{}>", self.type_at(element, bound)),
            Type::Buffer(element) => format!("Buffer<{}>", self.type_at(element, bound)),
            Type::Bool => "bool".into(),
            Type::U8 => "u8".into(),
            Type::Int => "Int".into(),
            Type::Machine(ty) => ty.name().into(),
            Type::Prop => "Prop".into(),
            Type::Proof(claim) => match &**claim {
                Term::PropApp(..) | Term::Call(..) | Term::Free(_) | Term::Bound(_)
                    if !self.is_operator(claim) =>
                {
                    format!("@{}", self.term_at(claim, Level::Postfix, bound))
                }
                claim => format!("@({})", self.term_at(claim, Level::Implies, bound)),
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
                format!("fn{shown} -> {result}")
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
        // Comparisons do not chain, so both sides bind tighter; the
        // arithmetic operators associate to the left.
        let (l, r) = match level {
            Level::Implies => (Level::Or, Level::Implies),
            Level::Or => (Level::Or, Level::And),
            Level::And => (Level::And, Level::Compare),
            Level::Compare => (Level::Sum, Level::Sum),
            Level::Sum => (Level::Sum, Level::Product),
            Level::Product => (Level::Product, Level::Prefix),
            _ => (Level::Prefix, Level::Prefix),
        };
        // A comparison of two literals names the type on the first, as the
        // source must, since two bare literals would be `i32`.
        let left_text = match (level, literal_of(left), literal_of(right)) {
            (Level::Compare, Some((ty, value)), Some(_)) => format!("{value}{}", ty.name()),
            _ => self.term_at(left, l, bound),
        };
        let text = format!("{left_text} {op} {}", self.term_at(right, r, bound));
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
            Term::Instance(value, _) => self.term_at(value, at, bound),
            Term::Boxed(value) => format!("box({})", self.term_at(value, Level::Implies, bound)),
            Term::Buffer { op, arguments, .. } => format!(
                "buffer::{op:?}({})",
                arguments
                    .iter()
                    .map(|arg| self.term_at(arg, Level::Implies, bound))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Term::Free(id) => self.labels.get(id).cloned().unwrap_or_else(|| "_".into()),
            Term::Bound(index) => bound
                .len()
                .checked_sub(1 + *index as usize)
                .and_then(|position| bound.get(position).cloned())
                .unwrap_or_else(|| "_".into()),
            Term::Bool(value) => value.to_string(),
            Term::U8(value) => value.to_string(),
            Term::Int(value) => format!("{value}"),
            // A literal of a type other than `u8` shows its type, so that
            // `0i32 <= 3i32` is not mistaken for a claim about bytes.
            Term::Machine(ty, value) => format!("{value}{}", ty.name()),
            Term::Prim(prim, operands) => match (prim, operands.as_slice()) {
                (Prim::Cmp(CmpOp::Eq, _) | Prim::IntCmp(CmpOp::Eq), [a, b]) => {
                    self.binary("==", Level::Compare, at, a, b, bound)
                }
                (Prim::Cmp(CmpOp::Lt, _) | Prim::IntCmp(CmpOp::Lt), [a, b]) => {
                    self.binary("<", Level::Compare, at, a, b, bound)
                }
                (Prim::Cmp(CmpOp::Le, _) | Prim::IntCmp(CmpOp::Le), [a, b]) => {
                    self.binary("<=", Level::Compare, at, a, b, bound)
                }
                // `a < b` over `Int` is `a + 1 <= b`; a view of a machine
                // value is written as it was, without its cast, so that
                // `x <= 3` reads back as `x <= 3`.
                (Prim::IntLe, [Term::Prim(Prim::IntAdd, sum), b]) if matches!(sum.as_slice(), [_, Term::Int(one)] if *one == Integer::from(1i64)) => {
                    self.binary("<", Level::Compare, at, &sum[0], b, bound)
                }
                (Prim::IntLe, [a, b]) => self.binary("<=", Level::Compare, at, a, b, bound),
                // The view of an operator's result keeps its cast, since
                // the operator and its counterpart on `Int` read the same:
                // `(a + b) as Int == a + b` says what a claim about the
                // wrapped sum says.
                (Prim::View(_), [x @ Term::Prim(Prim::Op(op, _), _)])
                    if !op.name().starts_with("wrapping_") =>
                {
                    format!("({}) as Int", self.term_at(x, Level::Implies, bound))
                }
                (Prim::View(_), [x]) => self.term_at(x, at, bound),
                (Prim::Wrap(ty), [n]) => {
                    format!(
                        "({} as {})",
                        self.term_at(n, Level::Implies, bound),
                        ty.name()
                    )
                }
                (Prim::Cast(_, to), [x]) => {
                    format!(
                        "({} as {})",
                        self.term_at(x, Level::Implies, bound),
                        to.name()
                    )
                }
                (Prim::IntAdd, [a, b]) => self.binary("+", Level::Sum, at, a, b, bound),
                (Prim::IntSub, [a, b]) => self.binary("-", Level::Sum, at, a, b, bound),
                (Prim::IntMul, [a, b]) => self.binary("*", Level::Product, at, a, b, bound),
                (Prim::IntDiv, [a, b]) => self.binary("/", Level::Product, at, a, b, bound),
                (Prim::IntRem, [a, b]) => self.binary("%", Level::Product, at, a, b, bound),
                (Prim::IntNeg, [a]) => format!("-{}", self.term_at(a, Level::Prefix, bound)),
                // The operators at a machine type read as written; the
                // wrapping methods fall through to the method form below.
                (Prim::Op(Op::Add, _), [a, b]) => self.binary("+", Level::Sum, at, a, b, bound),
                (Prim::Op(Op::Sub, _), [a, b]) => self.binary("-", Level::Sum, at, a, b, bound),
                (Prim::Op(Op::Mul, _), [a, b]) => self.binary("*", Level::Product, at, a, b, bound),
                (Prim::Op(Op::Div, _), [a, b]) => self.binary("/", Level::Product, at, a, b, bound),
                (Prim::Op(Op::Rem, _), [a, b]) => self.binary("%", Level::Product, at, a, b, bound),
                (Prim::Op(Op::Neg, _), [a]) => {
                    format!("-{}", self.term_at(a, Level::Prefix, bound))
                }
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
            Term::Fn(id) => self
                .fn_by_id(*id)
                .map_or_else(|| "lemma".to_string(), |info| info.name.clone()),
            Term::Call(callee, arguments) => format!(
                "{}({})",
                self.term_at(callee, Level::Postfix, bound),
                self.list(arguments, bound)
            ),
            Term::Variant(id, index, payload) => {
                let Some(info) = self.enum_by_id(*id) else {
                    return format!("enum::{index}({})", self.list(payload, bound));
                };
                let variant = &info.variants[*index];
                let name = format!("{}::{}", info.name, variant.name);
                if variant.named {
                    let fields: Vec<String> = variant
                        .payload
                        .iter()
                        .zip(payload)
                        .map(|(field, value)| {
                            format!(
                                "{}: {}",
                                field.name,
                                self.term_at(value, Level::Implies, bound)
                            )
                        })
                        .collect();
                    format!("{name} {{ {} }}", fields.join(", "))
                } else if payload.is_empty() {
                    name
                } else {
                    format!("{name}({})", self.list(payload, bound))
                }
            }
            Term::PropApp(id, arguments) => match arguments.as_slice() {
                [Term::Lambda { params, body, .. }]
                    if params.len() == 1
                        && self
                            .quantifiers
                            .iter()
                            .any(|pair| pair.forall_id() == *id || pair.exists_id() == *id) =>
                {
                    let word = if self.quantifiers.iter().any(|pair| pair.forall_id() == *id) {
                        "forall"
                    } else {
                        "exists"
                    };
                    let name = format!("x{}", bound.len());
                    let ty = self.type_at(&params[0], bound);
                    bound.push(name.clone());
                    let body = self.term_at(body, Level::Implies, bound);
                    bound.pop();
                    format!("{word} ({name}: {ty}) {{ {body} }}")
                }
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
            Term::Lambda { .. } => "logic |...| { ... }".into(),
        }
    }
}
