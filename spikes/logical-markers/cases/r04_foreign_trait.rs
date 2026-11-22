// expect: E0117
// One marker: a program cannot give the standard library's trait to its logical type,
// because that type is the library's marker.
extern crate lu;
use lu::*;
type Version = LogicalData;          // #[derive(Logical)] struct Version { .. }
impl std::ops::Add for Version { type Output = Version; fn add(self, _: Version) -> Version { self } }
fn main() {}
