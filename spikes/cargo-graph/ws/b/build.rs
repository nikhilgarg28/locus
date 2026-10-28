//! A build script that calls a Locus library (a build-dependency), so it is
//! checked too.
fn main() {
    println!("cargo::rustc-check-cfg=cfg(b_fast)");
    if a::double(21) == 42 {
        println!("cargo::rustc-cfg=b_fast");
    }
}
