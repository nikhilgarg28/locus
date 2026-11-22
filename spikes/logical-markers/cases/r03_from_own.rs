// expect: E0119
// One marker: the library cannot write its own conversion either. It overlaps the reflexive one.
#[derive(Clone, Copy)] struct LogicalData;
type Int = LogicalData;
type Nat = LogicalData;
impl From<Nat> for Int { fn from(n: Nat) -> Int { n } }
fn main() {}
