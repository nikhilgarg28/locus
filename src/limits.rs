//! Counted implementation budgets. Their complete inventory is checked against
//! Kernel contract §2.40; changing a value requires the documented row to change.
use crate::diagnostic::Diagnostic;
use crate::source::Span;

#[derive(Clone, Copy, Debug)]
pub struct Limit {
    pub name: &'static str,
    pub value: u64,
    pub scope: &'static str,
    pub failure: &'static str,
}

macro_rules! limits {
    ($( $name:ident : $ty:ty = $value:expr; $scope:literal; $failure:literal; )*) => {
        $(pub const $name: $ty = $value;)*
        pub const ALL: &[Limit] = &[$(Limit { name: stringify!($name), value: $name as u64, scope: $scope, failure: $failure },)*];
    };
}

limits! {
    MAX_RUSTDOC_BYTES: usize = 134217728; "one rustdoc JSON interface"; "L0512; no interface accepted";
    MAX_RUSTC_CAPTURE_BYTES: usize = 16777216; "one captured compiler invocation"; "L0512; no interface accepted";
    MAX_RUSTC_CAPTURE_STRINGS: usize = 100000; "one captured argument or environment string list"; "L0512; no interface accepted";

    MAX_SOURCE_BYTES: usize = 64 << 20; "one source unit, including assembled libraries"; "L0010; no tokenization or elaboration";
    MAX_DIAGNOSTICS: usize = 1000; "retained diagnostics in one compilation phase"; "L0011 replaces the final diagnostic; explicit suppression notice";
    MAX_DIAGNOSTIC_FACTS: usize = 6; "distinct facts printed for one failed obligation"; "explicit omission note; proof search still receives all facts";
    MAX_PARSER_DEPTH: usize = 64; "nested guarded parser productions"; "L0108; parsing rejects the construct";
    MAX_EXPRESSION_CHAIN: usize = 128; "postfix/binary expression chain and cast lookahead"; "L0108; parsing rejects an excessive chain";
    MAX_KERNEL_DEPTH: usize = 256; "kernel input terms, types and proofs; stored-text nesting"; "KernelError::TooDeep or named text parse error";
    MAX_EVALUATION_STEPS: usize = 2_000_000; "one kernel evaluation"; "KernelError::EvaluationStepLimit";
    MAX_EVALUATION_DEPTH: usize = 200; "nested kernel evaluation calls"; "KernelError::EvaluationTooDeep";
    MAX_DERIVED_STEPS: usize = 10_000; "one derived fold/unfold construction"; "KernelError::StepLimit";
    MAX_LINEAR_PAIRS: usize = 256; "certificate pairs; default arithmetic pairs"; "LinearError::TooManyPairs or arithmetic Budget(pairs)";
    MAX_LINEAR_ATOMS: usize = 256; "linear certificate atoms; default arithmetic atoms"; "LinearError::TooManyAtoms or arithmetic Budget(atoms)";
    MAX_LINEAR_BITS: usize = 512; "linear certificate literal magnitude bits"; "LinearError::LiteralTooLarge";
    MAX_ARITHMETIC_ELIMINATIONS: usize = 1024; "default arithmetic eliminations shared by nested runs"; "arithmetic Budget(eliminations), surfaced in L0230 notes";
    MAX_ARITHMETIC_DERIVED: usize = 16384; "default arithmetic derived constraints"; "arithmetic Budget(derived), surfaced in L0230 notes";
    MAX_ARITHMETIC_BITS: usize = 256; "default arithmetic coefficient/multiplier bits"; "arithmetic Budget(bits), surfaced in L0230 notes";
    MAX_ARITHMETIC_DEPTH: usize = 4; "default nested arithmetic premise searches"; "arithmetic Budget(depth) if search remains unresolved";
    MAX_ARITHMETIC_BRANCHES: usize = 64; "default arithmetic case splits"; "arithmetic Budget(branches), surfaced in L0230 notes";
    MAX_NORMALIZATION_STEPS: usize = 400; "elaborator normalization and explanatory unfolding"; "unresolved proof L0230 names exhaustion; checked partial normalization is valid";
    MAX_EXPLANATION_DEPTH: usize = 8; "diagnostic structural proof exploration"; "diagnostic note names incomplete bounded explanation";
    MAX_GENERIC_INSTANCES: usize = 256; "distinct source generic instances per unit"; "L0281 names the instance ceiling";
    MAX_GENERIC_TYPE_DEPTH: usize = 64; "source generic type expansion nesting"; "L0281 names the nesting ceiling";
    MAX_PROOF_FILE_BYTES: usize = 64 << 20; "one stored-proof file"; "store reader returns a named size error";
    MAX_PROOF_TEXT_BYTES: usize = 4 << 20; "one stored term or proof text"; "ParseError names text-size ceiling";
    MAX_PROOF_EXPANDED_NODES: usize = 1 << 20; "one expanded stored-proof tree and total retained named-step nodes"; "ParseError names MAX_PROOF_EXPANDED_NODES";
    MAX_PROOF_DIGITS: usize = 4096; "decimal digits in one stored integer"; "ParseError names integer-digit ceiling";
    MAX_INTERPRETER_CALL_DEPTH: usize = 200; "calls in either reference interpreter"; "RunError::TooDeep names call-depth ceiling";
    DEFAULT_RUN_FUEL: u64 = 10_000_000; "CLI interpreter step allowance; API callers supply fuel"; "RunError::OutOfFuel; CLI reports step allowance";
    COUNTEREXAMPLE_BOX: i64 = 8; "diagnostic integer search interval in either direction"; "Counterexample::NoneInBox explicitly identifies bounded search";
    MAX_COUNTEREXAMPLE_ATOMS: usize = 4; "diagnostic counterexample enumeration variables"; "Counterexample::TooManyAtoms; explicit omission note; no proof is inferred";
}

