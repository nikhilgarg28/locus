//! The lockfile (E11, P12): every proof the corpus produces is written to
//! `Locus.lock`, read back over a fresh elaboration, and accepted by the
//! kernel; the file is byte-identical under a second run, under
//! reformatting, and under edits elsewhere; a hostile file costs a search
//! or a report and never a false claim; a piece a proof uses twice is
//! written once; and the reader is total on random and mutated text.
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
use locus::store::{Entry, Key, Lockfile, ProofStore, Stats, v1};
use rng::{Rng, case_seed};

const SEED: u64 = 0x4C4F_4355_5300_0012;
const MUTATED_PROOFS: u64 = 3_000;
const MUTATED_FILES: u64 = 60;
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

/// The store of `path` in a lockfile that reads without a warning.
fn read(text: &str, path: &str) -> ProofStore {
    let (mut lockfile, warnings) = Lockfile::parse(text).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    lockfile.take(path)
}

/// A lockfile holding `entries` under `path`.
fn file_of(path: &str, entries: Vec<Entry>) -> String {
    let mut lockfile = Lockfile::new();
    lockfile.insert(path, entries);
    lockfile.render()
}

/// The entries of `path` in a rendered lockfile, by key.
fn entries(rendered: &str, path: &str) -> BTreeMap<String, Entry> {
    read(rendered, path)
        .entries()
        .into_iter()
        .map(|entry| (entry.key.to_string(), entry.clone()))
        .collect()
}

fn lock() -> String {
    fs::read_to_string(root().join("examples/lock.lc")).unwrap()
}

/// The number of step lines in a block, and the number of uses of each
/// step name in the lines after its own.
fn steps_of(block: &str) -> (usize, BTreeMap<String, usize>) {
    let lines: Vec<&str> = block.lines().collect();
    let mut uses = BTreeMap::new();
    for (index, line) in lines.iter().enumerate() {
        let name = line.split(" = ").next().unwrap().to_string();
        let count = lines[index + 1..]
            .iter()
            .map(|later| {
                later
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .filter(|word| *word == name)
                    .count()
            })
            .sum();
        uses.insert(name, count);
    }
    (lines.len(), uses)
}

/// Expand a writer-produced block into the version-1 tree spelling.
fn expanded_tree(block: &str) -> String {
    let mut lines: Vec<(String, String)> = block
        .lines()
        .map(|line| {
            let (name, body) = line.split_once(" = ").unwrap();
            (name.to_string(), body.to_string())
        })
        .collect();
    for index in (0..lines.len()).rev() {
        let (name, body) = lines[index].clone();
        for later in &mut lines[index + 1..] {
            later.1 = later
                .1
                .split_inclusive(|c: char| !c.is_ascii_alphanumeric())
                .map(|piece| {
                    let (word, rest) = piece.split_at(
                        piece
                            .find(|c: char| !c.is_ascii_alphanumeric())
                            .unwrap_or(piece.len()),
                    );
                    if word == name {
                        format!("{body}{rest}")
                    } else {
                        piece.to_string()
                    }
                })
                .collect();
        }
    }
    lines.pop().unwrap().1
}

#[test]
#[doc = "spec: 1.20:1"]
fn every_found_proof_is_written_read_back_and_accepted() {
    let mut files = 0;
    let mut proofs = 0;
    let (mut nodes, mut lines, mut shared) = (0, 0, 0);
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
        let rendered = store.render(&name);
        if let Some(names) = store.names() {
            // Each proof, printed over its own context as a block of
            // steps, reads back as the same proof, which the kernel accepts
            // again.
            for hole in first.holes.iter().filter(|hole| hole.found.is_some()) {
                let found = hole.found.as_ref().unwrap();
                let printed = print_proof(&found.proof, &found.context, names)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                assert!(
                    printed.lines().last().unwrap().starts_with('s'),
                    "{printed}"
                );
                let back = parse_proof(&printed, &found.context, names)
                    .unwrap_or_else(|error| panic!("{name}: {printed}: {error}"));
                assert_eq!(back, found.proof, "{name}: {printed}");
                let mut ctx = found.context.clone();
                check_proof(&mut ctx, &back, &found.claim).unwrap();
                let (count, uses) = steps_of(&printed);
                nodes += hole.proof_size;
                lines += count;
                shared += uses.values().filter(|uses| **uses > 1).count();
                proofs += 1;
            }
        } else {
            assert!(first.holes.iter().all(|hole| hole.found.is_none()));
        }
        // Every entry has a claim, and every claim reads as a term over
        // the context of some hole.
        for entry in store.entries() {
            assert!(!entry.claim.is_empty(), "{name}: {}", entry.label);
        }

        // Read back, with the search forbidden: every obligation is met by
        // its entry, no entry is left over, and the file is rewritten byte
        // for byte.
        let (second, store) = run(
            &name,
            &text,
            read(&rendered, &name).locked(true).searching(false),
        );
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
        assert_eq!(store.render(&name), rendered, "{name}");
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
        // Found again from nothing: the same bytes. The examples and the
        // target files stand for the corpus here, as the fast run is
        // timed.
        if !path.to_string_lossy().contains("corpus/accept") {
            let (_, store) = run(&name, &text, ProofStore::new());
            assert_eq!(store.render(&name), rendered, "{name}: found differently");
        }
        files += 1;
    }
    assert!(files >= 30, "{files} files");
    assert!(proofs >= 60, "{proofs} proofs");
    println!(
        "corpus proofs: {proofs} proofs of {nodes} tree nodes written as {lines} steps, {shared} \
         of them used more than once"
    );
}

