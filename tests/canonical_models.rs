//! Canonical observation boundaries and proof-checked natural arithmetic.
use locus::{elab, parser, source::SourceMap};
fn check(text: &str) -> elab::Elaborated {
    let mut sources = SourceMap::default();
    let file = sources.add("canonical.lc", text);
    let source = sources.get(file);
    let parsed = parser::parse(source);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    elab::elaborate(source, &parsed.program)
}
fn accept(text: &str) -> elab::Elaborated {
    let result = check(text);
    assert!(result.is_success(), "{:#?}", result.diagnostics);
    result
}
const MODEL: &str = "struct Point { x: u8 }
#[derive(Logical)] struct Position { horizontal: Nat }
impl Model for Point {
    type Logic = Position;
    logic fn model(&self) -> Self::Logic { Position { horizontal: model!(self.x) } }
}";
#[test]
#[doc = "spec: 1.92:1, 3.8:2"]
fn canonical_receiver_and_explicit_physical_projection_are_distinct() {
    accept(&format!(
        "{MODEL}
        fn implicit(point: Point) -> @(point.horizontal >= 0) {{ _ }}
        fn explicit(point: Point) -> @(model!(point.x) >= 0) {{ _ }}"
    ));
    let result = check(&format!(
        "{MODEL} fn wrong(point: Point) -> @(point.x >= 0) {{ _ }}"
    ));
    assert!(result.diagnostics.iter().any(|d| d.code == "L0210"));
}
#[test]
#[doc = "spec: 1.92:5"]
fn structural_derivation_is_opt_in_and_requires_field_models() {
    accept(
        "#[derive(Model)] struct Point { x: u8, y: i32 }
        fn read(point: Point) -> @(point.x >= 0) { _ }
        fn model(point: Point) -> PointModel { model!(point) }",
    );
    accept(
        "#[derive(Model)] struct Packet { byte: u8, flag: Bool, size: Nat }
        fn read(packet: Packet) -> PacketModel { model!(packet) }",
    );
    for text in [
        "struct Point { x: u8 } fn read(point: Point) -> @(point.x >= 0) { _ }",
        "struct Opaque {} #[derive(Model)] struct Point { x: u8, opaque: Opaque }",
    ] {
        assert!(check(text).diagnostics.iter().any(|d| d.code == "L0282"));
    }
    accept(
        "struct Opaque {} struct Point { x: u8, opaque: Opaque }
        fn read(point: Point) -> @(model!(point.x) >= 0) { _ }",
    );
}
#[test]
#[doc = "spec: 1.92:4"]
fn physical_observations_preserve_snapshot_identity_and_do_not_move() {
    accept(
        "struct Point { x: u8 }
        fn read() -> u8 {
            let mut point = Point { x: 3 };
            let before = model!(point.x);
            point.x = 4;
            let historical = prove!(before == 3);
            let current = prove!(model!(point.x) == 4);
            point.x
        }",
    );
    let wrong = check(
        "struct Point { x: u8 }
        fn wrong() -> @(true) {
            let mut point = Point { x: 3 };
            let before = model!(point.x);
            point.x = 4;
            let stale = prove!(model!(point.x) == before);
            True::Intro
        }",
    );
    assert!(wrong.diagnostics.iter().any(|d| d.code == "L0230"));
}
#[test]
#[doc = "spec: 1.92:2"]
fn observations_reject_hidden_runtime_computation() {
    for expr in ["compute()", "n + 1", "{ n }"] {
        let result = check(&format!(
            "fn compute() -> u8 {{ 1 }} fn f(n: u8) -> Nat {{ model!({expr}) }}"
        ));
        assert!(result.diagnostics.iter().any(|d| d.code == "L0282"));
    }
}
#[test]
#[doc = "spec: 3.8:1"]
fn canonical_model_cannot_be_overridden_by_another_destination() {
    let result = check(&format!(
        "{MODEL}
        impl Model for Point {{ type Logic = Int; logic fn model(&self) -> Self::Logic {{ 0 }} }}"
    ));
    assert!(result.diagnostics.iter().any(|d| d.code == "L0282"));
    let result =
        check("impl Model for u8 { type Logic = Int; logic fn model(&self) -> Self::Logic { 0 } }");
    assert!(result.diagnostics.iter().any(|d| d.code == "L0282"));
}
#[test]
#[doc = "spec: 1.92:7, 3.8:3"]
fn naturals_are_checked_values_and_subtraction_needs_evidence() {
    accept("logic fn nonnegative(n: Nat) -> @(n >= 0) { _ }
        logic fn add(a: Nat, b: Nat) -> Nat { a + b }
        logic fn multiply(a: Nat, b: Nat) -> Nat { a * b }
        logic fn quotient(a: Nat, b: Nat) -> Nat { a / b }
        logic fn remainder(a: Nat, b: Nat) -> Nat { a % b }
        logic fn zero_divisor(n: Nat) -> @(n / 0 == 0 && n % 0 == n) { And::Intro(prove!(n / 0 == 0), prove!(n % 0 == n)) }
        logic fn difference(a: Nat, b: Nat, ordered: @(b <= a)) -> Nat { a - b }
        logic fn widen(n: Nat) -> Int { n as Int }
        fn unsigned(n: u32) -> Nat { model!(n) }
        fn signed(n: i32) -> Int { model!(n) }");
    for text in [
        "logic fn bad(a: Nat, b: Nat) -> Nat { a - b }",
        "logic fn bad(n: Int) -> Nat { n as Nat }",
        "logic fn bad() -> Nat { -1 }",
        "logic fn bad() -> Nat { Nat { value: -1, nonnegative: _ } }",
    ] {
        let result = check(text);
        assert!(!result.is_success(), "{text}");
        assert!(
            !result.diagnostics.iter().any(|d| d.code == "L0300"),
            "{:#?}",
            result.diagnostics
        );
    }
}

#[test]
fn invalid_model_derivations_and_logical_sources_are_rejected() {
    for text in [
        "#[derive(Model)] enum Wrong { A }",
        "#[derive(Model)] struct Point { x: u8 } struct PointModel {}",
        "#[derive(Logical)] struct Logical {} impl Model for Logical { type Logic = Int; logic fn model(&self) -> Int { 0 } }",
    ] {
        let result = check(text);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0282"),
            "{:#?}",
            result.diagnostics
        );
    }
}
#[test]
fn observation_can_select_a_checked_model_equation() {
    accept(&format!(
        "{MODEL} logic fn equation(point: &Point, h: @(model!(point.x) == 3)) -> @(point.horizontal == 3) {{ fold!(model!(point), h) }}"
    ));
}

