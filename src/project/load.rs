use super::{Error, Graph};
use crate::{
    ast,
    diagnostic::Diagnostic,
    lexer::{self, TokenKind as K},
    parser,
    source::{FileId, SourceBundle, SourceMap, Span},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub struct Loaded {
    pub sources: SourceMap,
    pub bundle: SourceBundle,
    pub program: ast::Program,
    pub graph: Graph,
    pub inputs: BTreeMap<PathBuf, String>,
    pub entry: PathBuf,
    pub cargo: Option<super::cargo::Workspace>,
    pub packages: Vec<usize>,
    pub native: crate::imports::Imports,
}

impl Loaded {
    pub fn diagnostic(&self, diagnostic: &Diagnostic) -> Diagnostic {
        let mut d = self.bundle.diagnostic(diagnostic);
        // Canonical compiler names are never useful in a source diagnostic.
        let mut names: Vec<_> = self
            .graph
            .items
            .iter()
            .filter(|i| !i.original.is_empty())
            .collect();
        names.sort_by_key(|i| std::cmp::Reverse(i.canonical.len()));
        for item in names {
            let replace = |text: &mut String| {
                *text = text.replace(&item.canonical, &item.original);
            };
            replace(&mut d.message);
            for label in &mut d.labels {
                replace(&mut label.message);
            }
            for note in &mut d.notes {
                replace(note);
            }
            for help in &mut d.details.helps {
                replace(help);
            }
            for suggestion in &mut d.details.suggestions {
                replace(&mut suggestion.message);
                replace(&mut suggestion.replacement);
            }
            let p = &mut d.details.proof;
            for text in [
                &mut p.claim,
                &mut p.claim_after_computing,
                &mut p.counterexample,
                &mut p.suggested_explicit_form,
            ]
            .into_iter()
            .flatten()
            {
                replace(text);
            }
            if let Some(facts) = &mut p.facts_considered {
                for fact in facts {
                    replace(&mut fact.claim);
                    if let Some(name) = &mut fact.name {
                        replace(name);
                    }
                }
            }
        }
        d
    }
}

pub fn entry_file(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("export.lc")
    } else {
        path.into()
    }
}

pub fn load(path: &Path) -> Result<Loaded, Error> {
    let entry = entry_file(path);
    let mut loader = Loader {
        sources: SourceMap::default(),
        inputs: BTreeMap::new(),
        stack: Vec::new(),
        fragments: Vec::new(),
        bytes: 0,
    };
    let driver = loader.sources.add(entry.display().to_string(), "");
    let origin = Span::new(driver, 0, 0);
    let dir = entry.parent().unwrap_or(Path::new("."));
    if let Err(d) = loader.file(&entry, dir, origin, 0) {
        return Err(Error {
            sources: loader.sources,
            diagnostics: vec![d],
        });
    }
    let bundle = SourceBundle::fragments(
        &mut loader.sources,
        &entry.display().to_string(),
        &loader.fragments,
    );
    let parsed = parser::parse(loader.sources.get(bundle.file));
    if !parsed.is_success() {
        return Err(Error {
            sources: loader.sources,
            diagnostics: parsed
                .diagnostics
                .iter()
                .map(|d| bundle.diagnostic(d))
                .collect(),
        });
    }
    let (program, graph, diagnostics) =
        super::resolve::resolve(parsed.program, loader.sources.get(bundle.file));
    if !diagnostics.is_empty() {
        return Err(Error {
            sources: loader.sources,
            diagnostics: diagnostics.iter().map(|d| bundle.diagnostic(d)).collect(),
        });
    }
    Ok(Loaded {
        sources: loader.sources,
        bundle,
        program,
        graph,
        inputs: loader.inputs,
        entry: entry.canonicalize().unwrap_or(entry),
        cargo: None,
        packages: Vec::new(),
        native: crate::imports::Imports::default(),
    })
}

