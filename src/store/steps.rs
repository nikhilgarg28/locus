//! Proofs as blocks of named steps: the DAG form of the text.
//!
//! A proof is a tree, and the trees the search finds repeat themselves: the
//! same equation is transported along twice, the same literal is viewed in
//! every premise. Written as a tree, each repetition is written out. Here a
//! proof is written as steps instead, one per line:
//!
//! ~~~text
//! t1 = fn:within_limit(#0.0)
//! s1 = transport(h3, t1, of_term(proof(of_term($10))))
//! s2 = transport(transport(h3, (#0 ==[struct:Lock] $9), refl($9)), t1, s1)
//! ~~~
//!
//! `tN` names a term and `sN` a proof; a line may name earlier steps only,
//! and the last line is the conclusion. The writer hash-conses: as the
//! tree is printed, every composite term and proof is interned by its own
//! text with its composite children replaced by their ids, so two pieces
//! that are structurally equal are one piece. A piece referred to more than
//! once, by distinct parents or twice by one, becomes a step; a piece used
//! once stays inline, and so does a piece used only inside one shared
//! parent, since the parent is written once. The block is then written by
//! expanding the root, naming each shared piece at its first use after its
//! own steps, so the names follow one deterministic traversal and two
//! machines write the same text. Leaves (a variable, a literal, a
//! hypothesis, `omitted`) are never steps: a name is no shorter.
//!
//! The reader parses each line with the text parser, which resolves a name
//! to the tree of the step it names, copied in. A name not yet defined,
//! whether unknown, later, or the line's own (the only way to write a
//! cycle), a duplicate, a malformed line, a `t` where a proof is wanted or
//! the reverse, and a last line that is a term are errors: the entry is a
//! miss. The size and depth of the tree built are counted at every use of
//! a step, so a block cannot name a tree larger than `MAX_NODES` nodes or
//! deeper than `MAX_DEPTH`, however it is written. All expanded step
//! trees retained by a block together also fit `MAX_NODES`, so many
//! individually small aliases cannot evade the allocation budget.

use std::collections::HashMap;

use crate::kernel::{Context, Proof, Term};

use super::text::{self, Names, ParseError, Parsed};

/// What a step or a piece is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Kind {
    Term,
    Proof,
}

impl Kind {
    fn prefix(self) -> char {
        match self {
            Self::Term => 't',
            Self::Proof => 's',
        }
    }

    pub(super) fn noun(self) -> &'static str {
        match self {
            Self::Term => "term",
            Self::Proof => "proof",
        }
    }
}

// --- Writing -------------------------------------------------------------------

/// A term with nothing inside it that could be a step of its own.
pub(super) fn leaf_term(term: &Term) -> bool {
    match term {
        Term::Free(_)
        | Term::Bound(_)
        | Term::Bool(_)
        | Term::U8(_)
        | Term::Int(_)
        | Term::Machine(..)
        | Term::Fn(_) => true,
        Term::Prim(_, arguments)
        | Term::PropApp(_, arguments)
        | Term::Struct(_, arguments)
        | Term::Variant(_, _, arguments) => arguments.is_empty(),
        Term::Tuple(fields, values) => fields.is_empty() && values.is_empty(),
        _ => false,
    }
}

pub(super) fn leaf_proof(proof: &Proof) -> bool {
    matches!(proof, Proof::Hyp(_) | Proof::Omitted)
}

/// How an interned piece stands in the text of its parent: its id between
/// two NUL bytes, which the text form never contains.
pub(super) fn placeholder(id: usize) -> String {
    format!("\0{id}\0")
}

/// One composite piece of the tree being printed.
struct Piece {
    kind: Kind,
    /// Its text, with each composite child as a placeholder.
    body: String,
    /// How many parents refer to it, a parent that refers to it twice
    /// counting twice; the root is referred to once.
    references: usize,
    /// Its step name, once it has been written.
    name: Option<String>,
}

/// The pieces of one proof, interned as the printer meets them.
#[derive(Default)]
pub(super) struct Sharing {
    ids: HashMap<String, usize>,
    pieces: Vec<Piece>,
    terms: usize,
    proofs: usize,
}

impl Sharing {
    /// The id of the piece whose body is `body`, new or seen before.
    pub(super) fn intern(&mut self, kind: Kind, body: String) -> usize {
        let key = format!("{}{body}", kind.prefix());
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        for child in placeholders(&body) {
            if let Some(piece) = self.pieces.get_mut(child) {
                piece.references += 1;
            }
        }
        let id = self.pieces.len();
        self.pieces.push(Piece {
            kind,
            body,
            references: 0,
            name: None,
        });
        self.ids.insert(key, id);
        id
    }

    /// The block: the steps, and the conclusion last, from the text of the
    /// root, which is a placeholder or a leaf.
    pub(super) fn block(mut self, root: &str) -> String {
        for child in placeholders(root) {
            if let Some(piece) = self.pieces.get_mut(child) {
                piece.references += 1;
            }
        }
        let mut lines = Vec::new();
        let conclusion = self.expand(root, &mut lines);
        let name = self.name(Kind::Proof);
        lines.push(format!("{name} = {conclusion}"));
        lines.join("\n")
    }

    fn name(&mut self, kind: Kind) -> String {
        let counter = match kind {
            Kind::Term => &mut self.terms,
            Kind::Proof => &mut self.proofs,
        };
        *counter += 1;
        format!("{}{counter}", kind.prefix())
    }

