//! The proofs file (E11): every proof the corpus produces is written, read
//! back over a fresh elaboration, and accepted by the kernel; the file is
//! stable under a second run, under reformatting, and under edits elsewhere;
//! a hostile file costs a search or a report and never a false claim; and
//! the reader is total on random and mutated text.
//!
//! The randomized tests take a fixed seed, so a run is reproducible, and
//! `LOCUS_EXTENDED` (any value) multiplies their case counts by 100.

#[path = "common/rng.rs"]
mod rng;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use locus::elab::{Elaborated, Options, elaborate_with_store_and_options};
use locus::kernel::theory;
use locus::kernel::{
    Axiom, CmpOp, Context, Definitions, ForLoop, Integer, MachineInt, Op, Prim, Proof, ProofArm,
    PropVariant, Term, TermArm, Type, check_proof,
};
use locus::parser::parse;
use locus::source::SourceMap;
use locus::store::text::{Names, parse_proof, parse_term, parse_type, print_proof, print_term};
use locus::store::{Key, ProofStore, Stats};
use rng::{Rng, case_seed};

const SEED: u64 = 0x4C4F_4355_5300_0011;
const MUTATED_PROOFS: u64 = 3_000;
const RANDOM_TEXTS: u64 = 2_000;
const EXTENDED_FACTOR: u64 = 100;

fn extended() -> bool {
    std::env::var_os("LOCUS_EXTENDED").is_some_and(|value| !value.is_empty())
}

fn cases(base: u64) -> u64 {
    if extended() {
        base * EXTENDED_FACTOR
    } else {
        base
    }
}

/// The tiers the store stands in front of.
const SEARCHED: [&str; 4] = ["exact", "computed", "evaluation", "arithmetic"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every source file the corpus accepts: the examples, the accepted files,
/// and the target files the elaborator has caught up with.
fn corpus_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for directory in ["examples", "tests/corpus/accept", "tests/corpus/target"] {
        for entry in fs::read_dir(root().join(directory)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| extension != "lc") {
                continue;
            }
            if fs::read_to_string(&path)
                .unwrap()
                .contains("//~ parse-only")
            {
                continue;
            }
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Elaborates `text` with `store` installed.
fn run(name: &str, text: &str, store: ProofStore) -> (Elaborated, ProofStore) {
    let mut sources = SourceMap::default();
    let file = sources.add(name, text);
    let source = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{name} does not parse");
    let mut options = Options::default();
    for line in text.lines() {
        if let Some(name) = line.trim().strip_prefix("//~ preview:") {
            let feature = locus::preview::Feature::parse(name.trim()).unwrap();
            if feature.status() == locus::preview::Status::Preview {
                options.previews.enable(feature.name()).unwrap();
            }
        }
    }
    elaborate_with_store_and_options(source, &parsed.program, store, &options)
}

fn read(text: &str) -> ProofStore {
    let (store, warnings) = ProofStore::parse(text).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    store
}

fn lock() -> String {
    fs::read_to_string(root().join("examples/lock.lc")).unwrap()
}

/// The lines of a rendered file that hold proofs, by the key line before
/// each.
fn entries(rendered: &str) -> BTreeMap<String, String> {
    let lines: Vec<&str> = rendered.lines().collect();
    let mut entries = BTreeMap::new();
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("obligation ") {
            entries.insert((*line).to_string(), lines[index + 1].to_string());
        }
    }
    entries
}

#[test]
#[doc = "spec: 1.20:1"]
fn every_found_proof_is_written_read_back_and_accepted() {
    let mut files = 0;
    let mut proofs = 0;
    for path in corpus_files() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = fs::read_to_string(&path).unwrap();
        let (first, store) = run(&name, &text, ProofStore::new());
        assert!(first.is_success(), "{name}: {:?}", first.diagnostics);
        // A run may ask for one obligation twice, when two holes have the
        // same context and claim; the second is a hit on what the first
        // recorded.
        let found = store.stats();
        assert_eq!(found.stale, 0, "{name}");
        assert_eq!(found.unprintable, 0, "{name}: a proof could not be written");
        let rendered = store.render();
        if let Some(names) = store.names() {
            // Each proof, printed over its own context, reads back as the
            // same proof, which the kernel accepts again.
            for hole in first.holes.iter().filter(|hole| hole.found.is_some()) {
                let found = hole.found.as_ref().unwrap();
                let printed = print_proof(&found.proof, &found.context, names)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                let back = parse_proof(&printed, &found.context, names)
                    .unwrap_or_else(|error| panic!("{name}: {printed}: {error}"));
                assert_eq!(back, found.proof, "{name}: {printed}");
                let mut ctx = found.context.clone();
                check_proof(&mut ctx, &back, &found.claim).unwrap();
                proofs += 1;
            }
        } else {
            assert!(first.holes.iter().all(|hole| hole.found.is_none()));
        }

        // Read back, with the search forbidden: every obligation is met by
        // its entry, no entry is left over, and the file is rewritten byte
        // for byte.
        let (second, store) = run(&name, &text, read(&rendered).locked(true).searching(false));
        assert!(second.is_success(), "{name}: {:?}", second.diagnostics);
        // An obligation whose search failed has nothing stored and misses
        // again: the elaborator makes such attempts and withdraws them, as
        // when it tries to show a panic unreachable before treating it as
        // a check that runs. Every proof that was found is a hit.
        let again = store.stats();
        assert_eq!(again.searches, 0, "{name}");
        assert_eq!(again.misses, found.misses - found.recorded, "{name}");
        assert_eq!(again.stale, 0, "{name}");
        assert_eq!(again.hits, found.recorded + found.hits, "{name}");
        assert_eq!(
            store.used(),
            store.len(),
            "{name}: an entry was not asked for"
        );
        assert_eq!(store.render(), rendered, "{name}");
        for hole in &second.holes {
            assert!(hole.solved, "{name}");
            assert!(!SEARCHED.contains(&hole.tier), "{name}: {}", hole.tier);
        }
        // A function that turns out not to be a term is elaborated twice,
        // and the reports of its first pass are dropped: the store counts
        // both passes, so its hits bound the reports and need not equal
        // them.
        let stored = second
            .holes
            .iter()
            .filter(|hole| hole.tier == "stored")
            .count();
        assert!(stored <= again.hits, "{name}: {stored} > {}", again.hits);
        assert_eq!(stored == 0, again.hits == 0, "{name}");
        files += 1;
    }
    assert!(files >= 30, "{files} files");
    assert!(proofs >= 60, "{proofs} proofs");
}