/// Bounded collection with an explicit terminal error. Speculative elaboration
/// may truncate back to a checkpoint; that also restores its overflow state.
#[derive(Clone, Debug, Default)]
pub(crate) struct DiagnosticBuffer {
    values: Vec<Diagnostic>,
    overflowed: bool,
}
impl DiagnosticBuffer {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        if self.overflowed {
            return;
        }
        if self.values.len() < MAX_DIAGNOSTICS {
            self.overflowed = diagnostic.code == "L0011";
            self.values.push(diagnostic);
        } else {
            let span = diagnostic
                .labels
                .first()
                .map_or(Span::new(crate::source::FileId(0), 0, 0), |label| {
                    label.span
                });
            self.values.pop();
            self.values.push(Diagnostic::error("L0011", format!("MAX_DIAGNOSTICS limit of {MAX_DIAGNOSTICS} was exceeded"), span)
                .note("further diagnostics are omitted; compilation failed; fix the earlier errors and retry"));
            self.overflowed = true;
        }
    }
    pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        for diagnostic in diagnostics {
            self.push(diagnostic);
        }
    }
    pub fn truncate(&mut self, length: usize) {
        self.values.truncate(length);
        self.overflowed = self
            .values
            .iter()
            .any(|diagnostic| diagnostic.code == "L0011");
    }
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.values
    }
}
impl From<Vec<Diagnostic>> for DiagnosticBuffer {
    fn from(values: Vec<Diagnostic>) -> Self {
        let mut result = Self::default();
        result.extend(values);
        result
    }
}
impl std::ops::Deref for DiagnosticBuffer {
    type Target = Vec<Diagnostic>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
impl std::ops::DerefMut for DiagnosticBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speculative_diagnostic_rollback_restores_capacity() {
        let make = || Diagnostic::error("L0001", "test", Span::new(crate::source::FileId(0), 0, 0));
        let mut out = DiagnosticBuffer::default();
        for _ in 0..=MAX_DIAGNOSTICS {
            out.push(make());
        }
        assert!(out.overflowed());
        out.truncate(3);
        out.push(make());
        assert!(!out.overflowed());
        assert_eq!(out.len(), 4);
    }
}