#[test]
#[doc = "spec: 1.92:2, 1.92:4"]
fn observations_follow_complete_paths_and_check_index_bounds() {
    accept("struct Inner { number: u8 } struct Outer { inner: Inner }
        fn nested(outer: Outer) -> Nat { model!(outer.inner.number) }
        fn tuple(pair: (u8, bool)) -> Nat { model!(pair.0) }
        fn boxed(value: Box<u8>) -> Nat { model!(*value) }
        fn indexed(items: &[u8], index: usize, valid: @(index < items.len())) -> Nat { model!(items[index]) }
        fn entry(point: &mut Outer) -> @(model!(point.inner.number) == model!(old!(point).inner.number)) { _ }");
    let result =
        check("fn out_of_bounds(items: &[u8], index: usize) -> Nat { model!(items[index]) }");
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0230"),
        "{:#?}",
        result.diagnostics
    );
    let result = check(
        "fn wrong() -> Nat { let mut values: Vec<u8> = Vec::from([3]); let borrowed = &values; values.push(4); model!(borrowed[0]) }",
    );
    assert!(!result.is_success());
    assert!(
        !result.diagnostics.iter().any(|d| d.code == "L0300"),
        "{:#?}",
        result.diagnostics
    );
}

#[test]
fn implicit_models_resolve_before_users_independent_of_source_order() {
    accept(&format!(
        "logic fn read(point: &Point) -> @(point.horizontal >= 0) {{ _ }} {MODEL}"
    ));
    accept(&format!(
        "fn make() -> Point {{ Point {{ x: 3 }} }}
        fn observe_result() -> Position {{ let point = make(); model!(point) }} {MODEL}"
    ));
    accept(
        "struct Point { x: u8 } struct Outer { point: Point }
        #[derive(Logical)] struct Position { x: Nat }
        impl Model for Outer { type Logic = Position;
            logic fn model(&self) -> Self::Logic { model!(self.point) } }
        impl Model for Point { type Logic = Position;
            logic fn model(&self) -> Self::Logic { Position { x: model!(self.x) } } }
        fn read(outer: Outer) -> Position { model!(outer) }",
    );
    accept(
        "struct Point { x: u8 } struct Holder { value: Box<Point> }
        fn read(holder: Holder) -> Nat { model!(*holder.value) }
        impl Model for Point { type Logic = Nat;
            logic fn model(&self) -> Self::Logic { model!(self.x) } }",
    );
}