#[test]
fn reformatting_comments_and_unrelated_edits_leave_every_entry_in_use() {
    let original = lock();
    let (_, store) = run("lock.lc", &original, ProofStore::new());
    let rendered = store.render();
    let total = store.stats().recorded;
    assert_eq!(total, 8);
    let again = |edited: &str| -> (Stats, String) {
        let (elaborated, store) = run("lock.lc", edited, read(&rendered));
        assert!(elaborated.is_success(), "{:?}", elaborated.diagnostics);
        (store.stats(), store.render())
    };
    let all_in_use = |edited: &str| {
        let (stats, text) = again(edited);
        assert_eq!(stats.hits, total, "{edited}");
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.searches, 0);
        assert_eq!(text, rendered);
    };

    // A comment and a blank line before every line.
    let commented: String = original
        .lines()
        .map(|line| format!("// a comment\n\n{line}\n"))
        .collect();
    all_in_use(&commented);
    // Every line indented and given a trailing space.
    let reindented: String = original
        .lines()
        .map(|line| format!("    {line} \n"))
        .collect();
    all_in_use(&reindented);
    // Names are not part of the key: `run`'s parameters and the function
    // `attempts_left` renamed, with their uses.
    let renamed = original
        .replace("attempts", "tries")
        .replace("correct", "code");
    assert_ne!(renamed, original);
    all_in_use(&renamed);
    // An unrelated function edited: `event_at` written the other way round.
    let edited = original.replace(
        "if attempt == correct { Event::Right } else { Event::Wrong }",
        "if attempt != correct { Event::Wrong } else { Event::Right }",
    );
    assert_ne!(edited, original);
    all_in_use(&edited);
    // A function edited after its obligation: `attempts_left` gains a
    // binding, and it has no obligation of its own.
    let edited = original.replace(
        "    let (left, _) = remaining(last.failures, bounded);\n    left\n",
        "    let (left, _) = remaining(last.failures, bounded);\n    let answer = left;\n    answer\n",
    );
    assert_ne!(edited, original);
    all_in_use(&edited);

    // A function edited before its obligations: `run` gains a binding, so
    // the contexts of its two obligations change and their entries alone
    // miss. The other six entries stay in use and are written unchanged.
    let edited = original.replace(
        "    let mut lock = Lock { failures: 0, open: false };",
        "    let limit: u8 = 3;\n    let mut lock = Lock { failures: 0, open: false };",
    );
    assert_ne!(edited, original);
    let (stats, text) = again(&edited);
    assert_eq!(stats.hits, 6);
    assert_eq!(stats.misses, 2);
    assert_eq!(stats.searches, 2);
    assert_eq!(stats.recorded, 2);
    let (before, after) = (entries(&rendered), entries(&text));
    assert_eq!(before.len(), 8);
    assert_eq!(after.len(), 8);
    let kept = before
        .iter()
        .filter(|(key, _)| after.contains_key(*key))
        .count();
    assert_eq!(kept, 6);
    assert!(
        before
            .iter()
            .filter(|(key, _)| after.contains_key(*key))
            .all(|(key, proof)| after[key] == *proof)
    );
    let replaced: Vec<&String> = after
        .keys()
        .filter(|key| !before.contains_key(*key))
        .collect();
    assert_eq!(replaced.len(), 2);
    assert!(
        replaced.iter().all(|key| key.contains(" run ")),
        "{replaced:?}"
    );
}