/// Assemble one package graph into one checked unit. Each Cargo package has a
/// distinct crate root; export entries are children of their library root.
pub(super) fn load_cargo(
    path: &Path,
    workspace: super::cargo::Workspace,
    options: &super::cargo::CargoOptions,
) -> Result<Loaded, Error> {
    let entry = entry_file(path);
    let mut packages = vec![workspace.host];
    let mut next = 0;
    while next < packages.len() {
        let id = packages[next];
        for &dep in workspace.packages[id].dependencies.values() {
            if workspace.packages[dep].library.is_some() && !packages.contains(&dep) {
                packages.push(dep);
            }
        }
        next += 1;
    }
    let mut loader = Loader {
        sources: SourceMap::default(),
        inputs: workspace.inputs.clone(),
        stack: Vec::new(),
        fragments: Vec::new(),
        bytes: 0,
    };
    let driver = loader.sources.add(entry.display().to_string(), "");
    let origin = Span::new(driver, 0, 0);
    let mut shapes = Vec::new();
    let mut selection = None;
    let mut parts = 0;
    for (index, &package) in packages.iter().enumerate() {
        let p = &workspace.packages[package];
        let root = p.library.as_ref().unwrap_or(&entry);
        let entries: Vec<_> = if index == 0 {
            if !same_path(root, &entry) {
                selection = Some(0);
                vec![entry.clone()]
            } else {
                Vec::new()
            }
        } else {
            p.targets.iter().map(|t| t.entry.clone()).collect()
        };
        let mut shape = Vec::new();
        for file in std::iter::once(Some(root)).chain(
            entries
                .iter()
                .map(|e| if same_path(e, root) { None } else { Some(e) }),
        ) {
            if let Some(file) = file {
                loader
                    .fragments
                    .push((origin, format!("mod __locus_unit{parts} {{\n")));
                parts += 1;
                if let Err(d) =
                    loader.file(file, file.parent().unwrap_or(Path::new(".")), origin, 0)
                {
                    return Err(Error {
                        sources: loader.sources,
                        diagnostics: vec![d],
                    });
                }
                loader.fragments.push((origin, "\n}\n".into()));
                shape.push(true);
            } else {
                shape.push(false);
            }
        }
        shapes.push(shape);
    }
    let bundle = SourceBundle::fragments(
        &mut loader.sources,
        &entry.display().to_string(),
        &loader.fragments,
    );
    let parsed = parser::parse(loader.sources.get(bundle.file));
    if !parsed.is_success() {
        return Err(Error {
            sources: loader.sources,
            diagnostics: parsed
                .diagnostics
                .iter()
                .map(|d| bundle.diagnostic(d))
                .collect(),
        });
    }
    let mut bodies = parsed
        .program
        .declarations
        .into_iter()
        .map(|d| match d.kind {
            ast::DeclarationKind::Module {
                body: Some(body), ..
            } => body,
            _ => unreachable!("synthetic module"),
        });
    let mut units = Vec::new();
    let mut native = crate::imports::Imports::default();
    for (index, shape) in shapes.into_iter().enumerate() {
        let package = &workspace.packages[packages[index]];
        let mut program = bodies.next().unwrap();
        if let Err(diagnostics) =
            native.expand(&mut program, Some(&workspace), packages[index], options)
        {
            return Err(Error {
                sources: loader.sources,
                diagnostics: diagnostics.iter().map(|d| bundle.diagnostic(d)).collect(),
            });
        }
        let mut exports: Vec<Option<ast::Program>> = shape
            .into_iter()
            .skip(1)
            .map(|present| present.then(|| bodies.next().unwrap()))
            .collect();
        let dependencies = package
            .dependencies
            .iter()
            .filter_map(|(alias, id)| {
                packages
                    .iter()
                    .position(|p| p == id)
                    .map(|i| (alias.clone(), i))
            })
            .collect();
        for export in exports.iter_mut().flatten() {
            if let Err(diagnostics) =
                native.expand(export, Some(&workspace), packages[index], options)
            {
                return Err(Error {
                    sources: loader.sources,
                    diagnostics: diagnostics.iter().map(|d| bundle.diagnostic(d)).collect(),
                });
            }
        }
        units.push(super::resolve::Unit {
            program,
            name: package.name.clone(),
            dependencies,
            exports,
        });
    }
    let (program, graph, diagnostics) =
        super::resolve::resolve_units(units, selection, loader.sources.get(bundle.file));
    if !diagnostics.is_empty() {
        return Err(Error {
            sources: loader.sources,
            diagnostics: diagnostics.iter().map(|d| bundle.diagnostic(d)).collect(),
        });
    }
    if let Err(diagnostics) = native.validate(&program) {
        return Err(Error {
            sources: loader.sources,
            diagnostics: diagnostics.iter().map(|d| bundle.diagnostic(d)).collect(),
        });
    }
    Ok(Loaded {
        sources: loader.sources,
        bundle,
        program,
        graph,
        inputs: loader.inputs,
        entry: entry.canonicalize().unwrap_or(entry),
        cargo: Some(workspace),
        packages,
        native,
    })
}
fn same_path(a: &Path, b: &Path) -> bool {
    a == b
        || a.canonicalize()
            .ok()
            .zip(b.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}

struct Loader {
    sources: SourceMap,
    inputs: BTreeMap<PathBuf, String>,
    stack: Vec<PathBuf>,
    fragments: Vec<(Span, String)>,
    bytes: usize,
}
impl Loader {
    fn file(
        &mut self,
        path: &Path,
        dir: &Path,
        origin: Span,
        depth: usize,
    ) -> Result<(), Diagnostic> {
        if depth >= crate::limits::MAX_PARSER_DEPTH {
            return Err(Diagnostic::error(
                "L0501",
                "module nesting exceeds MAX_PARSER_DEPTH",
                origin,
            ));
        }
        let canonical = path.canonicalize().map_err(|e| {
            Diagnostic::error(
                "L0501",
                format!("cannot load module `{}`: {e}", path.display()),
                origin,
            )
        })?;
        if self.stack.contains(&canonical) {
            return Err(
                Diagnostic::error("L0501", "cyclic module file inclusion", origin)
                    .note(canonical.display().to_string()),
            );
        }
        let size = std::fs::metadata(&canonical)
            .map_err(|e| Diagnostic::error("L0501", e.to_string(), origin))?
            .len();
        if size > crate::limits::MAX_SOURCE_BYTES.saturating_sub(self.bytes) as u64 {
            return Err(Diagnostic::error(
                "L0010",
                "assembled modules exceed MAX_SOURCE_BYTES",
                origin,
            ));
        }
        let text = std::fs::read_to_string(&canonical)
            .map_err(|e| Diagnostic::error("L0501", e.to_string(), origin))?;
        if text.len() > crate::limits::MAX_SOURCE_BYTES.saturating_sub(self.bytes) {
            return Err(Diagnostic::error(
                "L0010",
                "assembled modules exceed MAX_SOURCE_BYTES",
                origin,
            ));
        }
        self.bytes += text.len();
        self.inputs.insert(canonical.clone(), text.clone());
        let file = self.sources.add(path.display().to_string(), text);
        self.stack.push(canonical);
        let result = self.expand(file, 0, self.sources.get(file).text().len(), dir, depth);
        self.stack.pop();
        result
    }
    fn copy(&mut self, file: FileId, start: usize, end: usize) {
        if start < end {
            self.fragments.push((
                Span::new(file, start, end),
                self.sources.get(file).text()[start..end].into(),
            ));
        }
    }
    fn expand(
        &mut self,
        file: FileId,
        start: usize,
        end: usize,
        dir: &Path,
        depth: usize,
    ) -> Result<(), Diagnostic> {
        if depth >= crate::limits::MAX_PARSER_DEPTH {
            return Err(Diagnostic::error(
                "L0501",
                "module nesting exceeds MAX_PARSER_DEPTH",
                Span::new(file, start, start),
            ));
        }
        let lexed = lexer::lex(self.sources.get(file));
        let tokens = &lexed.tokens;
        for token in tokens {
            if token.kind == K::Name
                && self
                    .sources
                    .get(file)
                    .slice(token.span)
                    .is_some_and(|s| s.starts_with("__locus_") || s.starts_with("LocusM"))
            {
                return Err(Diagnostic::error(
                    "L0502",
                    "this identifier prefix is reserved for generated module identities",
                    token.span,
                ));
            }
        }
        let mut i = tokens.partition_point(|t| t.span.start < start);
        let mut cursor = start;
        let mut nesting = 0usize;
        while i < tokens.len() && tokens[i].span.start < end {
            let t = tokens[i];
            if nesting == 0
                && t.kind == K::Keyword
                && self.sources.get(file).slice(t.span) == Some("mod")
                && tokens.get(i + 1).is_some_and(|t| t.kind == K::Name)
            {
                let name = self
                    .sources
                    .get(file)
                    .slice(tokens[i + 1].span)
                    .unwrap()
                    .to_string();
                if let Some(next) = tokens.get(i + 2) {
                    if next.kind == K::Semicolon {
                        self.copy(file, cursor, next.span.start);
                        self.fragments.push((next.span, "{".into()));
                        let first = dir.join(format!("{name}.lc"));
                        let second = dir.join(&name).join("mod.lc");
                        let selected = match (first.is_file(), second.is_file()) {
                            (true, false) => first,
                            (false, true) => second,
                            (true, true) => {
                                return Err(Diagnostic::error(
                                    "L0501",
                                    format!(
                                        "ambiguous module `{name}`: both {} and {} exist",
                                        first.display(),
                                        second.display()
                                    ),
                                    tokens[i + 1].span,
                                ));
                            }
                            _ => {
                                return Err(Diagnostic::error(
                                    "L0501",
                                    format!("module `{name}` not found"),
                                    tokens[i + 1].span,
                                )
                                .note(format!(
                                    "expected {} or {}",
                                    first.display(),
                                    second.display()
                                )));
                            }
                        };
                        self.file(&selected, &dir.join(&name), tokens[i + 1].span, depth + 1)?;
                        self.fragments.push((next.span, "}".into()));
                        cursor = next.span.end;
                        i += 3;
                        continue;
                    }
                    if next.kind == K::LBrace {
                        let mut j = i + 3;
                        let mut braces = 1;
                        while j < tokens.len() && braces > 0 {
                            if tokens[j].kind == K::LBrace {
                                braces += 1;
                            }
                            if tokens[j].kind == K::RBrace {
                                braces -= 1;
                            }
                            if braces > 0 {
                                j += 1;
                            }
                        }
                        if j < tokens.len() && braces == 0 {
                            self.copy(file, cursor, next.span.end);
                            self.expand(
                                file,
                                next.span.end,
                                tokens[j].span.start,
                                &dir.join(&name),
                                depth + 1,
                            )?;
                            self.copy(file, tokens[j].span.start, tokens[j].span.end);
                            cursor = tokens[j].span.end;
                            i = j + 1;
                            continue;
                        }
                    }
                }
            }
            match t.kind {
                K::LBrace | K::LParen | K::LBracket => nesting += 1,
                K::RBrace | K::RParen | K::RBracket => nesting = nesting.saturating_sub(1),
                _ => {}
            }
            i += 1;
        }
        self.copy(file, cursor, end);
        Ok(())
    }
}
