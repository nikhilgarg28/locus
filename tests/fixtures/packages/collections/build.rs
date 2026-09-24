fn main() {
    locus::project::Build::new("locus/export.lc")
        .name("checked")
        .offline(true)
        .generate()
        .expect("check and generate verified collections")
        .cargo_rerun_directives();
}