#[test]
#[doc = "spec: 1.20:1"]
fn hostile_files_cost_a_search_or_a_report_and_never_pass_a_false_claim() {
    let source = lock();
    let (_, store) = run("lock.lc", &source, ProofStore::new());
    let rendered = store.render();
    let lines: Vec<String> = rendered.lines().map(str::to_string).collect();
    let proofs_at: Vec<usize> = (0..lines.len())
        .filter(|&index| lines[index].starts_with("obligation "))
        .map(|index| index + 1)
        .collect();
    assert_eq!(proofs_at.len(), 8);
    let file = |lines: &[String]| lines.join("\n") + "\n";
    // Whatever was accepted was accepted by the kernel: every hit is a
    // proof the kernel accepts again, over its context, of its claim.
    let accepted_by_the_kernel = |elaborated: &Elaborated| {
        for hole in elaborated.holes.iter().filter(|hole| hole.tier == "stored") {
            let found = hole.found.as_ref().unwrap();
            let mut ctx = found.context.clone();
            check_proof(&mut ctx, &found.proof, &found.claim).unwrap();
        }
    };

    // Entries swapped between obligations: two proofs of other claims. The
    // kernel refuses both, the search runs for both, and the file is
    // written right again; under `--locked` they are errors that name the
    // obligation.
    let mut swapped = lines.clone();
    let step_proofs: Vec<usize> = proofs_at
        .iter()
        .copied()
        .filter(|&at| lines[at - 1].contains(" step "))
        .collect();
    assert!(step_proofs.len() >= 2);
    swapped.swap(step_proofs[0], step_proofs[1]);
    assert_ne!(lines[step_proofs[0]], lines[step_proofs[1]]);
    let (elaborated, store) = run("lock.lc", &source, read(&file(&swapped)));
    assert!(elaborated.is_success());
    accepted_by_the_kernel(&elaborated);
    let stats = store.stats();
    assert_eq!((stats.hits, stats.stale, stats.searches), (6, 2, 2));
    assert_eq!(store.render(), rendered);
    let (elaborated, store) = run("lock.lc", &source, read(&file(&swapped)).locked(true));
    assert!(!elaborated.is_success());
    assert_eq!(store.stats().searches, 0);
    assert!(elaborated.diagnostics.iter().all(|diagnostic| {
        diagnostic.code == "L0230" && diagnostic.message.contains("the proofs file has none")
    }));
    assert!(
        elaborated
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.starts_with("`step` needs a proof of `"))
    );

    // A proof edited by hand to prove something else, and proofs of
    // nothing at all: each is refused, and the obligation is searched.
    let evaluations: Vec<usize> = proofs_at
        .iter()
        .copied()
        .filter(|&at| lines[at - 1].ends_with(" run 1") || lines[at - 1].ends_with(" step 4"))
        .collect();
    assert_eq!(evaluations.len(), 2);
    for (edit, count) in [
        ("evaluate(int_le(view[u8](0), view[u8](2)))", 2),
        ("omitted", 2),
        ("h0", 2),
        ("refl(", 2),
        ("axiom(int_le_refl, view[u8](0))", 2),
        ("evaluate(int_le(view[u8](0), view[u8](3))) extra", 2),
    ] {
        let mut edited = lines.clone();
        for &at in &evaluations {
            edited[at] = edit.to_string();
        }
        let (elaborated, store) = run("lock.lc", &source, read(&file(&edited)));
        assert!(elaborated.is_success(), "{edit}");
        accepted_by_the_kernel(&elaborated);
        let stats = store.stats();
        assert_eq!(stats.stale, count, "{edit}");
        assert_eq!(stats.hits, 8 - count, "{edit}");
        assert_eq!(store.render(), rendered, "{edit}");
    }

    // The file truncated at every length: read without a panic, and what
    // survives is used or refused, never more.
    let mut truncations = 0;
    let mut used = 0;
    for length in (0..=rendered.len()).step_by(if extended() { 1 } else { 3 }) {
        let Some(prefix) = rendered.get(..length) else {
            continue;
        };
        match ProofStore::parse(prefix) {
            Err(problem) => assert!(length < "locus-proofs 1".len(), "{length}: {problem}"),
            Ok((store, _)) => {
                let (elaborated, store) = run("lock.lc", &source, store);
                assert!(elaborated.is_success(), "{length}");
                accepted_by_the_kernel(&elaborated);
                let stats = store.stats();
                assert_eq!(stats.hits + stats.misses, 8, "{length}");
                assert_eq!(store.render(), rendered, "{length}");
                used += stats.hits;
            }
        }
        truncations += 1;
    }
    assert!(truncations > 100, "{truncations}");
    assert!(used > 0);

    // Random bytes, bare and behind the header: read without a panic.
    let mut rng = Rng::new(SEED);
    for _ in 0..cases(RANDOM_TEXTS) {
        let length = rng.range(0..300);
        let bytes: Vec<u8> = (0..length).map(|_| rng.below(256) as u8).collect();
        let text = String::from_utf8_lossy(&bytes).to_string();
        let _ = ProofStore::parse(&text);
        let _ = ProofStore::parse(&format!("locus-proofs 1\n\n{text}"));
        let _ = ProofStore::parse(&format!("locus-proofs 1\n\nobligation {text}"));
    }
}

#[test]
fn locked_fails_on_a_missing_entry_and_searches_nothing_with_a_complete_file() {
    let source = lock();
    let (_, store) = run("lock.lc", &source, ProofStore::new());
    let rendered = store.render();

    // Complete: no search, every obligation stored.
    let (elaborated, store) = run("lock.lc", &source, read(&rendered).locked(true));
    assert!(elaborated.is_success());
    let stats = store.stats();
    assert_eq!(stats.searches, 0);
    assert_eq!(stats.hits, 8);
    assert!(elaborated.holes.iter().all(|hole| hole.tier == "stored"));

    // One entry removed: the obligation is named, and nothing is searched.
    let missing: String = rendered
        .split("\n\n")
        .filter(|entry| !entry.contains(" run 1\n"))
        .collect::<Vec<_>>()
        .join("\n\n");
    assert_ne!(missing, rendered);
    let (elaborated, store) = run("lock.lc", &source, read(&missing).locked(true));
    assert!(!elaborated.is_success());
    assert_eq!(store.stats().searches, 0);
    let messages: Vec<&str> = elaborated
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        messages,
        ["`run` needs a proof of `0 <= 3` at line 56, and the proofs file has none"]
    );

    // Surviving an upgrade: with every tier made to fail, the complete
    // file still checks, and an empty store does not.
    let (elaborated, store) = run("lock.lc", &source, read(&rendered).searching(false));
    assert!(elaborated.is_success());
    assert_eq!(store.stats().searches, 0);
    let (elaborated, store) = run("lock.lc", &source, ProofStore::new().searching(false));
    assert!(!elaborated.is_success());
    assert_eq!(store.stats().searches, 0);
    assert!(
        elaborated
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "L0230"
                && (diagnostic.message.starts_with("cannot show")
                    || diagnostic.message.starts_with("this is evidence of"))),
        "{:?}",
        elaborated.diagnostics
    );
}

#[test]
fn keys_are_fnv1a_over_the_key_text() {
    // The reference values of FNV-1a, 64 bits.
    assert_eq!(Key::of("").to_string(), "cbf29ce484222325");
    assert_eq!(Key::of("a").to_string(), "af63dc4c8601ec8c");
    assert_eq!(Key::of("foobar").to_string(), "85944171f73967e8");
}