#[test]
#[doc = "spec: 1.27:3, 1.92:8"]
fn only_pre_operation_evidence_permits_plain_rust_arithmetic() {
    let result = accept(
        "fn bounded(n: u8, h: @(n < 255)) -> u8 { n + 1 }
        fn guarded(n: u8) -> u8 { if n < 255 { n + 1 } else { 0 } }
        fn unbounded(n: u8) -> (out: u8, @(out == n + 1)) { let out = n + 1; (out, _) }",
    );
    let rust = locus::erased::print_module(result.session.erased());
    let bounded = rust
        .split("fn bounded")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(bounded.contains("n + 1_u8"), "{rust}");
    let guarded = rust
        .split("fn guarded")
        .nth(1)
        .unwrap()
        .split("fn unbounded")
        .next()
        .unwrap();
    assert!(!guarded.contains("checked_add"), "{rust}");
    let unbounded = rust.split("fn unbounded").nth(1).unwrap();
    assert!(unbounded.contains("checked_add"), "{rust}");
}

#[test]
#[doc = "spec: 1.92:4"]
fn applying_a_derived_model_cannot_hide_a_moved_source() {
    for source in [
        "#[derive(Model)] struct Token { id: u8 } fn consume(t: Token) -> u8 { t.id } fn bad(t: Token) -> Nat { let used = consume(t); model!(t.id) }",
        "#[derive(Model)] struct Token { id: u8 } fn consume(t: Token) -> u8 { t.id } fn bad(t: Token) -> @(t.id == 3) { let used = consume(t); prove!(t.id == 3) }",
    ] {
        let result = check(source);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| matches!(d.code, "L0240" | "L0241")),
            "{:#?}",
            result.diagnostics
        );
    }
}

#[test]
#[doc = "spec: 1.15:1, 1.92:13"]
fn machine_constants_keep_their_values_when_observed_in_logic() {
    // Explicit expected types check the model, not only numerical equality.
    for (machine, logical, min, max) in [
        ("u8", "Nat", "0", "255"),
        ("u16", "Nat", "0", "65535"),
        ("u32", "Nat", "0", "4294967295"),
        ("u64", "Nat", "0", "18446744073709551615"),
        ("i8", "Int", "-128", "127"),
        ("i16", "Int", "-32768", "32767"),
        ("i32", "Int", "-2147483648", "2147483647"),
        ("i64", "Int", "-9223372036854775808", "9223372036854775807"),
    ] {
        accept(&format!(
            "logic fn lower() -> {logical} {{ {machine}::MIN }}
             logic fn upper() -> {logical} {{ model!({machine}::MAX) }}
             logic fn min_value() -> @({machine}::MIN == {min}) {{ _ }}
             logic fn max_value() -> @({machine}::MAX == {max}) {{ _ }}
             logic fn beyond() -> @({machine}::MAX + 1 > {machine}::MAX) {{ _ }}"
        ));
    }
    let wrong = check("logic fn wrong() -> @(u32::MAX + 1 == 0) { _ }");
    assert!(wrong.diagnostics.iter().any(|d| d.code == "L0230"));
}