#[test]
fn a_piece_used_twice_is_written_once_and_named_in_first_use_order() {
    // The examples' lock, `run 2`: the template `within_limit(#0.0)` is
    // transported along three times and written once; and the target's
    // lock, `remaining 3`, where an equation's proof is used twice.
    let (_, store) = run("lock.lc", &lock(), ProofStore::new());
    let entry = store
        .entries()
        .into_iter()
        .find(|entry| entry.label.to_string() == "run 2")
        .unwrap();
    let (count, uses) = steps_of(&entry.steps);
    assert_eq!(count, 2, "{}", entry.steps);
    assert_eq!(uses["t1"], 3, "{}", entry.steps);
    assert!(
        entry
            .steps
            .starts_with("t1 = fn:within_limit(view[u8](#0.0))\ns1 = ")
    );
    assert_eq!(entry.claim, "fn:within_limit(view[u8]($11.0))");

    let target = fs::read_to_string(root().join("tests/corpus/target/lock.lc")).unwrap();
    let (_, store) = run("lock.lc", &target, ProofStore::new());
    let entry = store
        .entries()
        .into_iter()
        .find(|entry| entry.label.to_string() == "remaining 3")
        .unwrap();
    let (count, uses) = steps_of(&entry.steps);
    assert!(count >= 6, "{}", entry.steps);
    let proofs_used_twice = uses
        .iter()
        .filter(|(name, uses)| name.starts_with('s') && **uses >= 2)
        .count();
    assert!(proofs_used_twice >= 2, "{uses:?}\n{}", entry.steps);
    assert_eq!(
        entry
            .steps
            .matches("transport(h0, (#0 ==[u32] $4), refl($4))")
            .count(),
        1,
        "{}",
        entry.steps
    );
    // Every step is used after it is written, and the names are given in
    // the order the steps are written.
    for (index, line) in entry.steps.lines().enumerate() {
        let name = line.split(" = ").next().unwrap();
        let (kind, number) = name.split_at(1);
        let earlier = entry
            .steps
            .lines()
            .take(index)
            .filter(|line| line.starts_with(kind))
            .count();
        assert_eq!(number.parse::<usize>().unwrap(), earlier + 1, "{name}");
        if index + 1 < count {
            assert!(
                uses[name] >= 2,
                "{name} is a step used {} times",
                uses[name]
            );
        }
    }
}

#[test]
fn the_lockfile_is_no_larger_than_the_sidecars_it_replaced() {
    // Compare the same current certificates in both formats. Language
    // changes add obligations, so an old eight-file byte count is not a
    // valid baseline for a lockfile containing the expanded corpus.
    let (mut lockfiles, mut sidecars) = (0usize, 0usize);
    for directory in ["examples", "tests/corpus/target"] {
        let path = root().join(directory).join("Locus.lock");
        let text =
            fs::read_to_string(&path).unwrap_or_else(|_| panic!("{directory} has no Locus.lock"));
        let (mut lockfile, warnings) = Lockfile::parse(&text).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(!lockfile.is_empty());
        let paths: Vec<_> = lockfile.paths().map(str::to_owned).collect();
        for path in paths {
            let store = lockfile.take(&path);
            sidecars += "locus-proofs 1\n".len();
            for entry in store.entries() {
                let tree = expanded_tree(&entry.steps);
                sidecars += format!("\nobligation {} {}\n{tree}\n", entry.key, entry.label).len();
            }
        }
        lockfiles += text.len();
    }
    println!("lockfiles: {lockfiles} bytes; equivalent tree sidecars: {sidecars} bytes");
    assert!(lockfiles <= sidecars, "{lockfiles} > {sidecars}");
}

#[test]
fn reformatting_comments_and_unrelated_edits_leave_every_entry_in_use() {
    let original = lock();
    let (_, store) = run("lock.lc", &original, ProofStore::new());
    let rendered = store.render("lock.lc");
    let total = store.stats().recorded;
    assert_eq!(total, 8);
    let again = |edited: &str| -> (Stats, String) {
        let (elaborated, store) = run("lock.lc", edited, read(&rendered, "lock.lc"));
        assert!(elaborated.is_success(), "{:?}", elaborated.diagnostics);
        (store.stats(), store.render("lock.lc"))
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
    let (before, after) = (entries(&rendered, "lock.lc"), entries(&text, "lock.lc"));
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
            .all(|(key, entry)| after[key] == *entry)
    );
    let replaced: Vec<&Entry> = after
        .iter()
        .filter(|(key, _)| !before.contains_key(*key))
        .map(|(_, entry)| entry)
        .collect();
    assert_eq!(replaced.len(), 2);
    assert!(
        replaced.iter().all(|entry| entry.label.function == "run"),
        "{replaced:?}"
    );
}