/// A context with one declaration of each kind, and the names for them.
fn declared() -> (Context, Names) {
    let (mut definitions, prelude) = Definitions::with_prelude();
    let theory = theory::declare(&mut definitions, &prelude).unwrap();
    let pair = Type::Tuple(vec![Type::U8, Type::Bool]);
    let lock = definitions.declare_struct(&pair).unwrap();
    let event = definitions
        .declare_enum(&[Type::Tuple(vec![]), Type::Tuple(vec![Type::U8])])
        .unwrap();
    let small = definitions
        .declare_prop(
            vec![Type::U8],
            vec![
                PropVariant::indexed(Type::Tuple(vec![]), |_| vec![Term::U8(0)]),
                PropVariant::Params(Type::Tuple(vec![Type::U8])),
            ],
        )
        .unwrap();
    let mut names = Names::new();
    names.structure("Lock", lock);
    names.enumeration("Event", event);
    names.proposition("Small", small);
    names.proposition("False", prelude.falsehood);
    names.proposition("And", prelude.and);
    for (name, id) in theory.lemma_names() {
        names.function(name, id);
    }
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let x = ctx.declare(Type::U8).unwrap();
    let _ = ctx.declare_ghost(Type::Int).unwrap();
    let _ = ctx
        .assume(Term::eq(Type::U8, Term::Free(x), Term::U8(3)))
        .unwrap();
    let _ = ctx.declare(Type::Struct(lock)).unwrap();
    let _ = ctx
        .assume(Term::PropApp(small, vec![Term::Free(x)]))
        .unwrap();
    (ctx, names)
}