    /// `text` with each placeholder replaced by the name of its piece, or
    /// by the piece's own text when it is used once, writing the steps
    /// the pieces need as it goes.
    fn expand(&mut self, text: &str, lines: &mut Vec<String>) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find('\0') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            let end = after.find('\0').unwrap_or(after.len());
            if let Ok(id) = after[..end].parse::<usize>() {
                let reference = self.reference(id, lines);
                out.push_str(&reference);
            }
            rest = after.get(end + 1..).unwrap_or("");
        }
        out.push_str(rest);
        out
    }

    fn reference(&mut self, id: usize, lines: &mut Vec<String>) -> String {
        let Some(piece) = self.pieces.get(id) else {
            return String::new();
        };
        if let Some(name) = &piece.name {
            return name.clone();
        }
        let (kind, shared, body) = (piece.kind, piece.references > 1, piece.body.clone());
        let expanded = self.expand(&body, lines);
        if !shared {
            return expanded;
        }
        let name = self.name(kind);
        lines.push(format!("{name} = {expanded}"));
        self.pieces[id].name = Some(name.clone());
        name
    }
}

/// The ids of the placeholders in `text`, in order.
fn placeholders(text: &str) -> Vec<usize> {
    let mut ids = Vec::new();
    let mut fields = text.split('\0');
    // Placeholders are `\0id\0`: after the first field, every other field
    // is an id.
    fields.next();
    while let Some(id) = fields.next() {
        if let Ok(id) = id.parse() {
            ids.push(id);
        }
        fields.next();
    }
    ids
}

// --- Reading -------------------------------------------------------------------

/// A step read so far, with the size and depth of its tree.
pub(super) struct Step {
    value: Value,
    pub(super) size: usize,
    pub(super) depth: usize,
}

enum Value {
    Term(Term),
    Proof(Proof),
}

impl Step {
    pub(super) fn kind(&self) -> Kind {
        match self.value {
            Value::Term(_) => Kind::Term,
            Value::Proof(_) => Kind::Proof,
        }
    }

    /// The term, of a step whose kind is `Term`; a proof step gives a
    /// placeholder the kernel rejects, which `kind` rules out first.
    pub(super) fn term(&self) -> &Term {
        match &self.value {
            Value::Term(term) => term,
            Value::Proof(_) => &Term::Bound(0),
        }
    }

    pub(super) fn proof(&self) -> &Proof {
        match &self.value {
            Value::Proof(proof) => proof,
            Value::Term(_) => &Proof::Omitted,
        }
    }
}

/// The steps a line may name.
#[derive(Default)]
pub(super) struct Steps {
    by_name: HashMap<String, Step>,
}

impl Steps {
    pub(super) fn get(&self, name: &str) -> Option<&Step> {
        self.by_name.get(name)
    }
}

/// `t` or `s` and digits: a name the text form reserves for steps.
pub(super) fn is_step_name(word: &str) -> bool {
    let mut chars = word.chars();
    matches!(chars.next(), Some('t' | 's')) && word.len() > 1 && chars.all(|c| c.is_ascii_digit())
}

/// A line of a block, `name = body`, with the body's offset in the line.
fn step_line(line: &str) -> Option<(&str, &str, usize)> {
    let name_end = line.find(|c: char| !c.is_ascii_alphanumeric())?;
    let name = &line[..name_end];
    if !is_step_name(name) {
        return None;
    }
    let rest = line[name_end..].trim_start();
    let body = rest.strip_prefix('=')?;
    // `==[` and `=>` are the text form's own; a step line has one `=`.
    if body.starts_with(['=', '>']) {
        return None;
    }
    let body = body.trim_start();
    Some((name, body, line.len() - body.len()))
}

/// Whether `text` is a block of steps rather than one bare expression:
/// some line of it is a step line.
pub(super) fn is_block(text: &str) -> bool {
    text.lines().any(|line| step_line(line.trim()).is_some())
}

/// Reads a block: each line in order, over the steps before it, and the
/// last line's proof.
pub(super) fn parse_block(text: &str, ctx: &Context, names: &Names) -> Parsed<Proof> {
    let mut steps = Steps::default();
    let mut last: Option<String> = None;
    let mut retained_nodes = 0usize;
    let mut offset = 0;
    for (index, raw) in text.split('\n').enumerate() {
        let start = offset;
        offset += raw.len() + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let at = start + (raw.len() - raw.trim_start().len());
        let number = index + 1;
        let Some((name, body, body_at)) = step_line(line) else {
            return Err(ParseError {
                at,
                message: format!("line {number}: not a step, `tN = term` or `sN = proof`"),
            });
        };
        if steps.by_name.contains_key(name) {
            return Err(ParseError {
                at,
                message: format!("line {number}: `{name}` is defined twice"),
            });
        }
        let located = |error: ParseError| ParseError {
            at: at + body_at + error.at,
            message: format!("line {number}: {}", error.message),
        };
        let parser = text::parser(body, ctx, names, &steps)
            .map_err(located)?
            .with_node_limit(text::MAX_NODES.saturating_sub(retained_nodes));
        let (value, size, depth) = if name.starts_with('t') {
            text::whole(parser, |parser| parser.term().map(Value::Term))
        } else {
            text::whole(parser, |parser| parser.proof().map(Value::Proof))
        }
        .map_err(located)?;
        retained_nodes += size;
        steps
            .by_name
            .insert(name.to_string(), Step { value, size, depth });
        last = Some(name.to_string());
    }
    let Some(last) = last else {
        return Err(ParseError {
            at: 0,
            message: "the block has no steps".into(),
        });
    };
    match steps.by_name.remove(&last).map(|step| step.value) {
        Some(Value::Proof(proof)) => Ok(proof),
        _ => Err(ParseError {
            at: text.len(),
            message: format!("the last step, `{last}`, is a term, and the conclusion is a proof"),
        }),
    }
}