#[test]
#[doc = "spec: 1.20:1"]
fn hostile_files_cost_a_search_or_a_report_and_never_pass_a_false_claim() {
    let source = lock();
    let (_, store) = run("lock.lc", &source, ProofStore::new());
    let rendered = store.render("lock.lc");
    let written: Vec<Entry> = store.used_entries().into_iter().cloned().collect();
    assert_eq!(written.len(), 8);

    // Entries swapped between obligations: two proofs of other claims. The
    // kernel refuses both, the search runs for both, and the file is
    // written right again; under `--locked` they are errors that name the
    // obligation, and say what the entry proves and what is wanted.
    let mut swapped = written.clone();
    let (first, second) = (swapped[1].key, swapped[2].key);
    swapped[1].key = second;
    swapped[2].key = first;
    assert_ne!(written[1].claim, written[2].claim);
    let swapped = file_of("lock.lc", swapped);
    let (elaborated, store) = run("lock.lc", &source, read(&swapped, "lock.lc"));
    assert!(elaborated.is_success());
    accepted_by_the_kernel(&elaborated);
    let stats = store.stats();
    assert_eq!((stats.hits, stats.stale, stats.searches), (6, 2, 2));
    assert_eq!(store.render("lock.lc"), rendered);
    let (elaborated, store) = run("lock.lc", &source, read(&swapped, "lock.lc").locked(true));
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
    // `step` stops at its first obligation that fails, so one stale entry
    // is met: it says what it proves and what was wanted.
    let stale: Vec<(String, String)> = store
        .misses()
        .iter()
        .filter_map(|miss| miss.stale.clone())
        .collect();
    assert_eq!(
        stale,
        [(written[2].claim.clone(), written[1].claim.clone())]
    );
    assert_eq!(store.misses().len(), 1);
    assert_eq!(store.misses()[0].label.to_string(), "step 2");

    // A proof edited by hand to prove something else, proofs of nothing at
    // all, and blocks that do not read: each is refused, and the
    // obligation is searched. A block that reads as the right proof, in
    // some other form, is a hit and is written in the writer's form.
    let evaluations: Vec<usize> = (0..written.len())
        .filter(|&index| {
            matches!(
                written[index].label.to_string().as_str(),
                "run 1" | "step 4"
            )
        })
        .collect();
    assert_eq!(evaluations.len(), 2);
    for (edit, stale) in [
        ("s1 = evaluate(int_le(view[u8](0), view[u8](2)))", 2),
        ("s1 = omitted", 2),
        ("s1 = h0", 2),
        ("s1 = refl(", 2),
        ("s1 = axiom(int_le_refl, view[u8](0))", 2),
        ("s1 = evaluate(int_le(view[u8](0), view[u8](3))) extra", 2),
        // A term where the proof is wanted, and the reverse.
        ("t1 = evaluate(int_le(view[u8](0), view[u8](3)))", 2),
        ("t1 = view[u8](0)\ns1 = t1", 2),
        (
            "s1 = evaluate(int_le(view[u8](0), view[u8](3)))\nt1 = s1",
            2,
        ),
        // A later step, an unknown one, a cycle, a duplicate name, a line
        // that is not a step, and no step at all.
        (
            "s1 = s2\ns2 = evaluate(int_le(view[u8](0), view[u8](3)))",
            2,
        ),
        ("s1 = s7", 2),
        ("s1 = implies_elim(s1, h0)", 2),
        (
            "t1 = view[u8](0)\nt1 = view[u8](3)\ns1 = evaluate(int_le(t1, t1))",
            2,
        ),
        ("evaluate(int_le(view[u8](0), view[u8](3)))\ns1 = s0", 2),
        (
            "s1 = evaluate(int_le(view[u8](0), view[u8](3)))\nnot a step",
            2,
        ),
        ("", 2),
        ("t1 = view[u8](0)", 2),
        // Native IntLe is a different proposition from the held Bool
        // comparison in these obligations, even when both are true.
        (
            "t1 = view[u8](0)\nt2 = view[u8](3)\ns1 = evaluate(int_le(t1, t2))",
            2,
        ),
        (
            "s3 = int_le(view[u8](0), view[u8](3))\ns9 = evaluate(s3)",
            2,
        ),
        (
            "t3 = int_le(view[u8](0), view[u8](3))\ns9 = evaluate(t3)",
            2,
        ),
        ("evaluate(int_le(view[u8](0), view[u8](3)))", 2),
    ] {
        let mut edited = written.clone();
        for &at in &evaluations {
            edited[at].steps = edit.to_string();
        }
        let (elaborated, store) = run(
            "lock.lc",
            &source,
            read(&file_of("lock.lc", edited), "lock.lc"),
        );
        assert!(elaborated.is_success(), "{edit}");
        accepted_by_the_kernel(&elaborated);
        let stats = store.stats();
        assert_eq!(stats.stale, stale, "{edit:?}");
        assert_eq!(stats.hits, 8 - stale, "{edit:?}");
        assert_eq!(store.render("lock.lc"), rendered, "{edit:?}");
    }

    // Alternate DAG names, an extra proof alias, whitespace, and the
    // legacy tree all replay and canonicalize to the writer's form.
    for style in 0..3 {
        let mut edited = written.clone();
        for &at in &evaluations {
            let block = &written[at].steps;
            edited[at].steps = match style {
                0 => expanded_tree(block),
                1 => format!("  {block}  \n\n"),
                _ => {
                    let last = block.lines().last().unwrap().split_once(" = ").unwrap().0;
                    format!("{block}\ns999 = {last}")
                }
            };
        }
        let (elaborated, store) = run(
            "lock.lc",
            &source,
            read(&file_of("lock.lc", edited), "lock.lc"),
        );
        assert!(elaborated.is_success());
        accepted_by_the_kernel(&elaborated);
        assert_eq!((store.stats().hits, store.stats().stale), (8, 0));
        assert_eq!(store.render("lock.lc"), rendered);
    }

    // The entries under another file's path: the source misses every one,
    // and the other file's entries are kept as they are.
    let moved = file_of("other.lc", written.clone());
    let (elaborated, store) = run("lock.lc", &source, read(&moved, "lock.lc"));
    assert!(elaborated.is_success());
    assert_eq!(store.stats().hits, 0);
    assert_eq!(store.stats().searches, 8);
    let (mut lockfile, _) = Lockfile::parse(&moved).unwrap();
    let _ = lockfile.take("lock.lc");
    lockfile.put("lock.lc", &store);
    let both = lockfile.render();
    assert_eq!(entries(&both, "lock.lc"), entries(&rendered, "lock.lc"));
    assert_eq!(entries(&both, "other.lc"), entries(&moved, "other.lc"));
    assert_eq!(read(&both, "other.lc").entries().len(), 8);

    // The file truncated at every length: read without a panic, and what
    // survives is used or refused, never more.
    let mut truncations = 0;
    let mut used = 0;
    let mut refused = 0;
    for length in (0..=rendered.len()).step_by(if extended() { 1 } else { 7 }) {
        let Some(prefix) = rendered.get(..length) else {
            continue;
        };
        match Lockfile::parse(prefix) {
            Err(_) => refused += 1,
            Ok((mut lockfile, _)) => {
                let (elaborated, store) = run("lock.lc", &source, lockfile.take("lock.lc"));
                assert!(elaborated.is_success(), "{length}");
                accepted_by_the_kernel(&elaborated);
                let stats = store.stats();
                assert_eq!(stats.hits + stats.misses, 8, "{length}");
                assert_eq!(store.render("lock.lc"), rendered, "{length}");
                used += stats.hits;
            }
        }
        truncations += 1;
    }
    assert!(truncations > 100, "{truncations}");
    assert!(used > 0);
    assert!(refused > 0);

    // Random bytes, bare and behind a valid head: read without a panic.
    let mut rng = Rng::new(SEED);
    let head = "version = 2\n\n[[file]]\npath = \"lock.lc\"\n\n  [[file.obligation]]\n";
    for _ in 0..cases(RANDOM_TEXTS) {
        let length = rng.range(0..300);
        let bytes: Vec<u8> = (0..length).map(|_| rng.below(256) as u8).collect();
        let text = String::from_utf8_lossy(&bytes).to_string();
        let _ = Lockfile::parse(&text);
        let _ = Lockfile::parse(&format!("version = 2\n{text}"));
        let _ = Lockfile::parse(&format!("{head}{text}"));
        let _ = Lockfile::parse(&format!(
            "{head}  key = \"{text}\"\n  steps = '''\n{text}\n'''\n"
        ));
    }
    println!(
        "hostile lockfiles: {truncations} truncations ({refused} refused as files), {} random texts \
         in 4 framings",
        cases(RANDOM_TEXTS)
    );
}