#[test]
fn every_term_and_proof_form_round_trips() {
    let (ctx, names) = declared();
    let lock = match parse_type("struct:Lock", &ctx, &names).unwrap() {
        Type::Struct(id) => id,
        other => panic!("{other:?}"),
    };
    let event = match parse_type("enum:Event", &ctx, &names).unwrap() {
        Type::Enum(id) => id,
        other => panic!("{other:?}"),
    };
    let small = match parse_term("prop:Small()", &ctx, &names).unwrap() {
        Term::PropApp(id, _) => id,
        other => panic!("{other:?}"),
    };
    let le_trans = match parse_term("fn:u8_le_trans", &ctx, &names).unwrap() {
        Term::Fn(id) => id,
        other => panic!("{other:?}"),
    };
    let x = parse_term("$0", &ctx, &names).unwrap();
    let n = parse_term("$1", &ctx, &names).unwrap();
    let h = parse_proof("h0", &ctx, &names).unwrap();
    let s = parse_proof("h1", &ctx, &names).unwrap();
    let arm = |vars, hyps, body| ProofArm {
        vars,
        hyps,
        body: Box::new(body),
    };
    let int = |value: i64| Term::Int(Integer::from(value));
    let mut terms = vec![
        Term::Boxed(Box::new(Term::U8(3))),
        Term::eq(
            Type::Boxed(Box::new(Type::U8)),
            Term::Boxed(Box::new(Term::U8(3))),
            Term::Boxed(Box::new(Term::U8(3))),
        ),
        Term::Buffer {
            op: locus::kernel::BufferOp::Literal,
            element: Type::U8,
            arguments: vec![Term::U8(3)],
        },
        Term::Buffer {
            op: locus::kernel::BufferOp::Length,
            element: Type::U8,
            arguments: vec![x.clone()],
        },
        Term::proof(Proof::BufferStep(x.clone())),
        Term::proof(Proof::BufferBound {
            value: x.clone(),
            upper: false,
        }),
        Term::proof(Proof::BufferBound {
            value: x.clone(),
            upper: true,
        }),
        Term::Lambda {
            params: vec![Type::Int],
            result: Type::Int,
            body: Box::new(Term::int_add(Term::Bound(0), n.clone())),
        },
        x.clone(),
        n.clone(),
        Term::Bound(7),
        Term::Bool(true),
        Term::Bool(false),
        Term::U8(0),
        Term::U8(255),
        int(0),
        int(-1),
        Term::Int("-170141183460469231731687303715884105729".parse().unwrap()),
        Term::machine_int(MachineInt::U16, 65535),
        Term::machine_int(MachineInt::I8, -128),
        Term::machine_int(MachineInt::I64, i64::MIN.into()),
        Term::machine_int(MachineInt::U64, u64::MAX.into()),
        Term::int_add(int(1), int(2)),
        Term::int_sub(int(1), int(2)),
        Term::int_mul(int(1), int(2)),
        Term::int_div(int(1), int(2)),
        Term::int_rem(int(1), int(2)),
        Term::int_neg(int(1)),
        Term::int_le(int(1), int(2)),
        Term::view(MachineInt::U8, x.clone()),
        Term::wrap(MachineInt::I32, n.clone()),
        Term::cast(MachineInt::U8, MachineInt::I16, x.clone()),
        Term::cmp(CmpOp::Eq, MachineInt::U8, x.clone(), Term::U8(3)),
        Term::cmp(CmpOp::Lt, MachineInt::U8, x.clone(), Term::U8(3)),
        Term::cmp(CmpOp::Le, MachineInt::U8, x.clone(), Term::U8(3)),
        Term::Prim(Prim::IntAdd, vec![]),
        Term::eq(Type::U8, x.clone(), Term::U8(3)),
        Term::eq(Type::Prop, Term::Bool(true), Term::Bool(false)),
        Term::implies(Term::Bool(true), Term::Bool(false)),
        Term::forall(Type::U8, |v| Term::eq(Type::U8, v.clone(), v)),
        Term::exists(Type::Int, |v| Term::int_le(v, int(0))),
        Term::Tuple(vec![], vec![]),
        Term::Tuple(
            vec![
                Type::U8,
                Type::proof(Term::eq(Type::U8, Term::Bound(0), Term::U8(1))),
            ],
            vec![Term::U8(1), Term::proof(Proof::Refl(Term::U8(1)))],
        ),
        Term::Struct(lock, vec![Term::U8(1), Term::Bool(true)]),
        Term::Struct(lock, vec![]),
        Term::proj(Term::proj(x.clone(), 0), 1),
        Term::proj(Term::Tuple(vec![Type::U8], vec![Term::U8(1)]), 0),
        Term::proof(h.clone()),
        Term::Fn(le_trans),
        Term::call(Term::Fn(le_trans), vec![x.clone(), x.clone(), x.clone()]),
        Term::call(Term::proj(x.clone(), 0), vec![]),
        Term::call(Term::call(x.clone(), vec![n.clone()]), vec![n.clone()]),
        Term::Variant(event, 0, vec![]),
        Term::Variant(event, 1, vec![Term::U8(2)]),
        Term::Case {
            scrutinee: Box::new(Term::Variant(event, 1, vec![Term::U8(2)])),
            result: Type::U8,
            arms: vec![
                TermArm {
                    binders: 0,
                    body: Term::U8(0),
                },
                TermArm {
                    binders: 1,
                    body: Term::Bound(0),
                },
            ],
        },
        Term::Case {
            scrutinee: Box::new(Term::Bool(true)),
            result: Type::proof(Term::Bool(true)),
            arms: vec![],
        },
        Term::PropApp(small, vec![x.clone()]),
        Term::Absurd(
            Box::new(s.clone()),
            Type::Fn(vec![Type::U8], Box::new(Type::Prop)),
        ),
        Term::Absurd(
            Box::new(s.clone()),
            Type::proof(Term::proj(Term::Struct(lock, vec![]), 3)),
        ),
    ];
    for op in Op::ALL {
        let operands = vec![x.clone(); op.arity()];
        terms.push(Term::op(op, MachineInt::I8, operands));
    }
    let looped = Term::For(Box::new(ForLoop {
        lo: Term::U8(0),
        hi: x.clone(),
        ordered: Proof::Omitted,
        state: vec![Type::U8, Type::Bool],
        init: Term::Tuple(
            vec![Type::U8, Type::Bool],
            vec![Term::U8(0), Term::Bool(true)],
        ),
        body: Term::Bound(0),
    }));
    terms.push(looped.clone());
    let every_type = Type::Tuple(vec![
        Type::Bool,
        Type::U8,
        Type::Int,
        Type::Prop,
        Type::Machine(MachineInt::I64),
        Type::proof(Term::Bool(true)),
        Type::Tuple(vec![Type::Tuple(vec![])]),
        Type::Struct(lock),
        Type::Enum(event),
        Type::Fn(
            vec![Type::U8, Type::Prop],
            Box::new(Type::proof(Term::Bound(0))),
        ),
    ]);
    terms.push(Term::Tuple(vec![every_type.clone()], vec![Term::Bound(0)]));
    for term in &terms {
        let printed = print_term(term, &ctx, &names).unwrap();
        let back =
            parse_term(&printed, &ctx, &names).unwrap_or_else(|error| panic!("{printed}: {error}"));
        assert_eq!(&back, term, "{printed}");
    }

    let a = Term::view(MachineInt::U8, x.clone());
    let mut proofs = vec![
        h.clone(),
        s.clone(),
        Proof::Hyp(locus::kernel::HypRef::Bound(3)),
        Proof::OfTerm(x.clone()),
        Proof::Refl(x.clone()),
        Proof::Transport {
            eq: Box::new(h.clone()),
            template: Term::eq(Type::U8, Term::Bound(0), Term::U8(3)),
            proof: Box::new(Proof::Refl(x.clone())),
        },
        Proof::ImpliesIntro {
            hyp: Term::Bool(true),
            body: Box::new(Proof::Hyp(locus::kernel::HypRef::Bound(0))),
        },
        Proof::ImpliesElim(Box::new(h.clone()), Box::new(s.clone())),
        Proof::ForallIntro {
            ty: every_type.clone(),
            body: Box::new(Proof::Refl(Term::Bound(0))),
        },
        Proof::ForallElim(Box::new(h.clone()), x.clone()),
        Proof::Projection(Term::proj(x.clone(), 0)),
        Proof::Literal(a.clone()),
        Proof::Definition(Term::call(Term::Fn(le_trans), vec![])),
        Proof::CaseStep(Term::Bool(true)),
        Proof::CaseKnown {
            term: Term::Bool(true),
            equation: Box::new(Proof::Refl(Term::Bool(true))),
        },
        Proof::BufferStep(Term::Bool(true)),
        Proof::BufferBound {
            value: Term::Bool(true),
            upper: false,
        },
        Proof::BufferBound {
            value: Term::Bool(true),
            upper: true,
        },
        Proof::Construct {
            prop: small,
            variant: 0,
            params: vec![],
            payload: vec![],
        },
        Proof::Construct {
            prop: small,
            variant: 1,
            params: vec![x.clone()],
            payload: vec![Term::U8(1)],
        },
        Proof::CaseProof {
            scrutinee: Box::new(s.clone()),
            goal: Term::Bool(true),
            arms: vec![
                arm(0, 1, Proof::Hyp(locus::kernel::HypRef::Bound(0))),
                arm(1, 0, Proof::Refl(Term::Bound(0))),
            ],
        },
        Proof::CaseData {
            scrutinee: x.clone(),
            goal: Term::Bool(true),
            arms: vec![],
        },
        Proof::ExistsIntro {
            prop: Term::exists(Type::U8, |v| Term::eq(Type::U8, v, Term::U8(1))),
            witness: Term::U8(1),
            proof: Box::new(Proof::Refl(Term::U8(1))),
        },
        Proof::ExistsElim {
            exists: Box::new(h.clone()),
            goal: Term::Bool(false),
            arm: arm(1, 1, Proof::Omitted),
        },
        Proof::ExcludedMiddle(Term::Bool(true)),
        Proof::ForEmpty(looped.clone()),
        Proof::ForStep {
            looped: looped.clone(),
            lower: Box::new(h.clone()),
            upper: Box::new(s.clone()),
        },
        Proof::Omitted,
        Proof::Evaluate(a.clone()),
        Proof::PropInduction {
            scrutinee: Box::new(h.clone()),
            motive: locus::kernel::TermArm {
                binders: 1,
                body: Term::eq(Type::Int, Term::Bound(0), Term::Bound(0)),
            },
            arms: vec![arm(1, 1, Proof::Omitted)],
        },
        Proof::DataInduction {
            target: Term::Variant(event, 0, vec![]),
            motives: vec![(
                event,
                Term::eq(Type::Enum(event), Term::Bound(0), Term::Bound(0)),
            )],
            arms: vec![arm(0, 0, Proof::Omitted)],
        },
        Proof::IntInduction {
            motive: Term::int_le(int(0), Term::Bound(0)),
            base: Box::new(Proof::Omitted),
            step: arm(1, 2, Proof::Omitted),
            target: int(5),
        },
        Proof::linear(
            Term::int_le(a.clone(), int(255)),
            3,
            vec![
                (h.clone(), -2),
                (Proof::Axiom(Axiom::ViewUpper(MachineInt::U8, x.clone())), 1),
            ],
        ),
        Proof::Linear {
            goal: Term::PropApp(small, vec![]),
            goal_coefficient: "-99999999999999999999999".parse().unwrap(),
            pairs: vec![],
        },
    ];
    let axioms = vec![
        Axiom::IntAddAssoc(a.clone(), a.clone(), a.clone()),
        Axiom::IntAddComm(a.clone(), a.clone()),
        Axiom::IntAddZero(a.clone()),
        Axiom::IntAddNeg(a.clone()),
        Axiom::IntSubDef(a.clone(), a.clone()),
        Axiom::IntMulAssoc(a.clone(), a.clone(), a.clone()),
        Axiom::IntMulComm(a.clone(), a.clone()),
        Axiom::IntMulOne(a.clone()),
        Axiom::IntMulAdd(a.clone(), a.clone(), a.clone()),
        Axiom::IntLeRefl(a.clone()),
        Axiom::IntLeTrans(a.clone(), a.clone(), a.clone()),
        Axiom::IntLeAntisymm(a.clone(), a.clone()),
        Axiom::IntLeAdd(a.clone(), a.clone(), a.clone()),
        Axiom::IntLeMul(a.clone(), a.clone()),
        Axiom::IntLeTotal(a.clone(), a.clone()),
        Axiom::IntLtIrrefl(a.clone()),
        Axiom::IntDivRem(a.clone(), a.clone()),
        Axiom::IntDivZero(a.clone()),
        Axiom::IntRemLowerPos(a.clone(), a.clone()),
        Axiom::IntRemUpperPos(a.clone(), a.clone()),
        Axiom::IntRemLowerNeg(a.clone(), a.clone()),
        Axiom::IntRemUpperNeg(a.clone(), a.clone()),
        Axiom::IntRemNonneg(a.clone(), a.clone()),
        Axiom::IntRemNonpos(a.clone(), a.clone()),
        Axiom::ViewLower(MachineInt::U8, x.clone()),
        Axiom::ViewUpper(MachineInt::I16, x.clone()),
        Axiom::WrapView(MachineInt::U32, x.clone()),
        Axiom::ViewWrap(MachineInt::I64, n.clone()),
        Axiom::WrapPeriod(MachineInt::U64, n.clone()),
        Axiom::CastDef(MachineInt::U8, MachineInt::I8, x.clone()),
        Axiom::OpModel(Op::Add, MachineInt::U8, vec![x.clone(), x.clone()]),
        Axiom::OpModel(Op::Neg, MachineInt::I8, vec![x.clone()]),
        Axiom::OpExact(Op::WrappingMul, MachineInt::U16, vec![x.clone(), x.clone()]),
        Axiom::OpExact(Op::WrappingNeg, MachineInt::I32, vec![x.clone()]),
        Axiom::CmpReflect(
            Term::cmp(CmpOp::Le, MachineInt::U8, x.clone(), x.clone()),
            true,
        ),
        Axiom::CmpReflect(
            Term::cmp(CmpOp::Le, MachineInt::U8, x.clone(), x.clone()),
            false,
        ),
        Axiom::CmpReify(Term::int_cmp(CmpOp::Le, n.clone(), n.clone()), true),
        Axiom::CmpReify(Term::int_cmp(CmpOp::Eq, n.clone(), n.clone()), false),
    ];
    let mut seen: Vec<&str> = Vec::new();
    for axiom in axioms {
        if !seen.contains(&axiom.name()) {
            seen.push(axiom.name());
        }
        proofs.push(Proof::Axiom(axiom));
    }
    assert_eq!(seen.len(), 34, "every axiom is listed");
    let mut rules: Vec<&str> = Vec::new();
    for proof in &proofs {
        if !rules.contains(&proof.rule_name()) {
            rules.push(proof.rule_name());
        }
        let printed = print_proof(proof, &ctx, &names).unwrap();
        let back = parse_proof(&printed, &ctx, &names)
            .unwrap_or_else(|error| panic!("{printed}: {error}"));
        assert_eq!(&back, proof, "{printed}");
    }
    assert_eq!(rules.len(), 31, "every rule is listed: {rules:?}");
}