#[test]
#[doc = "spec: 1.9:5, 1.92:13"]
fn constants_are_observed_after_their_physical_initializer_is_checked() {
    accept(
        "const LIMIT: u8 = 255u8.wrapping_add(1);
        const NEXT: u8 = LIMIT.wrapping_add(1);
        logic fn wrapped() -> @(LIMIT == 0) { _ }
        logic fn next() -> @(NEXT == 1) { _ }
        fn runtime() -> u8 { NEXT }",
    );
    accept(&format!(
        "const ORIGIN: Point = Point {{ x: 7 }}; {MODEL}
        logic fn physical_field() -> @(model!(ORIGIN.x) == 7) {{ _ }}
        logic fn logical_field() -> @(ORIGIN.horizontal == 7) {{
            fold!(model!(ORIGIN), prove!(model!(ORIGIN.x) == 7))
        }}
        fn runtime() -> u8 {{ ORIGIN.x }}"
    ));
    let wrong = check(
        "const LIMIT: u8 = 255u8.wrapping_add(1);
        logic fn wrong() -> @(LIMIT == 256) { _ }",
    );
    assert!(wrong.diagnostics.iter().any(|d| d.code == "L0230"));
}

#[test]
#[doc = "spec: 1.9:4, 1.92:13"]
fn an_effect_free_associated_runtime_function_is_not_a_logical_constant() {
    let result = check(
        "struct Limits {}
        impl Limits {
            #[no_panic] #[no_io] #[no_alloc] #[terminates]
            fn upper() -> u32 { u32::MAX }
        }
        logic fn bad() -> @(Limits::upper() == u32::MAX) { _ }",
    );
    assert!(result.diagnostics.iter().any(|d| d.code == "L0209"));
    accept(
        "struct Limits {}
        impl Limits { logic fn upper() -> Nat { u32::MAX } }
        logic fn bound() -> @(Limits::upper() == u32::MAX) {
            fold!(Limits::upper, prove!(u32::MAX == u32::MAX))
        }",
    );
}

#[test]
#[doc = "spec: 1.92:13"]
fn const_functions_remain_outside_the_current_syntax() {
    for text in [
        "const fn upper() -> u32 { u32::MAX }",
        "struct Limits {} impl Limits { const fn upper() -> u32 { u32::MAX } }",
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("unsupported.lc", text);
        let parsed = parser::parse(sources.get(file));
        assert!(!parsed.is_success(), "{text}");
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| matches!(d.code, "L0100" | "L0115")),
            "{:#?}",
            parsed.diagnostics
        );
    }
}

