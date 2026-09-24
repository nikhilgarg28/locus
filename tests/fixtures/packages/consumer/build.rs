fn main() {
    locus::project::Build::new("locus/export.lc")
        .name("local")
        .offline(true)
        .generate()
        .expect("check and generate consumer")
        .cargo_rerun_directives();
    locus::project::Build::new("locus/auxiliary")
        .name("auxiliary")
        .offline(true)
        .generate()
        .expect("check independent component with two public modules")
        .cargo_rerun_directives();
}