#[test]
fn what_the_text_cannot_hold_is_refused_by_the_printer_and_the_reader() {
    let (ctx, names) = declared();
    // A variable or hypothesis that is not in the context, and a
    // declaration with no name, cannot be printed.
    let stranger = Term::Free(locus::kernel::VarId::fresh());
    assert!(print_term(&stranger, &ctx, &names).is_err());
    assert!(print_proof(&Proof::hyp(locus::kernel::HypId::fresh()), &ctx, &names).is_err());
    let unnamed = Names::new();
    assert!(
        print_term(
            &parse_term("struct:Lock { }", &ctx, &names).unwrap(),
            &ctx,
            &unnamed
        )
        .is_err()
    );
    assert!(
        print_term(
            &parse_term("fn:u8_le_refl", &ctx, &names).unwrap(),
            &ctx,
            &unnamed
        )
        .is_err()
    );
    // Positions past the context, unknown names, and every malformed
    // shape are errors, not panics.
    for text in [
        "$3",
        "$99999999999999999999",
        "#h",
        "h",
        "h99",
        "prop:Nothing()",
        "fn:nothing",
        "struct:Nothing { }",
        "enum:Nothing::0()",
        "enum:Event::99999999999999999999999()",
        "300",
        "-1",
        "256u8",
        "1x",
        "1n2",
        "-1n",
        "(1, 2)",
        "(1 ==[u8] 2",
        "(1 => )",
        "view[Int](1)",
        "cast[u8](1)",
        "add(1, 2)",
        "eq[u8](1, 2).",
        "case 1 : u8 { |x| 1 }",
        "for(1)",
        "absurd(h0) : u8",
        "forall (#: u8) { #0 } extra",
        "",
        "   ",
        "\u{0}",
        "é",
    ] {
        assert!(parse_term(text, &ctx, &names).is_err(), "{text:?}");
    }
    for text in [
        "",
        "hyp(0)",
        "refl",
        "refl()",
        "refl(1, 2)",
        "axiom()",
        "axiom(nothing, 1)",
        "axiom(int_add_comm, 1)",
        "axiom(op_model[add, u8], 1)",
        "axiom(op_model[nothing, u8], 1, 2)",
        "axiom(cmp_reflect[maybe], 1)",
        "axiom(view_lower, 1)",
        "linear(1, 1, [(h0, 1i)])",
        "linear(1, x, [])",
        "case_proof(h0, true, [|1| omitted])",
        "construct(prop:Small, 0, (), ())) ",
        "construct(Small, 0, (), ())",
        "omitted()",
        "int_induction(1, omitted, |1, 1| omitted)",
    ] {
        assert!(parse_proof(text, &ctx, &names).is_err(), "{text:?}");
    }
    // Deep nesting is refused by count, not by the stack, and so is a text
    // over the size limit.
    let deep = "refl(".repeat(100_000);
    assert!(parse_proof(&deep, &ctx, &names).is_err());
    let deep = "(".repeat(100_000);
    assert!(parse_term(&deep, &ctx, &names).is_err());
    let deep = format!("{}1{}", "int_neg(".repeat(100_000), ")".repeat(100_000));
    assert!(parse_term(&deep, &ctx, &names).is_err());
    let deep = format!("{}u8{}", "@(".repeat(100_000), ")".repeat(100_000));
    assert!(parse_type(&deep, &ctx, &names).is_err());
    let wide = format!("({})", vec!["1"; 100_000].join(", "));
    assert!(parse_term(&wide, &ctx, &names).is_err());
    let long = "1".repeat(5000);
    assert!(parse_term(&format!("{long}n"), &ctx, &names).is_err());
    let huge = " ".repeat(locus::store::text::MAX_TEXT + 1);
    assert!(parse_term(&huge, &ctx, &names).is_err());
}

