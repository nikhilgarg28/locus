//! `let` patterns: binding names to a value and to its projections.

use crate::ast::{self, PatternKind};
use crate::kernel::{HypId, Proof, Term, Type, VarId};
use crate::typed::{Binder, Pattern};

use super::env::{Elab, Env};

impl Env<'_> {
    /// Binds a `let` pattern to a value, as the checker will: a name is
    /// declared with the equation `name == value`, and a tuple pattern binds
    /// each part to a projection.
    pub(super) fn bind_pattern(&mut self, pattern: &ast::Pattern, value: Term) -> Elab<Pattern> {
        match &pattern.kind {
            PatternKind::Wildcard => Ok(Pattern::Wildcard),
            PatternKind::Group(inner) => self.bind_pattern(inner, value),
            PatternKind::Name(name) => {
                let (id, equation) = (VarId::fresh(), HypId::fresh());
                let defined = self.ctx.define_with(id, equation, &value);
                let ty = self.kernel(defined, pattern.span)?;
                if !matches!(ty, Type::Proof(_)) {
                    self.facts.push(super::env::Fact {
                        proof: Proof::hyp(equation),
                        claim: Term::eq(ty.clone(), Term::var(id), value),
                    });
                }
                self.bind(&name.text, id, &ty);
                Ok(Pattern::Bind {
                    binder: Binder {
                        id,
                        name: name.text.clone(),
                        ty,
                    },
                    equation,
                })
            }
            PatternKind::Unit => Ok(Pattern::Tuple(Vec::new())),
            PatternKind::Tuple(patterns) => {
                let ty = self.type_of(&value, pattern.span)?;
                match &ty {
                    Type::Tuple(fields) if fields.len() == patterns.len() => {}
                    _ => {
                        let shown = self.show_type(&ty);
                        let message = format!(
                            "this pattern has {} parts, and the value is `{shown}`",
                            patterns.len()
                        );
                        return self.fail("L0220", message, pattern.span);
                    }
                }
                let mut parts = Vec::new();
                for (index, part) in patterns.iter().enumerate() {
                    parts.push(self.bind_pattern(part, Term::proj(value.clone(), index))?);
                }
                Ok(Pattern::Tuple(parts))
            }
            _ => self.fail(
                "L0290",
                "a `let` pattern is a name, `_`, or a tuple of those for now",
                pattern.span,
            ),
        }
    }
}