#[test]
fn the_lockfile_reader_refuses_other_versions_and_skips_what_it_cannot_read() {
    // The pinned TOML parser bounds its own nesting before building an
    // attacker-controlled deeply recursive value (ordinary files are flat).
    let nested = format!(
        "version = 2\nunused = {}0{}\n",
        "[".repeat(81),
        "]".repeat(81)
    );
    let error = Lockfile::parse(&nested).unwrap_err();
    assert!(error.contains("recursion"), "{error}");

    for (text, why) in [
        ("", "no `version`"),
        ("version = 7\n", "version 7"),
        ("version = \"2\"\n", "not a number"),
        ("[[file]]\npath = \"a.lc\"\n", "no `version`"),
        ("not a lockfile\n", "not a TOML lockfile"),
        (
            "version = 2\n[[file]]\npath = \"a.lc\"\n  [[file.obligation]]\n  key = \"x\"\n  steps = '''\n",
            "not a TOML lockfile",
        ),
    ] {
        let problem = Lockfile::parse(text)
            .err()
            .unwrap_or_else(|| panic!("{text:?} was read"));
        assert!(problem.contains(why), "{text:?}: {problem}");
    }
    let (lockfile, warnings) = Lockfile::parse("version = 2\n").unwrap();
    assert!(lockfile.is_empty() && warnings.is_empty());
    assert_eq!(lockfile.render(), "version = 2\n");
    assert!(lockfile.changed().is_none());

    let text = "version = 2\nfile = 3\n";
    let (lockfile, warnings) = Lockfile::parse(text).unwrap();
    assert!(lockfile.is_empty());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let text = r#"version = 2

[[file]]
path = "a.lc"

  [[file.obligation]]
  key = "0123456789abcdef"
  at = "f 1"
  claim = "true"
  steps = "s1 = omitted"

  [[file.obligation]]
  key = "not sixteen hex digits"
  steps = "s1 = omitted"

  [[file.obligation]]
  key = "0123456789abcdef"
  steps = "s1 = h0"

  [[file.obligation]]
  key = "0123456789abcde0"
  at = 7

  [[file.obligation]]
  key = "0123456789abcde1"
  at = "odd"
  steps = '''
s1 = h0
'''

[[file]]
path = "a.lc"

[[file]]
name = "b.lc"

[[file]]
path = "c.lc"
obligation = "none"
"#;
    let (mut lockfile, warnings) = Lockfile::parse(text).unwrap();
    assert_eq!(warnings.len(), 6, "{warnings:#?}");
    assert_eq!(lockfile.paths().collect::<Vec<_>>(), ["a.lc", "c.lc"]);
    let store = lockfile.take("a.lc");
    let entries = store.entries();
    assert_eq!(entries.len(), 2);
    let by_function = |function: &str| {
        *entries
            .iter()
            .find(|entry| entry.label.function == function)
            .unwrap()
    };
    assert_eq!(by_function("f").label.to_string(), "f 1");
    assert_eq!(by_function("f").claim, "true");
    assert_eq!(by_function("f").steps, "s1 = omitted");
    assert_eq!(by_function("odd").label.ordinal, 0);
    assert_eq!(by_function("odd").steps, "s1 = h0");
    // What was read is written in the writer's layout, and reads again.
    let mut written = Lockfile::new();
    written.insert(
        "a.lc",
        vec![by_function("f").clone(), by_function("odd").clone()],
    );
    let rendered = written.render();
    assert!(rendered.starts_with("version = 2\n\n[[file]]\npath = \"a.lc\"\n\n  [[file.obligation]]\n  key = \"0123456789abcdef\"\n  at = \"f 1\"\n  claim = \"true\"\n  steps = '''\ns1 = omitted\n'''\n"), "{rendered}");
    let (again, warnings) = Lockfile::parse(&rendered).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(again.render(), rendered);
    assert!(again.changed().is_none());

    // A path or a block the literal forms cannot hold is escaped, and
    // reads back as it was.
    let odd = Entry {
        key: Key::of("odd"),
        label: locus::store::Label {
            function: "f\"g".into(),
            ordinal: 1,
        },
        claim: "a\\b\"c".into(),
        steps: "s1 = '''\u{7}".into(),
    };
    let rendered = file_of("dir/we\"ird\\.lc", vec![odd.clone()]);
    let (mut again, warnings) = Lockfile::parse(&rendered).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}\n{rendered}");
    let store = again.take("dir/we\"ird\\.lc");
    assert_eq!(store.entries(), [&odd]);
}

