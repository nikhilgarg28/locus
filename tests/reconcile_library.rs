//! Library abstractions use ordinary source declarations and checked evidence.
use locus::{
    elab::{Elaborated, Options, elaborate_with_options},
    parser::parse,
    source::SourceMap,
};
const LOGICAL: &str = include_str!("../library/logical.lc");
const MAP: &str = include_str!("../library/finite_map.lc");
fn check(parts: &[&str]) -> Elaborated {
    let text = parts.join("\n");
    let mut sources = SourceMap::default();
    let file = sources.add("library_acceptance.lc", text);
    let source = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let mut options = Options::default();
    for feature in [
        locus::preview::Feature::LogicalData,
        locus::preview::Feature::HeapViews,
    ] {
        if feature.status() == locus::preview::Status::Preview {
            options.previews.enable(feature.name()).unwrap();
        }
    }
    elaborate_with_options(source, &parsed.program, &options)
}
fn accept(parts: &[&str]) -> Elaborated {
    let result = check(parts);
    assert!(
        result.is_success(),
        "{}",
        result
            .diagnostics
            .iter()
            .map(|d| format!("{} {} @ {:?}", d.code, d.message, d.labels[0].span))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(locus::erased::check_module(result.session.erased()), Ok(()));
    result
}
#[test]
fn finite_map_packages_its_unique_keys_invariant() {
    accept(&[LOGICAL, MAP, "logic fn singleton() -> FiniteMap<Int, Int> {
        let map: FiniteMap<Int, Int> = map_empty();
        let entry: Entry<Int, Int> = Entry { key: 3, value: 7 };
        let empty: @(map.entries == Seq::Empty) = fold!(map_empty::<Int, Int>, prove!(Seq::<Entry<Int, Int>>::Empty == Seq::<Entry<Int, Int>>::Empty));
        let absent: @KeyAbsent(3, map.entries) = KeyAbsent::Empty @ empty;
        map_prepend(map, entry, absent)
    }"]);
}
#[test]
fn missing_map_invariant_is_rejected() {
    let result = check(&[LOGICAL, MAP, "logic fn bad() -> FiniteMap<Int, Int> {
        let entry: Entry<Int, Int> = Entry { key: 3, value: 7 };
        let entries: Seq<Entry<Int, Int>> = Seq::Cons { head: entry, tail: Seq::Cons { head: entry, tail: Seq::Empty } };
        FiniteMap { entries, unique: UniqueKeys::Empty @ prove!(entries == Seq::Empty) }
    }"]);
    assert!(!result.is_success());
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0230"),
        "{}",
        result
            .diagnostics
            .iter()
            .map(|d| format!("{} {} @ {:?}", d.code, d.message, d.labels[0].span))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
#[test]
fn named_membership_and_recursive_reachability_use_checked_arm_bodies() {
    accept(&[LOGICAL, include_str!("../library/relations.lc"), "logic fn membership() -> @Contains(7, Seq::Cons { head: 7, tail: Seq::Empty }) {
        contains_head(7, Seq::Empty)
    }
    logic fn path() -> @(1 <= 3) {
        let same: @Reachable(3, 3) = Reachable::Same @ prove!(3 == 3);
        let step: @Reachable(1, 3) = Reachable::Via { middle: 3 } @ And::Intro(prove!(1 <= 3), same);
        reachable_ordered(1, 3, step)
    }"]);
}
#[test]
fn library_integer_representation_matches_native_arithmetic_on_a_checked_fragment() {
    accept(&[LOGICAL, include_str!("../library/integer.lc"), "logic fn negative_two() -> @(integer_model(LibraryInt::Negative(Peano::Succ(Peano::Zero))) == -2) {
        prove!(integer_model(LibraryInt::Negative(Peano::Succ(Peano::Zero))) == -2)
    }
    logic fn positive_two() -> @(integer_model(LibraryInt::NonNegative(Peano::Succ(Peano::Succ(Peano::Zero)))) == 2) {
        prove!(integer_model(LibraryInt::NonNegative(Peano::Succ(Peano::Succ(Peano::Zero)))) == 2)
    }"]);
}
#[test]
#[doc = "spec: 1.25:1"]
fn directly_recursive_data_is_inductive_and_negative_recursion_is_rejected() {
    accept(&[
        LOGICAL,
        "logic fn length_nonnegative(items: Seq<Int>) -> @(seq_length(items) >= 0) { seq_nonnegative(items) }",
    ]);
    let rejected = check(&["#[derive(Logical)] enum Bad { Next(logic Fn(Bad) -> Int) }"]);
    assert!(
        rejected.diagnostics.iter().any(|d| d.code == "L0203"),
        "{:#?}",
        rejected.diagnostics
    );
}
#[test]
#[doc = "spec: 1.92:11"]
fn a_runtime_boxed_list_has_a_separate_persistent_logical_model() {
    let checked = accept(&[
        LOGICAL,
        include_str!("../library/runtime_list.lc"),
        "fn run() -> u8 {
        let list = RuntimeList::Cons(1, Box::new(RuntimeList::Empty));
        let observed = list as Seq<Peano>;
        let consumed = list;
        let length = prove!(seq_length(observed) == 1);
        7
    }",
    ]);
    let module = checked.session.erased();
    let value = locus::erased::Interpreter::new(module, 10000)
        .call(checked.function("run").unwrap(), vec![])
        .unwrap();
    assert_eq!(value.debug(module), "7");
    let rust = locus::erased::print_module(module);
    assert!(rust.contains("Box<RuntimeList>"), "{rust}");
    assert!(
        !rust.contains("enum Peano") && !rust.contains("enum __LocusSeq"),
        "{rust}"
    );
}
#[test]
fn box_of_logical_data_is_still_physical_and_cannot_derive_logical() {
    let result = check(&["#[derive(Logical)] enum Peano { Zero, Succ(Peano) }
        #[derive(Logical)] struct Bad { value: Box<Peano> }"]);
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0243"),
        "{:#?}",
        result.diagnostics
    );
}
#[test]
#[doc = "spec: 2.35:6"]
fn ordinary_buffer_model_proves_length_and_keeps_before_after_snapshots() {
    let checked = accept(&[LOGICAL, include_str!("../library/buffer_model.lc"), "fn run() -> u8 {
        let mut values: Vec<u8> = Vec::from([3, 4]);
        let before = values as Seq<Int>;
        values.push(7);
        let after = values as Seq<Int>;
        let length = buffer_model_length(&values);
        let appended = prove!(after == seq_append(before, Seq::<Int>::Cons { head: 7, tail: Seq::Empty }));
        values.get(2)
    }"]);
    let module = checked.session.erased();
    let function = checked.function("run").unwrap();
    let outcome = locus::erased::Interpreter::new(module, 10000)
        .call(function, vec![])
        .unwrap();
    let checked_outcome = locus::exec::CheckInterpreter::new(checked.session.program(), 10000)
        .with_lending(checked.session.lending())
        .call(function, vec![])
        .unwrap();
    assert_eq!(outcome, checked_outcome);
    assert_eq!(outcome.debug(module), "7");
}
#[test]
fn generic_buffer_observation_uses_a_checked_user_element_model() {
    accept(&[LOGICAL, include_str!("../library/buffer_model.lc"), "struct Item { key: u8 }
        impl Model for Item { type Logic = Int; logic fn model(source: &Item) -> Int { model!(source.key) as Int } }
        logic fn inspect(source: &[Item]) -> Seq<Int> {
            let count = source.len();
            let bounds: @(0 <= 0 && 0 <= count && 0 + count <= source.len()) =
                And::Intro(And::Intro(prove!(0 <= 0), prove!(0 <= count)), prove!(0 + count <= source.len()));
            buffer_segment::<Item, Int>(&source, 0, count, bounds)
        }
    "]);
}
#[test]
fn logical_library_function_and_lemma_inventory_is_explicit() {
    let mut sources = SourceMap::default();
    let file = sources.add("logical.lc", LOGICAL);
    let parsed = parse(sources.get(file));
    assert!(parsed.is_success());
    let names: Vec<_> = parsed
        .program
        .declarations
        .iter()
        .filter_map(|declaration| match &declaration.kind {
            locus::ast::DeclarationKind::Function {
                name,
                logical: true,
                ..
            } => Some(name.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        [
            "nat_to_int",
            "nat_nonnegative",
            "seq_length",
            "seq_get",
            "seq_append",
            "seq_map",
            "seq_nonnegative",
            "nat_from_int",
            "nat_to_from_int",
            "nat_from_to_int",
            "nat_from_nonnegative",
            "seq_append_length"
        ]
    );
}

#[test]
fn buffer_model_length_propagates_across_symbolic_push() {
    accept(&[
        LOGICAL,
        include_str!("../library/buffer_model.lc"),
        "fn append(values: &mut Vec<u8>, item: u8) -> () {
        let before = values as Seq<Int>;
        let old_length = buffer_model_length(&values);
        values.push(item);
        let after = values as Seq<Int>;
        let new_length = buffer_model_length(&values);
        let growth = prove!(seq_length(after) == seq_length(before) + 1);
        ()
    }",
    ]);
}

#[test]
fn false_snapshot_equations_cannot_be_obtained_by_retargeting_bound_evidence() {
    let result = check(&[
        LOGICAL,
        include_str!("../library/buffer_model.lc"),
        "fn bad() -> () {
        let mut values: Vec<u8> = Vec::from([]);
        let before = values as Seq<Int>;
        values.push(7);
        let after = values as Seq<Int>;
        let false_equation = prove!(after == before);
        ()
    }",
    ]);
    assert!(!result.is_success());
    assert!(result.diagnostics.iter().any(|d| d.code == "L0230"));
}

#[test]
#[doc = "spec: 1.25:2"]
fn append_length_is_proved_by_structural_induction_for_arbitrary_sequences() {
    accept(&[
        LOGICAL,
        "logic fn lengths(xs: Seq<Int>, ys: Seq<Int>)
        -> @(seq_length(seq_append(xs, ys)) == seq_length(xs) + seq_length(ys)) {
        seq_append_length(xs, ys)
    }",
    ]);
}

#[test]
fn constructor_fold_cannot_turn_reflexivity_into_a_wrong_length_law() {
    let rejected = check(&[
        LOGICAL,
        "logic fn wrong(head: Int, tail: Seq<Int>)
        -> @(seq_length(Seq::<Int>::Cons { head, tail }) == 2 + seq_length(tail)) {
        fold!(seq_length::<Int>, prove!(0 == 0))
    }",
    ]);
    assert!(!rejected.is_success());
    assert!(rejected.diagnostics.iter().any(|d| d.code == "L0230"));
}
