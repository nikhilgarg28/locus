# Observing runtime values

Logical code never runs, but it can read runtime values: it *observes* them. Inside a function body, logical code is a `logic!`, `prop!`, or `prove!` invocation, a proposition in a type annotation, or an argument for a parameter of logical type. `final!(r)` reads no place, so it observes nothing ([Contracts](contracts.md)). An observation is checked with Rust's own ownership rules, so logical code can read only what the runtime program could read at the same point.

Propositions in signatures and in struct field types read parameters, results, and the struct's own fields. These are readable by construction, so they observe nothing at a point in the body.

r[observe.place.readable]
Logical code inside a function body may read a place only where a shared borrow of that place would be valid at the same point of the program. The read ends immediately: it does not keep the place borrowed. Reading a value after it has moved, or a place while it is mutably borrowed, is rejected.

r[observe.let.after-initializer]
A proposition in the type annotation of a `let` reads places after the `let`'s initializer has been evaluated, where the `let`'s bindings come into scope.

r[observe.captures.rejected]
Logical code inside a closure body or an async block is rejected.