#[test]
fn version_1_entries_are_used_and_then_written_as_the_lockfile_writes_them() {
    // The proofs of the examples' lock as version 1 wrote them, keyed as
    // they are now: every entry is a hit, gets its claim, and is written
    // in the steps form, byte for byte as a fresh run writes it.
    let source = lock();
    let (_, store) = run("lock.lc", &source, ProofStore::new());
    let rendered = store.render("lock.lc");
    let mut sidecar = String::from("locus-proofs 1\n");
    for entry in store.used_entries() {
        let tree = expanded_tree(&entry.steps);
        sidecar.push_str(&format!(
            "\nobligation {} {}\n{tree}\n",
            entry.key, entry.label
        ));
    }
    let (entries, warnings) = v1::parse(&sidecar).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(entries.len(), 8);
    assert!(entries.iter().all(|entry| entry.claim.is_empty()));
    let (elaborated, store) = run("lock.lc", &source, ProofStore::with_entries(entries));
    assert!(elaborated.is_success(), "{:?}", elaborated.diagnostics);
    let stats = store.stats();
    assert_eq!((stats.hits, stats.searches, stats.stale), (8, 0, 0));
    assert_eq!(store.render("lock.lc"), rendered);

    // The version-1 reader on its own input, hostile or not.
    assert!(v1::parse("").is_err());
    assert!(v1::parse("locus-proofs 2\n").is_err());
    let (entries, warnings) =
        v1::parse("locus-proofs 1\n\nobligation zz f 1\nh0\n\nobligation 0123456789abcdef f 2\n")
            .unwrap();
    assert!(entries.is_empty());
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    let mut rng = Rng::new(SEED ^ 7);
    for _ in 0..cases(RANDOM_TEXTS / 4) {
        let length = rng.range(0..300);
        let bytes: Vec<u8> = (0..length).map(|_| rng.below(256) as u8).collect();
        let text = String::from_utf8_lossy(&bytes).to_string();
        let _ = v1::parse(&text);
        let _ = v1::parse(&format!("locus-proofs 1\n\nobligation {text}"));
    }
}

