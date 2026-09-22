//! `let` patterns: binding names to a value and to its projections.
//!
//! A tuple pattern opens a dependent product over its own names: in
//! `let (next, still) = step(...)`, `still` is typed over `next`, not over
//! `step(...).0` (`typed::opened_part`, which lowering uses too). A name
//! with `mut` is a mutable binding, and one whose type mentions other
//! mutable bindings is tracked evidence (`mutation.rs`).

use crate::ast::{self, PatternKind};
use crate::kernel::{HypId, Proof, Term, Type, VarId};
use crate::typed::{Binder, Named, Pattern, opened_part, opened_type};

use super::env::{Elab, Env, Fact};

impl Env<'_> {
    /// Binds a `let` pattern to a value, as the checker will: a name is
    /// declared with the equation `name == value`, and a tuple pattern binds
    /// each part to a projection, stated over the parts before it.
    pub(super) fn bind_pattern(&mut self, pattern: &ast::Pattern, value: Term) -> Elab<Pattern> {
        self.bind_pattern_in(pattern, value, &mut Vec::new())
    }

    fn bind_pattern_in(
        &mut self,
        pattern: &ast::Pattern,
        value: Term,
        earlier: &mut Vec<Named>,
    ) -> Elab<Pattern> {
        match &pattern.kind {
            PatternKind::Wildcard => Ok(Pattern::Wildcard),
            PatternKind::Group(inner) => self.bind_pattern_in(inner, value, earlier),
            PatternKind::Name { name, mutable } => {
                let (id, equation) = (VarId::fresh(), HypId::fresh());
                let over_projections = self.type_of(&value, pattern.span)?;
                let opened = opened_type(&over_projections, earlier);
                let value = opened_part(&value, &opened, earlier);
                let defined = self.ctx.define_with(id, equation, &value);
                let ty = self.kernel(defined, pattern.span)?;
                if !matches!(ty, Type::Proof(_)) {
                    self.facts.push(Fact::definition(
                        Proof::hyp(equation),
                        Term::eq(ty.clone(), Term::var(id), value.clone()),
                    ));
                    earlier.push(Named {
                        id,
                        equation,
                        ty: ty.clone(),
                        value,
                    });
                }
                self.bind(&name.text, id, &ty);
                if *mutable {
                    self.make_mutable(id);
                }
                Ok(Pattern::Bind {
                    binder: Binder {
                        id,
                        name: name.text.clone(),
                        ty,
                    },
                    equation,
                    mutable: *mutable,
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
                    let projection = Term::proj(value.clone(), index);
                    parts.push(self.bind_pattern_in(part, projection, earlier)?);
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
