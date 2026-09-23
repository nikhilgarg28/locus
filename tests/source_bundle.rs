use locus::{
    diagnostic::{Applicability, Diagnostic, Suggestion},
    source::{SourceBundle, SourceMap, Span},
};
#[test]
fn bundle_locations_and_suggestions_refer_to_original_files() {
    let mut sources = SourceMap::default();
    let library = sources.add("library.lc", "é\n");
    let main = sources.add("main.lc", "x\ny\n");
    let bundle = SourceBundle::join(&mut sources, &[library, main]);
    assert_eq!(sources.get(bundle.file).text(), "é\n\nx\ny\n\n");
    assert_eq!(
        bundle.span(Span::new(bundle.file, 0, 2)),
        Span::new(library, 0, 2)
    );
    assert_eq!(
        bundle.span(Span::new(bundle.file, 4, 5)),
        Span::new(main, 0, 1)
    );
    let diagnostic = Diagnostic::error("TEST", "message", Span::new(bundle.file, 6, 7))
        .label(Span::new(bundle.file, 0, 2), "library definition")
        .suggest(Suggestion {
            message: "replace".into(),
            span: Span::new(bundle.file, 6, 7),
            replacement: "z".into(),
            applicability: Applicability::MachineApplicable,
        });
    let diagnostic = bundle.diagnostic(&diagnostic);
    assert_eq!(diagnostic.labels[0].span, Span::new(main, 2, 3));
    assert_eq!(diagnostic.labels[1].span.file, library);
    assert_eq!(diagnostic.suggestions[0].span, Span::new(main, 2, 3));
    let rendered = diagnostic.render(&sources, false);
    assert!(rendered.contains("main.lc:2:"), "{rendered}");
    assert!(rendered.contains("library.lc:1:"), "{rendered}");
}
#[test]
fn single_source_and_cross_segment_recovery_keep_valid_ranges() {
    let mut sources = SourceMap::default();
    let file = sources.add("one.lc", "abc");
    let single = SourceBundle::join(&mut sources, &[file]);
    assert_eq!(single.file, file);
    let second = sources.add("two.lc", "z");
    let bundle = SourceBundle::join(&mut sources, &[file, second]);
    assert_eq!(
        bundle.span(Span::new(bundle.file, 1, 5)),
        Span::new(file, 1, 3)
    );
    assert_eq!(
        bundle.span(Span::new(bundle.file, 6, 6)),
        Span::new(second, 1, 1)
    );
}
