//! Source checks for Logical derivation and logical recursive data.
use locus::elab::{Elaborated, Options, elaborate_with_options};
use locus::preview::{Feature, Status};
use locus::source::SourceMap;

fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let file = sources.add("logical_data.lc", text);
    let source = sources.get(file);
    let parsed = locus::parser::parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    for feature in [
        Feature::LogicalSplit,
        Feature::LogicalData,
        Feature::NamedProps,
    ] {
        if feature.status() == Status::Preview {
            options.previews.enable(feature.name()).unwrap();
        }
    }
    elaborate_with_options(source, &parsed.program, &options)
}

#[test]
fn logical_structs_and_enums_have_no_emitted_declaration_or_tag() {
    let checked = check(
        r#"
#[derive(Logical)]
struct Snapshot { number: Int, flag: Bool }
#[derive(Logical)]
enum Choice { Empty, Value(Int) }
logic fn first() -> Snapshot { Snapshot { number: 1, flag: true } }
logic fn empty() -> Choice { Choice::Empty }
fn caller() -> u8 { let snapshot = first(); let choice = empty(); 7 }
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    assert!(checked.session.erased().structs.is_empty());
    assert!(checked.session.erased().enums.is_empty());
    assert_eq!(checked.session.erased().fns.len(), 1);
}

#[test]
fn logical_derivation_refuses_runtime_fields() {
    for field in ["u8", "bool"] {
        let checked = check(&format!(
            "#[derive(Logical)] struct Bad {{ value: {field} }}"
        ));
        assert!(!checked.is_success());
        assert!(
            checked.diagnostics.iter().any(|d| d.code == "L0243"),
            "{:?}",
            checked.diagnostics
        );
    }
    let checked =
        check("struct Runtime { value: Int } #[derive(Logical)] struct Bad { value: Runtime }");
    assert!(!checked.is_success());
    assert!(checked.diagnostics.iter().any(|d| d.code == "L0243"));
}

#[test]
fn an_ordinary_enum_retains_its_tag_around_a_logical_payload() {
    let checked = check(
        r#"
#[derive(Logical)] struct Snapshot { value: Int }
enum Maybe { None, Some(Snapshot) }
fn none() -> Maybe { Maybe::None }
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    assert!(checked.session.erased().structs.is_empty());
    assert_eq!(checked.session.erased().enums.len(), 1);
    assert_eq!(checked.session.erased().enums[0].name, "Maybe");
}

#[test]
fn finite_recursive_logical_data_needs_no_runtime_indirection() {
    let checked = check(
        r#"
#[derive(Logical)] enum Peano { Zero, Next(Peano) }
#[derive(Logical)] enum Seq { Empty, Push { head: Int, tail: Seq } }
logic fn two() -> Peano { Peano::Next(Peano::Next(Peano::Zero)) }
logic fn single() -> Seq { Seq::Push { head: 7, tail: Seq::Empty } }
logic fn head(xs: Seq) -> Int { match xs { Seq::Empty => 0, Seq::Push { head, tail } => head } }
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    assert!(checked.session.erased().enums.is_empty());
    assert!(checked.session.erased().fns.is_empty());
}

#[test]
fn source_recursive_payloads_reject_negative_or_nested_occurrences() {
    for payload in ["logic Fn(Bad) -> Int", "(Int, Bad)"] {
        let checked = check(&format!(
            "#[derive(Logical)] enum Bad {{ More({payload}) }}"
        ));
        assert!(!checked.is_success());
        assert!(
            checked.diagnostics.iter().any(|d| d.code == "L0203"),
            "{:?}",
            checked.diagnostics
        );
    }
}

#[test]
fn source_logic_functions_recurse_only_on_matched_descendants() {
    let checked = check(
        r#"
#[derive(Logical)] enum Seq { Empty, Push { head: Int, tail: Seq } }
logic fn length(xs: Seq) -> Int {
    match xs { Seq::Empty => 0, Seq::Push { head, tail } => 1 + length(tail) }
}
logic fn append(xs: Seq, ys: Seq) -> Seq {
    match xs { Seq::Empty => ys, Seq::Push { head, tail } => Seq::Push { head, tail: append(tail, ys) } }
}
logic fn singleton() -> Seq { Seq::Push { head: 7, tail: Seq::Empty } }
logic fn count_two() -> Int { length(append(singleton(), singleton())) }
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    assert!(checked.session.erased().fns.is_empty());
    let locus::typed::FnRef::Math(id) = checked.function("count_two").unwrap() else {
        panic!()
    };
    let mut ctx = locus::kernel::Context::with_definitions(std::rc::Rc::new(
        checked.session.program().definitions().clone(),
    ));
    let value = locus::kernel::Term::call(locus::kernel::Term::Fn(id), vec![]);
    assert!(
        locus::kernel::check_proof(
            &mut ctx,
            &locus::kernel::Proof::Evaluate(value.clone()),
            &locus::kernel::Term::eq(locus::kernel::Type::Int, value, locus::kernel::Term::int(2))
        )
        .is_ok()
    );
    let bad = check(
        "#[derive(Logical)] enum Peano { Zero, Next(Peano) } logic fn bad(n: Peano) -> Int { bad(n) }",
    );
    assert!(!bad.is_success());
    assert!(
        bad.diagnostics.iter().any(|d| d.code == "L0203"),
        "{:?}",
        bad.diagnostics
    );
}

#[test]
fn recursive_logic_lemmas_prove_properties_by_structural_induction() {
    let checked = check(
        r#"
#[derive(Logical)] enum Seq { Empty, Push { head: Int, tail: Seq } }
logic fn length(xs: Seq) -> Int {
    match xs { Seq::Empty => 0, Seq::Push { head, tail } => 1 + length(tail) }
}
logic fn length_nonnegative(xs: Seq) -> @(length(xs) >= 0) {
    match xs {
        Seq::Empty => fold!(length, prove!(0 >= 0)),
        Seq::Push { head, tail } => {
            let ih = length_nonnegative(tail);
            fold!(length, prove!(1 + length(tail) >= 0))
        }
    }
}
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
}

#[test]
fn source_inductive_predicates_construct_reachable_and_reject_negation() {
    let checked = check(
        r#"
prop Reachable(from: Int, to: Int) {
    Same => { prop!(from == to) }
    Via(middle: Int) => { prop!(from <= middle && Reachable(middle, to)) }
}
logic fn same(n: Int) -> @Reachable(n, n) { Reachable::Same @ prove!(n == n) }
logic fn step(a: Int, b: Int, ordered: @(a <= b)) -> @Reachable(a, b) {
    let tail: @Reachable(b, b) = same(b);
    Reachable::Via(b) @ And::Intro(ordered, tail)
}
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    let bad = check("prop Bad(n: Int) { Contradiction => { prop!(!Bad(n)) } }");
    assert!(!bad.is_success());
    assert!(
        bad.diagnostics.iter().any(|d| d.code == "L0203"),
        "{:?}",
        bad.diagnostics
    );
    let hidden = check(
        "logic fn hide(p: Prop) -> Prop { p } prop Bad(n: Int) { Hidden => { hide(Bad(n)) } }",
    );
    assert!(!hidden.is_success());
    assert!(
        hidden.diagnostics.iter().any(|d| d.code == "L0203"),
        "{:?}",
        hidden.diagnostics
    );
}

#[test]
fn recursive_evidence_lemmas_induct_over_named_predicate_bodies() {
    let checked = check(
        r#"
prop Reachable(from: Int, to: Int) {
    Same => { prop!(from == to) }
    Via(middle: Int) => { prop!(from <= middle && Reachable(middle, to)) }
}
logic fn ordered(a: Int, b: Int, path: @Reachable(a, b)) -> @(a <= b) {
    match path {
        Reachable::Same @ same => prove!(a <= b),
        Reachable::Via(middle) @ body => {
            match body {
                And::Intro(first, tail) => {
                    let rest = ordered(middle, b, tail);
                    prove!(a <= b)
                }
            }
        }
    }
}
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
}

#[test]
fn logical_case_refinement_does_not_prove_false_branch_goals_or_fake_descent() {
    let checked = check(
        r#"
#[derive(Logical)] enum Peano { Zero, Next(Peano) }
logic fn false_claim(n: Peano) -> @(false) {
    match n { Peano::Zero => prove!(false), Peano::Next(previous) => false_claim(previous) }
}
"#,
    );
    assert!(!checked.is_success());
    let checked = check(
        r#"
prop Reachable(a: Int, b: Int) { Same => { prop!(a == b) } Via(n: Int) => { prop!(Reachable(n,b)) } }
logic fn bad(a: Int, b: Int, h: @Reachable(a,b)) -> @(false) { bad(a,b,h) }
"#,
    );
    assert!(!checked.is_success());
    assert!(
        checked.diagnostics.iter().any(|d| d.code == "L0203"),
        "{:?}",
        checked.diagnostics
    );
}

#[test]
fn checked_int_measures_construct_library_naturals_from_ints() {
    let checked = check(
        r#"
#[derive(Logical)] enum Peano { Zero, Next(Peano) }
logic fn from_int(n: Int) -> Peano {
    if n <= 0 { Peano::Zero } else {
        let next = n - 1;
        let smaller: @(0 <= next && next < n) = And::Intro(prove!(0 <= next), prove!(next < n));
        Peano::Next(recurse!(smaller, from_int(next)))
    }
}
logic fn to_int(n: Peano) -> Int { match n { Peano::Zero => 0, Peano::Next(previous) => 1 + to_int(previous) } }
logic fn seven() -> Int { to_int(from_int(7)) }
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    let locus::typed::FnRef::Math(id) = checked.function("seven").unwrap() else {
        panic!()
    };
    let mut ctx = locus::kernel::Context::with_definitions(std::rc::Rc::new(
        checked.session.program().definitions().clone(),
    ));
    let call = locus::kernel::Term::call(locus::kernel::Term::Fn(id), vec![]);
    assert!(
        locus::kernel::check_proof(
            &mut ctx,
            &locus::kernel::Proof::Evaluate(call.clone()),
            &locus::kernel::Term::eq(locus::kernel::Type::Int, call, locus::kernel::Term::int(7))
        )
        .is_ok()
    );
}

#[test]
fn userland_nat_correspondence_is_checked() {
    use locus::kernel::{CmpOp, Context, Mode, Proof, Term, Type, check_proof, infer_term};
    let source = include_str!("../library/logical.lc");
    let checked = check(source);
    assert!(checked.is_success(), "{:#?}", checked.diagnostics);
    let function = |name| match checked.function(name).unwrap() {
        locus::typed::FnRef::Math(id) => Term::Fn(id),
        _ => panic!("logical declaration"),
    };
    let mut ctx = Context::with_definitions(std::rc::Rc::new(
        checked.session.program().definitions().clone(),
    ));
    for n in [0, 1, 7, 16] {
        let value = Term::int(n);
        let nat = Term::call(function("nat_from_int"), vec![value.clone()]);
        let converted = Term::call(function("nat_to_int"), vec![nat.clone()]);
        assert_eq!(
            check_proof(
                &mut ctx,
                &Proof::Evaluate(converted.clone()),
                &Term::eq(Type::Int, converted, value.clone())
            ),
            Ok(())
        );
        let nonnegative = Proof::Evaluate(Term::int_cmp(CmpOp::Le, Term::int(0), value.clone()));
        for call in [
            Term::call(
                function("nat_to_from_int"),
                vec![value.clone(), Term::proof(nonnegative.clone())],
            ),
            Term::call(function("nat_from_to_int"), vec![nat]),
            Term::call(
                function("nat_from_nonnegative"),
                vec![value, Term::proof(nonnegative)],
            ),
        ] {
            assert!(infer_term(&mut ctx, &call, Mode::Logical).is_ok());
        }
    }
    let bad = Term::call(
        function("nat_from_nonnegative"),
        vec![
            Term::int(-1),
            Term::proof(Proof::Evaluate(Term::int_cmp(
                CmpOp::Le,
                Term::int(0),
                Term::int(-1),
            ))),
        ],
    );
    assert!(infer_term(&mut ctx, &bad, Mode::Logical).is_err());
    let incorrect = source.replace(
        "pub logic fn nat_from_to_int(n: Peano) -> @(nat_from_int(nat_to_int(n)) == n)",
        "pub logic fn nat_from_to_int(n: Peano) -> @(nat_from_int(nat_to_int(n)) == Peano::Succ(n))",
    );
    assert!(!check(&incorrect).is_success());
}

#[test]
fn inductive_predicates_accept_only_positive_library_quantifier_bodies() {
    let good = check(
        r#"
prop Branching(n: Int) {
    Leaf => { prop!(true) },
    Some => { prop!(exists (child: Int) { Branching(child) }) },
    All => { prop!(forall (child: Int) { Branching(child) }) },
}
"#,
    );
    assert!(good.is_success(), "{:#?}", good.diagnostics);
    let bad = check(
        r#"
prop Bad(n: Int) {
    Negative => { prop!(forall (child: Int) { !Bad(child) }) },
}
"#,
    );
    assert!(!bad.is_success());
    assert!(
        bad.diagnostics.iter().any(|d| d.code == "L0203"),
        "{:#?}",
        bad.diagnostics
    );
}

#[test]
fn mutual_logical_tree_and_forest_are_declared_atomically() {
    let checked = check(
        r#"
#[derive(Logical)] enum Tree<T: Logical> { Leaf(T), Branch(Forest<T>) }
#[derive(Logical)] enum Forest<T: Logical> { Empty, Cons { tree: Tree<T>, rest: Forest<T> } }
logic fn first(tree: Tree<Int>) -> Int {
    match tree {
        Tree::Leaf(value) => value,
        Tree::Branch(forest) => match forest {
            Forest::Empty => 0,
            Forest::Cons { tree, rest } => first(tree),
        },
    }
}
logic fn sample() -> Int {
    first(Tree::Branch(Forest::Cons { tree: Tree::Leaf(7), rest: Forest::Empty }))
}
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    assert!(checked.session.erased().enums.is_empty());
    assert!(checked.session.erased().fns.is_empty());
    let locus::typed::FnRef::Math(id) = checked.function("sample").unwrap() else {
        panic!()
    };
    let mut ctx = locus::kernel::Context::with_definitions(std::rc::Rc::new(
        checked.session.program().definitions().clone(),
    ));
    let value = locus::kernel::Term::call(locus::kernel::Term::Fn(id), vec![]);
    locus::kernel::check_proof(
        &mut ctx,
        &locus::kernel::Proof::Evaluate(value.clone()),
        &locus::kernel::Term::eq(locus::kernel::Type::Int, value, locus::kernel::Term::int(7)),
    )
    .unwrap();
}

#[test]
fn structural_proof_recursion_descends_across_mutual_data_members() {
    let checked = check(
        r#"
#[derive(Logical)] enum Tree { Leaf, Branch(Forest) }
#[derive(Logical)] enum Forest { Empty, Cons(Tree, Forest) }
logic fn depth(tree: Tree) -> Int {
    match tree {
        Tree::Leaf => 0,
        Tree::Branch(forest) => match forest {
            Forest::Empty => 0,
            Forest::Cons(child, rest) => 1 + depth(child),
        },
    }
}
logic fn nonnegative(tree: Tree) -> @(depth(tree) >= 0) {
    match tree {
        Tree::Leaf => fold!(depth, prove!(0 >= 0)),
        Tree::Branch(forest) => match forest {
            Forest::Empty => fold!(depth, prove!(0 >= 0)),
            Forest::Cons(child, rest) => {
                let smaller = nonnegative(child);
                fold!(depth, prove!(1 + depth(child) >= 0))
            },
        },
    }
}
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    assert!(checked.function("nonnegative").is_some());
}

#[test]
fn mutual_data_rejects_negative_nested_and_runtime_members() {
    for field in ["logic Fn(Right) -> Int", "(Int, Right)", "Box<Right>"] {
        let source = format!(
            "#[derive(Logical)] enum Left {{ Next({field}) }} #[derive(Logical)] enum Right {{ Next(Left) }} logic fn independent() -> Int {{ 9 }}"
        );
        let rejected = check(&source);
        assert!(!rejected.is_success(), "accepted {source}");
        assert!(
            rejected.diagnostics.iter().any(|d| d.code == "L0203"),
            "{:?}",
            rejected.diagnostics
        );
        assert!(rejected.function("independent").is_some());
        assert!(rejected.session.erased().enums.is_empty());
    }
    let rejected = check("#[derive(Logical)] enum Left { Next(Right) } enum Right { Next(Left) }");
    assert!(!rejected.is_success());
}

#[test]
fn mutual_group_waits_for_external_dependencies() {
    let checked = check(
        r#"
#[derive(Logical)] enum Left { End(Label), Next(Right) }
#[derive(Logical)] enum Right { Next(Left) }
#[derive(Logical)] struct Label { value: Int }
logic fn make() -> Left { Left::Next(Right::Next(Left::End(Label { value: 3 }))) }
"#,
    );
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
}

#[test]
fn matching_another_group_member_does_not_make_the_root_smaller() {
    let rejected = check(
        r#"
#[derive(Logical)] enum Tree { Leaf, Branch(Forest) }
#[derive(Logical)] enum Forest { Empty, Cons(Tree, Forest) }
logic fn bad(root: Tree) -> Int {
    match root {
        Tree::Leaf => 0,
        Tree::Branch(forest) => match forest {
            Forest::Empty => 0,
            Forest::Cons(child, rest) => bad(root),
        },
    }
}
"#,
    );
    assert!(!rejected.is_success());
    assert!(
        rejected.diagnostics.iter().any(|d| d.code == "L0203"),
        "{:?}",
        rejected.diagnostics
    );
}