#[test]
#[doc = "spec: 1.9:5, 1.14:1, 1.92:13"]
fn associated_constants_resolve_self_aliases_models_and_forward_definitions() {
    let result = accept(
        "const OUTSIDE: u32 = Limits::LIMIT;
        fn read() -> u32 { Limits::LIMIT }
        fn modeled() -> Nat { model!(Limits::LIMIT) }
        fn generic<T>() -> u32 { T::LIMIT }
        fn instantiated() -> u32 { generic::<Limits>() }
        logic fn property() -> @Limits::CLAIM { fold!(Limits::CLAIM, prove!(Limits::LIMIT == 255)) }
        struct Limits {}
        impl Limits {
            pub const LIMIT: u32 = Self::BYTE as u32;
            const BYTE: u8 = u8::MAX;
            const CLAIM: Prop = prop!(Self::LIMIT == 255);
            const KNOWN: @(Self::LIMIT == 255) = _;
            fn evidence() -> @(Self::LIMIT == 255) { Self::KNOWN }
        }",
    );
    let rust = locus::erased::print_module_with(
        result.session.erased(),
        &result.visibilities,
        locus::erased::Markers::Here,
    );
    assert!(rust.contains("pub const LIMIT: u32"), "{rust}");
    assert!(!rust.contains("const Limits::"), "{rust}");
    assert!(!rust.contains("const CLAIM"), "{rust}");
    accept(&format!("fn observe() -> Position {{ model!(Defaults::ORIGIN) }}
        struct Defaults {{}}
        impl Defaults {{ const ORIGIN: Point = Point {{ x: 7 }}; }}
        {MODEL}
        logic fn logical_field() -> @(Defaults::ORIGIN.horizontal == 7) {{ fold!(model!(Defaults::ORIGIN), prove!(model!(Defaults::ORIGIN.x) == 7)) }}"));
}

#[test]
#[doc = "spec: 1.92:13"]
fn associated_constants_reject_cycles_collisions_calls_and_wrong_proofs() {
    for (text, code) in [
        (
            "struct S {} impl S { const A: u8 = Self::B; const B: u8 = Self::A; }",
            "L0203",
        ),
        (
            "struct S {} impl S { const A: u8 = 1; const A: u8 = 2; }",
            "L0202",
        ),
        (
            "struct S {} impl S { const A: u8 = 1; fn A() -> u8 { 2 } }",
            "L0202",
        ),
        ("enum S { A } impl S { const A: u8 = 1; }", "L0202"),
        (
            "struct S {} impl S { const A: u8 = 1; } fn bad() -> u8 { S::A() }",
            "L0208",
        ),
        ("const A: u8 = 1; fn bad() -> u8 { A() }", "L0208"),
        (
            "struct S {} impl S { const A: u8 = 1; } logic fn bad() -> @(S::A == 2) { _ }",
            "L0230",
        ),
    ] {
        let result = check(text);
        assert!(
            result.diagnostics.iter().any(|d| d.code == code),
            "{text}\n{:#?}",
            result.diagnostics
        );
    }
}

#[test]
#[doc = "spec: 1.14:1, 1.92:15"]
fn associated_constants_use_self_in_types_and_initializers() {
    accept(
        "struct Point { x: u8 }
        impl Point { const ORIGIN: Self = Self { x: 0 }; }
        enum Slot { Empty, Full(u8) }
        impl Slot { const DEFAULT: Self = Self::Full(7); }
        fn point() -> u8 { Point::ORIGIN.x }
        fn slot() -> u8 { match Slot::DEFAULT { Slot::Empty => 0, Slot::Full(n) => n } }
        logic fn correct() -> @(model!(Point::ORIGIN.x) == 0) { _ }",
    );
}

#[test]
#[doc = "spec: 1.9:5, 1.92:13, 1.92:15"]
fn constant_arithmetic_checks_physical_ranges_before_logical_observation() {
    accept(
        "const BASE: u32 = (2 + 3) * 8;
        struct Limits {}
        impl Limits {
            const LIMIT: u32 = BASE + 2;
            const HALF: u32 = Self::LIMIT / 2;
            const LEFT: u32 = Self::HALF % 5;
            const NEGATIVE: i8 = -Self::POSITIVE;
            const POSITIVE: i8 = 7;
            const QUOTIENT: i8 = Self::NEGATIVE / 2;
            const FLIPPED: i8 = Self::NEGATIVE / -1;
        }
        logic fn quotient() -> @(Limits::QUOTIENT == -3) { _ }
        logic fn flipped() -> @(Limits::FLIPPED == 7) { _ }
        logic fn answer() -> @(Limits::LIMIT == 42) { _ }
        logic fn half() -> @(Limits::HALF == 21) { _ }
        logic fn left() -> @(Limits::LEFT == 1) { _ }
        logic fn negative() -> @(Limits::NEGATIVE == -7) { _ }",
    );
    for initializer in ["u8::MAX + 1", "0u8 - 1", "128u8 * 2", "3u8 / 0", "3u8 % 0"] {
        let result = check(&format!(
            "struct Limits {{}} impl Limits {{ const LIMIT: u8 = {initializer}; }}"
        ));
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0235"),
            "{initializer}: {:#?}",
            result.diagnostics
        );
    }
    for initializer in ["-i8::MIN", "i8::MIN / -1", "i8::MIN % -1"] {
        let result = check(&format!("const LIMIT: i8 = {initializer};"));
        assert!(
            result.diagnostics.iter().any(|d| d.code == "L0235"),
            "{initializer}: {:#?}",
            result.diagnostics
        );
    }
}
