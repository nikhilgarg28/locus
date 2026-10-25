# Observing runtime values

Logical code never runs, but it can read runtime values: it *observes* them. Inside a function body, logical code is a `logic!`, `prop!`, or `prove!` invocation, or a proposition in a type annotation. An observation is checked with Rust's own ownership rules, so logical code can read only what the runtime program could read at the same point.

Propositions in signatures and in struct field types read parameters, results, and the struct's own fields. These are readable by construction, so they observe nothing at a point in the body.

r[observe.place.readable]
Logical code inside a function body may read a place only where a shared borrow of that place would be valid at the same point of the program. The read ends immediately: it does not keep the place borrowed. Reading a value after it has moved, or a place while it is mutably borrowed, is rejected.

r[observe.closures.rejected]
Logical code inside a closure body is rejected.