#[test]
fn locked_fails_on_a_missing_entry_and_searches_nothing_with_a_complete_file() {
    let source = lock();
    let (_, store) = run("lock.lc", &source, ProofStore::new());
    let rendered = store.render("lock.lc");

    // Complete: no search, every obligation stored.
    let (elaborated, store) = run("lock.lc", &source, read(&rendered, "lock.lc").locked(true));
    assert!(elaborated.is_success());
    let stats = store.stats();
    assert_eq!(stats.searches, 0);
    assert_eq!(stats.hits, 8);
    assert!(elaborated.holes.iter().all(|hole| hole.tier == "stored"));

    // One entry removed: the obligation is named, and nothing is searched.
    let kept: Vec<Entry> = store
        .used_entries()
        .into_iter()
        .filter(|entry| entry.label.to_string() != "run 1")
        .cloned()
        .collect();
    assert_eq!(kept.len(), 7);
    let missing = file_of("lock.lc", kept);
    let (elaborated, store) = run("lock.lc", &source, read(&missing, "lock.lc").locked(true));
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
    assert_eq!(store.misses().len(), 1);
    assert_eq!(store.misses()[0].label.to_string(), "run 1");
    assert!(store.misses()[0].stale.is_none());

    // Surviving an upgrade: with every tier made to fail, the complete
    // file still checks, and an empty store does not.
    let (elaborated, store) = run(
        "lock.lc",
        &source,
        read(&rendered, "lock.lc").searching(false),
    );
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
    assert_eq!(Key::parse("85944171f73967e8"), Some(Key::of("foobar")));
    assert_eq!(Key::parse("85944171F73967E8"), Some(Key::of("foobar")));
    assert_eq!(Key::parse("85944171f73967e"), None);
    assert_eq!(Key::parse("85944171f73967eg"), None);
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
        // As the one step of a proof, and shared: a term used twice is one
        // step, and a leaf is not.
        let twice = Proof::ImpliesElim(
            Box::new(Proof::OfTerm(term.clone())),
            Box::new(Proof::Refl(term.clone())),
        );
        let printed = print_proof(&twice, &ctx, &names).unwrap();
        let back = parse_proof(&printed, &ctx, &names)
            .unwrap_or_else(|error| panic!("{printed}: {error}"));
        assert_eq!(back, twice, "{printed}");
        let (count, uses) = steps_of(&printed);
        let leaf = printed.lines().count() == 1;
        if leaf {
            assert_eq!(count, 1);
        } else {
            assert!(count >= 2, "{printed}");
            let last_term = printed
                .lines()
                .rev()
                .find(|line| line.starts_with('t'))
                .unwrap()
                .split_once(" = ")
                .unwrap()
                .0;
            assert_eq!(uses[last_term], 2, "{printed}");
            let conclusion = printed.lines().last().unwrap().split_once(" = ").unwrap().1;
            assert_eq!(
                conclusion,
                format!("implies_elim(of_term({last_term}), refl({last_term}))")
            );
            for (name, uses) in uses.iter().filter(|(_, uses)| **uses > 0) {
                assert!(*uses >= 2, "unnecessary step {name}: {printed}");
            }
        }
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
        assert!(
            printed.lines().last().unwrap().starts_with('s'),
            "{printed}"
        );
        let back = parse_proof(&printed, &ctx, &names)
            .unwrap_or_else(|error| panic!("{printed}: {error}"));
        assert_eq!(&back, proof, "{printed}");
        // Used twice, a composite proof is one step; a leaf is inline.
        let twice = Proof::ImpliesElim(Box::new(proof.clone()), Box::new(proof.clone()));
        let printed = print_proof(&twice, &ctx, &names).unwrap();
        let back = parse_proof(&printed, &ctx, &names)
            .unwrap_or_else(|error| panic!("{printed}: {error}"));
        assert_eq!(back, twice, "{printed}");
        let leaf = matches!(proof, Proof::Hyp(_) | Proof::Omitted);
        let last = printed.lines().last().unwrap();
        let shared = last.split_once(" = ").unwrap().1;
        assert_eq!(
            shared.starts_with("implies_elim(s") && shared.ends_with(')') && {
                let inner = &shared["implies_elim(".len()..shared.len() - 1];
                let (a, b) = inner.split_once(", ").unwrap();
                a == b
            },
            !leaf,
            "{printed}"
        );
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
    assert!(print_proof(&Proof::Refl(stranger), &ctx, &names).is_err());
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
        // Step names are for blocks; a bare term names none.
        "t1",
        "s1",
        "t1 = 1",
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
        "s1",
        // Blocks: a later step, an unknown one, a cycle, a name twice, a
        // line that is not a step, the wrong kind, a term last, and a
        // bare expression among steps.
        "s1 = s2\ns2 = h0",
        "s1 = implies_elim(h0, s9)",
        "s1 = implies_elim(s1, h0)",
        "t1 = t1",
        "t1 = 1\nt1 = 2\ns1 = refl(t1)",
        "t1 = 1\nrefl(t1)",
        "t1 = 1\ns1 = refl(t1)\n)",
        "t1 = h0\ns1 = t1",
        "s1 = h0\nt1 = s1\ns2 = of_term(t1)",
        "s1 = h0\nt1 = 1",
        "t1 = 1",
        "t1 = 1\ns1 = refl(t1) extra",
        "s1 == h0",
        "s1 => h0",
        "h1 = h0",
        "s1 = h0\n\u{0}",
    ] {
        assert!(parse_proof(text, &ctx, &names).is_err(), "{text:?}");
    }
    // Blocks that read: whitespace, blank lines, any numbering, and
    // steps in either kind.
    for (text, expected) in [
        ("s1 = h0", "h0"),
        ("  s7 =h0  \n\n", "h0"),
        ("t1 = $0\ns1 = refl(t1)", "refl($0)"),
        (
            "t2 = $0\nt1 = ($0 ==[u8] t2)\ns1 = of_term(t1)",
            "of_term(($0 ==[u8] $0))",
        ),
        ("s1 = h0\ns2 = implies_elim(s1, s1)", "implies_elim(h0, h0)"),
        (
            "t1 = 1\ns1 = refl(t1)\nt2 = (t1, t1) : (u8, u8)\ns2 = implies_elim(s1, of_term(t2))",
            "implies_elim(refl(1), of_term((1, 1) : (u8, u8)))",
        ),
    ] {
        let proof =
            parse_proof(text, &ctx, &names).unwrap_or_else(|error| panic!("{text:?}: {error}"));
        assert_eq!(
            proof,
            parse_proof(expected, &ctx, &names).unwrap(),
            "{text:?}"
        );
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
    let huge_block = format!("t1 = 1\n{huge}\ns1 = refl(t1)");
    let error = parse_proof(&huge_block, &ctx, &names).unwrap_err();
    assert!(error.message.contains("MAX_PROOF_TEXT_BYTES"), "{error}");
    for postfix in [".0", "()"] {
        let term = format!("$1{}", postfix.repeat(locus::kernel::MAX_DEPTH));
        let error = parse_term(&term, &ctx, &names).unwrap_err();
        assert!(error.message.contains("MAX_KERNEL_DEPTH"), "{error}");
    }

    // A block whose steps nest, one inside the next, is as deep as the
    // tree it names; one that doubles is as large.
    let chain = |depth: usize| {
        let mut block = String::from("t1 = int_neg($1)\n");
        for index in 2..=depth {
            block.push_str(&format!("t{index} = int_neg(t{})\n", index - 1));
        }
        block.push_str(&format!("s1 = of_term(t{depth})"));
        block
    };
    assert!(parse_proof(&chain(200), &ctx, &names).is_ok());
    let error = parse_proof(&chain(300), &ctx, &names).unwrap_err();
    assert!(error.message.contains("MAX_KERNEL_DEPTH"), "{error}");
    let doubling = |lines: usize| {
        let mut block = String::from("t1 = int_add($1, $1)\n");
        for index in 2..=lines {
            block.push_str(&format!(
                "t{index} = int_add(t{}, t{})\n",
                index - 1,
                index - 1
            ));
        }
        block.push_str(&format!("s1 = of_term(t{lines})"));
        block
    };
    assert!(parse_proof(&doubling(15), &ctx, &names).is_ok());
    // Each alias is individually bounded, but all retained expansions
    // must also fit the one block's allocation budget.
    let mut retained = doubling(15);
    for index in 2..=40 {
        retained.push_str(&format!("\ns{index} = s1"));
    }
    let error = parse_proof(&retained, &ctx, &names).unwrap_err();
    assert!(
        error.message.contains("MAX_PROOF_EXPANDED_NODES"),
        "{error}"
    );

    let error = parse_proof(&doubling(25), &ctx, &names).unwrap_err();
    assert!(
        error.message.contains("MAX_PROOF_EXPANDED_NODES"),
        "{error}"
    );
    let error = parse_proof(&doubling(2_000), &ctx, &names).unwrap_err();
    assert!(
        error.message.contains("MAX_PROOF_EXPANDED_NODES"),
        "{error}"
    );
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
    // The steps form.
    "t1",
    "t2",
    "s1",
    "s2",
    " = ",
    "=",
    "\nt1 = ",
    "\ns1 = ",
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
    // duplicated, characters swapped, digits changed, lines swapped.
    let (mut parsed, mut accepted) = (0, 0);
    for index in 0..cases(MUTATED_PROOFS) {
        let seed = case_seed(SEED, index);
        let mut rng = Rng::new(seed);
        let (names, text, ctx, claim) = rng.choose(&seeds);
        let mut chars: Vec<char> = text.chars().collect();
        for _ in 0..rng.range(1..5) {
            let at = rng.range(0..chars.len() + 1);
            match rng.below(6) {
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
                4 => {
                    let mut lines: Vec<String> = chars
                        .iter()
                        .collect::<String>()
                        .lines()
                        .map(str::to_string)
                        .collect();
                    if lines.len() >= 2 {
                        let (a, b) = (rng.range(0..lines.len()), rng.range(0..lines.len()));
                        lines.swap(a, b);
                        chars = lines.join("\n").chars().collect();
                    }
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
    println!(
        "mutated blocks: {} cases, {parsed} read as proofs, {accepted} still proofs",
        cases(MUTATED_PROOFS)
    );

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

    // Mutations of a whole lockfile: the examples' lock with pieces of
    // TOML and of the text form inserted, deleted, and swapped; each reads
    // as a lockfile or a reason, and what it holds is used or refused.
    let source = lock();
    let (_, store) = run("lock.lc", &source, ProofStore::new());
    let rendered = store.render("lock.lc");
    let toml_pieces: &[&str] = &[
        "[[file]]",
        "[[file.obligation]]",
        "path = \"lock.lc\"",
        "key = \"",
        "at = \"",
        "claim = \"",
        "steps = '''",
        "'''",
        "\"",
        "'",
        "\n",
        "  ",
        "version = 2",
        "version = 3",
        "[",
        "]",
        "=",
        "#",
        "\\",
        "\u{0}",
        "é",
        "t1",
        "s1",
        " = ",
        "\nt1 = ",
        "\ns1 = ",
        "h0",
        "refl(",
        ")",
    ];
    let (mut files, mut refused, mut hits, mut stale) = (0, 0, 0, 0);
    for index in 0..cases(MUTATED_FILES) {
        let mut rng = Rng::new(case_seed(SEED ^ 2, index));
        let mut chars: Vec<char> = rendered.chars().collect();
        for _ in 0..rng.range(1..6) {
            let at = rng.range(0..chars.len() + 1);
            match rng.below(4) {
                0 if at < chars.len() => {
                    let end = (at + rng.range(1..12)).min(chars.len());
                    chars.drain(at..end);
                }
                1 => {
                    let piece: Vec<char> = rng.choose(toml_pieces).chars().collect();
                    chars.splice(at..at, piece);
                }
                2 if chars.len() >= 2 => {
                    let other = rng.range(0..chars.len());
                    let at = at.min(chars.len() - 1);
                    chars.swap(at, other);
                }
                _ if at < chars.len() && chars[at].is_ascii_alphanumeric() => {
                    chars[at] = rng
                        .choose(&['0', '9', 'a', 'f', 'x', 's', 't', '1'])
                        .to_owned();
                }
                _ => {}
            }
        }
        let mutated: String = chars.into_iter().collect();
        match Lockfile::parse(&mutated) {
            Err(_) => refused += 1,
            Ok((mut lockfile, _)) => {
                let (elaborated, store) = run("lock.lc", &source, lockfile.take("lock.lc"));
                assert!(elaborated.is_success(), "{mutated}");
                accepted_by_the_kernel(&elaborated);
                let stats = store.stats();
                assert_eq!(stats.hits + stats.misses, 8, "{mutated}");
                assert_eq!(store.render("lock.lc"), rendered, "{mutated}");
                hits += stats.hits;
                stale += stats.stale;
            }
        }
        files += 1;
    }
    assert!(
        refused > 0 && hits > 0 && stale > 0,
        "{refused} {hits} {stale}"
    );
    println!("mutated lockfiles: {files} cases, {refused} refused, {hits} hits, {stale} stale");
}

/// Every hit is a proof the kernel accepts again, over its context, of its
/// claim.
fn accepted_by_the_kernel(elaborated: &Elaborated) {
    for hole in elaborated.holes.iter().filter(|hole| hole.tier == "stored") {
        let found = hole.found.as_ref().unwrap();
        let mut ctx = found.context.clone();
        check_proof(&mut ctx, &found.proof, &found.claim).unwrap();
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
                    let last = printed.lines().last().unwrap().split_once(" = ").unwrap().0;
                    let aliased = format!("{printed}\ns999 = {last}");
                    assert_eq!(parse_proof(&aliased, &ctx, &names).unwrap(), proof);
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