/// The pieces a mutation inserts: every token the text form uses.
const PIECES: &[&str] = &[
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    ",",
    ":",
    "::",
    ".",
    "|",
    "@",
    "#",
    "#0",
    "#h0",
    "$0",
    "$1",
    "h0",
    "h1",
    "==[",
    "=>",
    "->",
    " ",
    "0",
    "1",
    "255",
    "-1",
    "-4i",
    "7u16",
    "-8i8",
    "true",
    "false",
    "u8",
    "Int",
    "Prop",
    "bool",
    "refl",
    "omitted",
    "transport",
    "implies_intro",
    "implies_elim",
    "forall_intro",
    "forall_elim",
    "of_term",
    "projection",
    "literal",
    "definition",
    "case_step",
    "construct",
    "case_proof",
    "case_data",
    "exists_intro",
    "exists_elim",
    "excluded_middle",
    "for_empty",
    "for_step",
    "evaluate",
    "axiom",
    "int_induction",
    "linear",
    "forall",
    "exists",
    "case",
    "absurd",
    "for",
    "proof",
    "fn:",
    "struct:",
    "enum:",
    "prop:",
    "Lock",
    "Event",
    "Small",
    "False",
    "u8_le_trans",
    "view[u8]",
    "wrap[i8]",
    "cast[u8, u16]",
    "add[u8]",
    "lt[u8]",
    "int_le",
    "int_add",
    "cmp_reflect[true]",
    "op_model[add, u8]",
    "view_upper[u8]",
    "int_add_comm",
    "\u{0}",
    "é",
    "\n",
];

