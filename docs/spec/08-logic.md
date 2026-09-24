+++
id = "language-logic"
title = "Logical computation"
group = "Now"
spec_chapter = 1
order = 107
route = "specification/logic.html"
description = "Logic functions and blocks, total recursion, and the boundary with runtime execution."
+++

# Logical computation

<!-- spec: 1.0:9 informative -->
Logical code calculates the values used in claims and proofs. It is checked for purity and termination, then erased. Ordinary code may use its results for verification, but cannot turn them into runtime decisions or data.

## Expressions and computation modes

<!-- spec: 1.5:2 dynamic-semantics -->
`fn` declares runtime execution. It may accept or return physical data, logical data, propositions, and evidence. Logical results do not erase the call. An ordinary function cannot be called or unfolded inside logical computation, even when it promises to terminate, avoid panic, allocation, and I/O.

<!-- spec: 1.5:3 dynamic-semantics -->
`logic fn` must return a Logical type. Its body may call logical definitions and observe authorized runtime inputs. It cannot call ordinary functions, mutate physical storage, allocate physical objects, or panic. `logic { ... }`, proposition literals, proof annotations, and `prove!` establish logical contexts with these same restrictions.

<!-- spec: 1.90:40 example -->
~~~rust check
logic fn successor(n: Int) -> Int { n + 1 }
fn describe(n: u8) -> Prop {
    let expected = successor(n as Int);
    prop!(expected > n)
}
fn proof(n: u8) -> @(successor(n) > n) {
    fold!(successor, prove!(n + 1 > n))
}
~~~

<!-- spec: 1.5:1 dynamic-semantics -->
Ordinary operands and arguments evaluate left to right. Assignment evaluates its right side before its destination. These evaluation steps remain even when the enclosing expression ultimately produces a Logical value.

<!-- spec: 1.5:4 dynamic-semantics -->
Logical calls and operators may appear directly in ordinary code. Ordinary argument-producing calls execute once in source order, then the logical operation erases. An explicit logical context rejects those ordinary calls. Runtime control needs a physical `bool` or enum tag; logical control uses `Bool` or logical data and cannot select runtime effects.

<!-- spec: 1.90:41 example -->
~~~rust run
fn take_next(counter: &mut u8) -> u8 {
    counter = counter.wrapping_add(1);
    counter
}
logic fn describe(n: Int) -> Int { n + 1 }
fn demo() -> u8 {
    let mut counter: u8 = 0;
    let ignored = describe(take_next(&mut counter));
    counter // take_next still ran
}
//~ run: demo() => 1
~~~

<!-- spec: 1.5:5 dynamic-semantics -->
Operand types select operators: addition on `Nat` or `Int` is logical; addition on `u8` is runtime arithmetic. Logical `Bool` and runtime `bool` stay distinct through fields, parameters, results, and control-flow joins. A shared internal kernel representation does not make the two source types interchangeable.

## Logical data

<!-- spec: 1.25:1 legality-rule -->
Logical structs and finite recursive enums erase completely. A logical recursive enum uses direct recursion, without Box. Mutually referring logical enums form one checked group, specialized first when generic. Logical function recursion may follow same-typed descendants exposed across that group. Mutually recursive functions are unsupported. Library types and lemmas are ordinary checked source included explicitly with `--library`.

<!-- spec: 1.90:42 example -->
~~~rust check
#[derive(Logical)]
enum Peano { Zero, Succ(Peano) }
logic fn size(n: Peano) -> Int {
    match n {
        Peano::Zero => 0,
        Peano::Succ(previous) => 1 + size(previous),
    }
}
logic fn nonnegative(n: Peano) -> @(size(n) >= 0) {
    match n {
        Peano::Zero => fold!(size, prove!(0 >= 0)),
        Peano::Succ(previous) => {
            let induction = nonnegative(previous);
            fold!(size, prove!(1 + size(previous) >= 0))
        }
    }
}
~~~

<!-- spec: 1.91:20 informative -->
The recursive theorem call is the induction hypothesis for `previous`. It is legitimate because matching `Succ` exposed a smaller part of `n`. `fold!` connects the branch calculation to the named function’s result. The compiler checks this once for an arbitrary finite value; it does not enumerate all naturals.

<!-- spec: 1.25:2 legality-rule -->
Structural recursion must use constructor subdata. Integer recursion uses `recurse!(decreases, self_call)`, with evidence of `0 <= next && next < current`. That evidence is checked before the recursive call is available. Recursive theorem calls provide checked induction. Recursive propositions additionally require [positive constructor conditions](09-propositions.md#recursive-propositions).

<!-- spec: 1.90:43 example -->
~~~rust check
logic fn steps(n: Int) -> Int {
    if n <= 0 { 0 } else {
        let next = n - 1;
        let smaller: @(0 <= next && next < n) =
            And::Intro(prove!(0 <= next), prove!(next < n));
        1 + recurse!(smaller, steps(next))
    }
}
~~~

## Logical closures

<!-- spec: 1.25:3 legality-rule -->
Logical closures use typed parameters, inferred captures, and logical results, including dependent proof results. Runtime captures need a canonical model, inferred for simple logical uses such as `|x: Int| x + n`. Use `model!(s.n)` to capture a physical field of an unmodeled enclosing value. Captures retain the observed versions. Closure inputs and outputs must be Logical; named logical functions may additionally observe physical inputs. Logical callable evaluation in ordinary code preserves eager runtime argument effects.

<!-- spec: 1.90:44 example -->
~~~rust check
logic fn apply(f: logic Fn(x: Int) -> Int, x: Int) -> Int { f(x) }
fn capture(n: u8) -> Int {
    let add_input = |x: Int| x + (n as Int);
    apply(add_input, 10)
}
~~~
