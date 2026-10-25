# Observing runtime values

Proofs and logical code never run, but they can read runtime values: they *observe* them. An observation is checked with Rust's own ownership rules, so logical code can read only what the runtime program could read at the same point.

r[observe.place.readable]
A proof or logical block may observe a place only where a shared borrow of that place would be valid at the same point of the program. The observation ends immediately: it does not keep the place borrowed. Observing a value after it has moved, or a place while it is mutably borrowed, is rejected.