#[test]
fn the_reader_never_panics_on_random_or_mutated_text() {
    // The seeds: every proof the corpus stores, each with the context it
    // was printed in and the names of its file.
    let mut seeds: Vec<(Rc<Names>, String, Context, Term)> = Vec::new();
    for path in corpus_files() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = fs::read_to_string(&path).unwrap();
        let (elaborated, store) = run(&name, &text, ProofStore::new());
        let Some(names) = store.names() else {
            continue;
        };
        let names = Rc::new(names.clone());
        for found in elaborated
            .holes
            .iter()
            .filter_map(|hole| hole.found.as_ref())
        {
            if let Ok(printed) = print_proof(&found.proof, &found.context, &names) {
                seeds.push((
                    Rc::clone(&names),
                    printed,
                    found.context.clone(),
                    found.claim.clone(),
                ));
            }
        }
    }
    assert!(seeds.len() >= 60, "{} seeds", seeds.len());

    // Mutations of a seed: bytes deleted, pieces inserted, spans
    // duplicated, characters swapped, digits changed.
    let (mut parsed, mut accepted) = (0, 0);
    for index in 0..cases(MUTATED_PROOFS) {
        let seed = case_seed(SEED, index);
        let mut rng = Rng::new(seed);
        let (names, text, ctx, claim) = rng.choose(&seeds);
        let mut chars: Vec<char> = text.chars().collect();
        for _ in 0..rng.range(1..5) {
            let at = rng.range(0..chars.len() + 1);
            match rng.below(5) {
                0 if at < chars.len() => {
                    let end = (at + rng.range(1..8)).min(chars.len());
                    chars.drain(at..end);
                }
                1 => {
                    let piece: Vec<char> = rng.choose(PIECES).chars().collect();
                    chars.splice(at..at, piece);
                }
                2 if at < chars.len() => {
                    let end = (at + rng.range(1..20)).min(chars.len());
                    let copy: Vec<char> = chars[at..end].to_vec();
                    chars.splice(end..end, copy);
                }
                3 if chars.len() >= 2 => {
                    let other = rng.range(0..chars.len());
                    let at = at.min(chars.len() - 1);
                    chars.swap(at, other);
                }
                _ if at < chars.len() && chars[at].is_ascii_digit() => {
                    chars[at] = char::from(b'0' + rng.below(10) as u8);
                }
                _ => {}
            }
        }
        let mutated: String = chars.into_iter().collect();
        match parse_proof(&mutated, ctx, names) {
            Err(_) => {}
            Ok(proof) => {
                parsed += 1;
                // A parsed proof is not trusted: the kernel judges it, and
                // a judgement either way is fine, a panic is not.
                let mut ctx = ctx.clone();
                if check_proof(&mut ctx, &proof, claim).is_ok() {
                    accepted += 1;
                }
            }
        }
    }
    assert!(parsed > 0, "no mutation parsed");
    assert!(accepted > 0, "no mutation was still a proof");

    // Random bytes, as terms, types, and proofs.
    let (ctx, names) = declared();
    for index in 0..cases(RANDOM_TEXTS) {
        let mut rng = Rng::new(case_seed(SEED ^ 1, index));
        let length = rng.range(0..200);
        let text: String = if rng.chance(1, 2) {
            let bytes: Vec<u8> = (0..length).map(|_| rng.below(256) as u8).collect();
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            (0..length).map(|_| *rng.choose(PIECES)).collect()
        };
        let _ = parse_term(&text, &ctx, &names);
        let _ = parse_type(&text, &ctx, &names);
        let _ = parse_proof(&text, &ctx, &names);
    }
}

#[test]
fn nesting_at_the_limit_fits_a_small_stack_and_beyond_it_is_refused() {
    // The kernel accepts input nested to `MAX_DEPTH`, so the text form
    // must read and write such input; it does so on a 1 MiB stack, in an
    // unoptimized build, for the deepest shapes: one level of nesting per
    // term, per proof, and per term inside a proof inside a term.
    // Each form is one or two levels per repetition around an innermost
    // level, `1` or `omitted`; the whole is within the limit or one over.
    let limit = locus::kernel::MAX_DEPTH;
    for (form, close, innermost, levels, over) in [
        ("int_neg(", ")", "1", 1, false),
        ("(1 => ", ")", "1", 1, false),
        ("implies_elim(omitted, ", ")", "omitted", 1, false),
        ("proof(of_term(", "))", "1", 2, false),
        ("absurd(of_term(", "), u8)", "1", 2, false),
        ("int_neg(", ")", "1", 1, true),
        ("implies_elim(omitted, ", ")", "omitted", 1, true),
        ("proof(of_term(", "))", "1", 2, true),
    ] {
        let depth = (limit - 1) / levels + usize::from(over);
        if std::env::var_os("LOCUS_FUZZ_TRACE").is_some() {
            eprintln!("{form} {depth}");
        }
        let handle = std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(move || {
                let (ctx, names) = declared();
                let text = format!("{}{innermost}{}", form.repeat(depth), close.repeat(depth));
                let term = parse_term(&text, &ctx, &names);
                let proof = parse_proof(&text, &ctx, &names);
                assert_eq!(term.is_ok() || proof.is_ok(), !over, "{form} {depth}");
                // What was read is written again, and reads as the same.
                if let Ok(term) = term {
                    let printed = print_term(&term, &ctx, &names).unwrap();
                    assert_eq!(parse_term(&printed, &ctx, &names).unwrap(), term);
                }
                if let Ok(proof) = proof {
                    let printed = print_proof(&proof, &ctx, &names).unwrap();
                    assert_eq!(parse_proof(&printed, &ctx, &names).unwrap(), proof);
                }
            })
            .unwrap();
        handle.join().unwrap_or_else(|_| panic!("{form} {depth}"));
    }
}
#[test]
fn qualified_names_do_not_confuse_enum_variant_separators() {
    let mut defs = Definitions::default();
    let id = defs
        .declare_fn(&Type::Fn(vec![], Box::new(Type::U8)), |_| Term::U8(7))
        .unwrap();
    let enum_id = defs.declare_enum(&[Type::Tuple(vec![])]).unwrap();
    let mut names = Names::new();
    names.function("View::__model_0", id);
    names.enumeration("library::Event", enum_id);
    let ctx = Context::with_definitions(Rc::new(defs));
    for term in [
        Term::call(Term::Fn(id), vec![]),
        Term::Variant(enum_id, 0, vec![]),
    ] {
        let text = print_term(&term, &ctx, &names).unwrap();
        assert_eq!(parse_term(&text, &ctx, &names).unwrap(), term);
    }
}
