//! The hole solver's use of the proofs file, `crate::store`.
//!
//! Before the tiers run for an obligation, the store is asked for a proof
//! under the obligation's key; a stored proof is read back over the current
//! context and checked by the kernel against the current claim, exactly as
//! a found proof is, and is used only when the kernel accepts it. After the
//! tiers find a proof, it is recorded. When no store is installed, which is
//! the case for every test that calls `elaborate` directly, nothing here
//! does anything and every obligation is searched.

use crate::diagnostic::Diagnostic;
use crate::kernel::{Proof, Term, check_proof};
use crate::source::Span;
use crate::store::{self, Key, Names, text};
use crate::typed::FnRef;

use super::env::{Env, Global};

impl Env<'_> {
    /// Every declaration a term may mention, under the name the source
    /// calls it by: the theory's lemmas and the items declared so far,
    /// built-in propositions included.
    fn definition_names(&self) -> Names {
        let mut names = Names::new();
        for (name, id) in self.theory.lemma_names() {
            names.function(name, id);
        }
        for (name, global) in &self.types {
            match global {
                Global::Struct(info) => names.structure(name, info.id),
                Global::Enum(info) => names.enumeration(name, info.id),
                Global::Prop(info) => names.proposition(name, info.id),
                Global::Fn(_) => {}
            }
        }
        for (name, global) in &self.values {
            if let Global::Fn(info) = global
                && let FnRef::Math(id) = info.reference
            {
                names.function(name, id);
            }
        }
        names
    }

    /// The key of the obligation and the names it was printed with, when a
    /// store is installed and the claim can be written.
    fn obligation_key(&self, goal: &Term) -> Option<(Key, Names)> {
        store::with_current(|_| ())?;
        let names = self.definition_names();
        let key = text::print_key(goal, &self.ctx, &names).ok()?;
        Some((Key::of(&key), names))
    }

    /// A proof of `goal`, accepted by the kernel: the stored one when the
    /// store has one that checks, else what `search` finds, if the store
    /// allows a search, which is then recorded. The tier is `stored` for a
    /// stored proof.
    pub(super) fn stored_or(
        &mut self,
        goal: &Term,
        search: impl FnOnce(&mut Self) -> Option<(Proof, &'static str)>,
    ) -> Option<(Proof, &'static str)> {
        let Some((key, names)) = self.obligation_key(goal) else {
            // No store: search as before. A claim the text cannot hold is
            // searched too, unless the store forbids it.
            return self.may_search().then(|| search(self)).flatten();
        };
        let found = self.stored_or_with(goal, key, &names, search);
        store::with_current(|store| store.set_names(names));
        found
    }

    fn stored_or_with(
        &mut self,
        goal: &Term,
        key: Key,
        names: &Names,
        search: impl FnOnce(&mut Self) -> Option<(Proof, &'static str)>,
    ) -> Option<(Proof, &'static str)> {
        let item = self.item_name.clone();
        if let Some(stored) = store::with_current(|store| store.lookup(key, &item)).flatten() {
            let accepted = text::parse_proof(&stored, &self.ctx, names)
                .ok()
                .filter(|proof| check_proof(&mut self.ctx, proof, goal).is_ok());
            store::with_current(|store| match accepted {
                Some(_) => store.accept(key),
                None => store.refuse(key),
            });
            if let Some(proof) = accepted {
                return Some((proof, "stored"));
            }
        }
        if !self.may_search() {
            return None;
        }
        let (proof, tier) = search(self)?;
        if check_proof(&mut self.ctx, &proof, goal).is_err() {
            return Some((proof, tier));
        }
        match text::print_proof(&proof, &self.ctx, names) {
            Ok(text) => store::with_current(|store| store.record(key, text)),
            Err(_) => store::with_current(|store| store.unprintable()),
        };
        Some((proof, tier))
    }

    fn may_search(&self) -> bool {
        store::with_current(|store| store.may_search()).unwrap_or(true)
    }

    /// Under `--locked`, reports that the obligation has no stored proof,
    /// naming the function, the line, and the claim, and says so; a miss is
    /// then an error, not something to search for.
    pub(super) fn locked_miss(&mut self, goal: &Term, span: Span) -> bool {
        if !store::with_current(|store| store.is_locked()).unwrap_or(false) {
            return false;
        }
        let claim = self.show(goal);
        let line = self
            .source
            .line_column(span.start)
            .map_or(0, |(line, _)| line);
        self.diagnostics.push(
            Diagnostic::error(
                "L0230",
                format!(
                    "`{}` needs a proof of `{claim}` at line {line}, and the proofs file has none",
                    self.item_name
                ),
                span,
            )
            .note("`--locked` never searches; run `locus check` without it to find the proof and store it"),
        );
        true
    }
}
