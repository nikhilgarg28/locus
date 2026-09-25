use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use locus::diagnostic::{self, Diagnostic, Level};
use locus::elab;
use locus::erased::{EType, Interpreter, Markers, Outcome, Value, print_module_with};
use locus::kernel::Integer;
use locus::lexer;
use locus::parser;
use locus::preview::Previews;
use locus::source::{SourceBundle, SourceMap, Span};
use locus::store::{self, Lockfile, ProofStore};

const HELP: &str = "Locus

Usage: locus <command> <file.lc> [arguments]

  check   Check types and proofs; --holes lists every `_` and how it was filled,
          --stats what each function cost to elaborate and to check, and the
          obligations counted by the tier that filled them. The proofs found
          are stored in Locus.lock in the source directory and used again on
          the next run; --locked never searches, so a missing or stale entry
          is an error, and --no-store neither reads nor writes the file.
          Legacy <file.lc>.proofs files migrate after a successful unlocked check
  bench   Measure checking/search/replay counts and timings as JSON
  explain Explain a diagnostic code: locus explain L0230
  audit   Check files/directories, then list trust, divergence, panic and classical sites
  run     Check, then interpret a function: locus run <file.lc> <function> [u8|true|false]...
  rust    Check, then print the generated Rust
  build   Check, then write a Rust crate: locus build <file.lc>... --out <dir> [--name <crate>]
          The crate's root defines the markers and holds one module per file
  tokens  Print tokens and their original source spans
  parse   Validate syntax only
  ast     Print the syntax tree of a syntactically valid file

  --error-format <text|json>  Render diagnostics as text (default) or schema-versioned JSON
  --library <file.lc>  Include checked declarations (repeat for multiple libraries)
  --preview <name>  Enable an unfinished feature (repeat for multiple features)
  -h, --help     Show this help
  -V, --version  Show the version
";

/// Steps the interpreter may take before it reports that it ran out.
use locus::limits::DEFAULT_RUN_FUEL as FUEL;

/// The tiers in the order they are tried, for the counts of `--stats`; a
/// tier not listed here, such as the lemma a `for` from `0` is filled by,
/// follows them, and `unsolved` last.
const TIERS: [&str; 5] = ["stored", "exact", "computed", "evaluation", "arithmetic"];

/// The flags `check` takes after the file.
const CHECK_FLAGS: [&str; 4] = ["--holes", "--stats", "--locked", "--no-store"];

/// How `check` uses the proofs file: read from the environment and the
/// flags. `LOCUS_PROOFS=off` is `--no-store`; `LOCUS_SEARCH=none` makes
/// every tier fail, which is the test hook for surviving an upgrade: a file
/// with every proof still checks under it.
struct StoreOptions {
    enabled: bool,
    locked: bool,
    search: bool,
}

impl StoreOptions {
    fn from(flags: &[&str]) -> Self {
        let off = |name: &str, value: &str| env::var(name).is_ok_and(|found| found == value);
        Self {
            enabled: !flags.contains(&"--no-store") && !off("LOCUS_PROOFS", "off"),
            locked: flags.contains(&"--locked"),
            search: !off("LOCUS_SEARCH", "none"),
        }
    }
}

/// The lockfile of a source: `Locus.lock` in the source's directory, and
/// the name the source has in it, its file name.
fn lock_path(source: &Path) -> (PathBuf, String) {
    let directory = source.parent().unwrap_or(Path::new(""));
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    (directory.join(store::FILE_NAME), name)
}

/// The path of a version-1 proofs file: the source's path with `.proofs`
/// appended.
fn proofs_path(source: &OsString) -> OsString {
    let mut path = source.clone();
    path.push(".proofs");
    path
}

/// A version-1 proofs file to move into the lockfile: its path and how
/// many entries it held.
struct Migration {
    sidecar: OsString,
    entries: usize,
}

/// The lockfile of the source, read, with the store to check the source
/// with taken out of it, and the version-1 proofs file to migrate when the
/// lockfile has no entry for the source and there is one.
struct Opened {
    lockfile: Lockfile,
    store: ProofStore,
    migration: Option<Migration>,
}

/// Opens the store of `source`: `Ok(None)` when a file could not be read
/// for a reason other than not being there, after reporting it.
fn open_store(
    source: &OsString,
    lock: &Path,
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) -> io::Result<Option<Opened>> {
    let mut lockfile = match read_store_text(lock) {
        Ok(text) => match Lockfile::parse(&text) {
            Ok((lockfile, warnings)) => {
                for warning in warnings {
                    diagnostics.push(Diagnostic::warning(
                        "L0402",
                        format!("{}: {warning}", lock.display()),
                        span,
                    ));
                }
                lockfile
            }
            Err(problem) => {
                diagnostics.push(Diagnostic::warning("L0402", format!("{}: {problem}; every proof of `{name}` is searched for and the file is rewritten with them",lock.display()),span));
                Lockfile::new()
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Lockfile::new(),
        Err(error) => {
            diagnostics.push(Diagnostic::error(
                "L0401",
                format!("cannot read {}: {error}", lock.display()),
                span,
            ));
            return Ok(None);
        }
    };
    let sidecar = proofs_path(source);
    if lockfile.has(name) {
        if fs::metadata(&sidecar).is_ok() {
            diagnostics.push(Diagnostic::warning(
                "L0402",
                format!(
                    "{} is ignored: {} already holds `{name}`; delete it",
                    sidecar.to_string_lossy(),
                    lock.display()
                ),
                span,
            ));
        }
        let store = lockfile.take(name);
        return Ok(Some(Opened {
            lockfile,
            store,
            migration: None,
        }));
    }
    match read_store_text(Path::new(&sidecar)) {
        Ok(text) => match store::v1::parse(&text) {
            Ok((entries, warnings)) => {
                for warning in warnings {
                    diagnostics.push(Diagnostic::warning(
                        "L0402",
                        format!("{}: {warning}", sidecar.to_string_lossy()),
                        span,
                    ));
                }
                let migration = Migration {
                    sidecar,
                    entries: entries.len(),
                };
                Ok(Some(Opened {
                    lockfile,
                    store: ProofStore::with_entries(entries),
                    migration: Some(migration),
                }))
            }
            Err(problem) => {
                diagnostics.push(Diagnostic::warning(
                    "L0402",
                    format!("{}: {problem}; it is not read", sidecar.to_string_lossy()),
                    span,
                ));
                Ok(Some(Opened {
                    lockfile,
                    store: ProofStore::new(),
                    migration: None,
                }))
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Some(Opened {
            lockfile,
            store: ProofStore::new(),
            migration: None,
        })),
        Err(error) => {
            diagnostics.push(Diagnostic::error(
                "L0401",
                format!("cannot read {}: {error}", sidecar.to_string_lossy()),
                span,
            ));
            Ok(None)
        }
    }
}

/// Under `--locked`, an obligation a stale entry fails is reported by the
/// elaborator as a miss. This adds to that report what the entry
/// concluded and what the obligation wanted, when the store knows: the
/// misses of a function are reported in the order they were met, so the
/// reports of a function and the store's misses for it pair up whenever
/// they are equally many.
fn note_stale_claims(diagnostics: &mut [Diagnostic], store: &ProofStore) {
    let mut by_function: BTreeMap<&str, Vec<&store::Miss>> = BTreeMap::new();
    for miss in store.misses() {
        by_function
            .entry(miss.label.function.as_str())
            .or_default()
            .push(miss);
    }
    for (function, misses) in by_function {
        let prefix = format!("`{function}` needs a proof of `");
        let reports: Vec<usize> = diagnostics
            .iter()
            .enumerate()
            .filter(|(_, diagnostic)| {
                diagnostic.code == "L0230" && diagnostic.message.starts_with(&prefix)
            })
            .map(|(index, _)| index)
            .collect();
        if reports.len() != misses.len() {
            continue;
        }
        for (index, miss) in reports.into_iter().zip(misses) {
            if let Some((stored, wanted)) = &miss.stale {
                diagnostics[index].notes.push(format!(
                    "the stored proof concludes `{stored}`, the obligation wants `{wanted}`"
                ));
            }
        }
    }
}

/// The size of the certificate the arithmetic tier found, for a report:
/// `, N pairs` after the tier, and nothing for any other tier.
fn pairs_of(hole: &elab::HoleReport) -> String {
    if hole.tier != "arithmetic" {
        return String::new();
    }
    let pairs = hole
        .found
        .as_ref()
        .map_or(0, |found| elab::certificate_pairs(&found.proof));
    format!(", {pairs} pairs")
}

/// The exit status of a Rust program that panicked.
const PANICKED: u8 = 101;

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let format = if arguments.iter().any(|arg| arg == "--error-format=json")
        || arguments
            .windows(2)
            .any(|args| args[0] == "--error-format" && args[1] == "json")
    {
        DiagnosticFormat::Json
    } else {
        DiagnosticFormat::Text
    };
    match run(arguments, format) {
        Ok(code) => ExitCode::from(code),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            let _ = emit_driver(format, Level::Error, "L0401", &error.to_string());
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<OsString>, format: DiagnosticFormat) -> io::Result<u8> {
    let arguments = match format_arguments(arguments) {
        Ok(arguments) => arguments,
        Err(message) => {
            emit_driver(format, Level::Error, "L0400", &message)?;
            return Ok(2);
        }
    };
    let mut output = io::BufWriter::new(io::stdout().lock());
    if arguments.is_empty() || matches!(arguments[0].to_str(), Some("-h" | "--help")) {
        write!(output, "{HELP}")?;
        output.flush()?;
        return Ok(0);
    }
    if matches!(arguments[0].to_str(), Some("-V" | "--version")) {
        writeln!(output, "locus {}", env!("CARGO_PKG_VERSION"))?;
        output.flush()?;
        return Ok(0);
    }
    let (arguments, previews) = match preview_arguments(arguments) {
        Ok(parsed) => parsed,
        Err(message) => {
            emit_driver(format, Level::Error, "L0400", &message)?;
            return Ok(2);
        }
    };
    let (arguments, libraries) = match library_arguments(arguments) {
        Ok(parsed) => parsed,
        Err(message) => {
            emit_driver(format, Level::Error, "L0400", &message)?;
            return Ok(2);
        }
    };
    let elab_options = elab::Options {
        previews,
        ..elab::Options::default()
    };
    let command = arguments[0].to_str().unwrap_or("");
    if command == "bench" {
        match locus::bench::command(&arguments[1..], &elab_options, &libraries) {
            Ok(json) => {
                writeln!(output, "{json}")?;
                output.flush()?;
                return Ok(0);
            }
            Err(message) => {
                emit_driver(format, Level::Error, "L0400", &message)?;
                return Ok(2);
            }
        }
    }
    if command == "explain" {
        if arguments.len() != 2 || !libraries.is_empty() {
            emit_driver(
                format,
                Level::Error,
                "L0400",
                "expected `locus explain L0230`",
            )?;
            return Ok(2);
        }
        let code = arguments[1].to_string_lossy();
        let Some(explanation) = diagnostic::explain::explanation(&code) else {
            emit_driver(
                format,
                Level::Error,
                "L0404",
                &format!("unknown diagnostic code `{code}`"),
            )?;
            return Ok(2);
        };
        write!(output, "{explanation}")?;
        output.flush()?;
        return Ok(0);
    }
    if matches!(command, "check" | "rust" | "run" | "build" | "audit")
        && uses_project_driver(&arguments)
    {
        if !libraries.is_empty() {
            emit_driver(
                format,
                Level::Error,
                "L0400",
                "module/package builds use `mod` and `use`; --library is the legacy flat-file mechanism",
            )?;
            return Ok(2);
        }
        return project_command(&arguments, &elab_options, format);
    }
    if command == "build" {
        return build(&arguments[1..], &elab_options, &libraries, format);
    }
    if command == "audit"
        && (arguments.len() > 2
            || arguments
                .get(1)
                .is_some_and(|path| std::path::Path::new(path).is_dir()))
    {
        let inputs: Vec<_> = arguments[1..]
            .iter()
            .map(std::path::PathBuf::from)
            .collect();
        let files = locus::audit::source_paths(&inputs)?;
        let mut status = 0;
        for path in files {
            writeln!(output, "{}:", path.display())?;
            output.flush()?;
            let mut args = vec![OsString::from("audit"), path.into_os_string()];
            for library in &libraries {
                args.extend([OsString::from("--library"), library.clone()]);
            }
            for feature in elab_options.previews.iter() {
                args.extend([OsString::from("--preview"), OsString::from(feature.name())]);
            }
            status = status.max(run(args, format)?);
        }
        return Ok(status);
    }
    let flags: Vec<&str> = arguments
        .iter()
        .skip(2)
        .map(|flag| flag.to_str().unwrap_or(""))
        .collect();
    let well_formed = match command {
        "tokens" | "parse" | "ast" | "rust" | "audit" => arguments.len() == 2,
        "check" => arguments.len() >= 2 && flags.iter().all(|flag| CHECK_FLAGS.contains(flag)),
        "run" => arguments.len() >= 3,
        _ => false,
    };
    if !well_formed {
        emit_driver(
            format,
            Level::Error,
            "L0400",
            "expected `locus <check|run|rust|tokens|parse|ast> <file.lc>`\nUse `locus --help` for available commands.",
        )?;
        return Ok(2);
    }
    let path = &arguments[1];
    if !source_size_allowed(path, 0, format)? {
        return Ok(1);
    }
    let text = match read_source(path) {
        Ok(text) => text,
        Err(error) => {
            emit_driver(
                format,
                Level::Error,
                "L0401",
                &format!("cannot read {}: {error}", path.to_string_lossy()),
            )?;
            return Ok(1);
        }
    };
    let mut sources = SourceMap::default();
    let Some(bundle) = load_bundle(&mut sources, path, text, &libraries, format)? else {
        return Ok(1);
    };
    let driver_file = sources.add("<driver>", "");
    let driver_span = Span::new(driver_file, 0, 0);
    let source = sources.get(bundle.file);
    if command == "tokens" {
        let lexed = lexer::lex(source);
        for token in lexed.tokens {
            let original_span = bundle.span(token.span);
            if !libraries.is_empty() {
                write!(output, "{}:", sources.get(original_span.file).name)?;
            }
            writeln!(
                output,
                "{:>6}..{:<6} {:<14} {:?}",
                original_span.start,
                original_span.end,
                format!("{:?}", token.kind),
                source.slice(token.span).unwrap()
            )?;
        }
        output.flush()?;
        emit_bundle_diagnostics(&sources, &bundle, &lexed.diagnostics, format)?;
        return Ok(u8::from(!lexed.diagnostics.is_empty()));
    }
    let parsed = parser::parse(source);
    if !parsed.is_success() {
        emit_bundle_diagnostics(&sources, &bundle, &parsed.diagnostics, format)?;
        return Ok(1);
    }
    if matches!(command, "check" | "run" | "rust" | "audit") {
        // The directory lockfile, for `check`: read before
        // elaboration, written after it when it changed, unless `--locked`,
        // under which nothing is searched and nothing is written.
        let mut pending = Vec::new();
        let options = StoreOptions::from(&flags);
        let (lock, name) = lock_path(Path::new(path));
        let (mut lockfile, store, migration) = if command == "check" && options.enabled {
            let Some(opened) = open_store(path, &lock, &name, &mut pending, driver_span)? else {
                emit_diagnostics(&sources, &pending, format)?;
                return Ok(1);
            };
            (
                opened.lockfile,
                Some(
                    opened
                        .store
                        .locked(options.locked)
                        .searching(options.search),
                ),
                opened.migration,
            )
        } else {
            (Lockfile::new(), None, None)
        };
        let (mut elaborated, store) = match store {
            Some(store) => {
                let (elaborated, store) = elab::elaborate_with_store_and_options(
                    source,
                    &parsed.program,
                    store,
                    &elab_options,
                );
                (elaborated, Some(store))
            }
            None => (
                elab::elaborate_with_options(source, &parsed.program, &elab_options),
                None,
            ),
        };
        if let Some(store) = &store
            && options.locked
        {
            note_stale_claims(&mut elaborated.diagnostics, store);
        }
        pending.extend(elaborated.diagnostics.iter().map(|d| bundle.diagnostic(d)));
        if command == "check" && flags.contains(&"--stats") {
            writeln!(
                output,
                "{:<28} {:>14} {:>12}",
                "function", "elaborate (us)", "check (us)"
            )?;
            for item in &elaborated.items {
                writeln!(
                    output,
                    "{:<28} {:>14} {:>12}",
                    item.name, item.elaborate_micros, item.check_micros
                )?;
            }
            let search: u128 = elaborated.holes.iter().map(|hole| hole.micros).sum();
            let nodes: usize = elaborated.holes.iter().map(|hole| hole.proof_size).sum();
            writeln!(
                output,
                "total: {} us elaborating ({} us of it searching for {} proofs, {} proof nodes), {} us checking",
                elaborated
                    .items
                    .iter()
                    .map(|item| item.elaborate_micros)
                    .sum::<u128>(),
                search,
                elaborated.holes.len(),
                nodes,
                elaborated
                    .items
                    .iter()
                    .map(|item| item.check_micros)
                    .sum::<u128>(),
            )?;
            // The obligations by tier: every `_`, `prove!`, conversion of
            // evidence, and operator premise under `no_panic`, counted
            // under the tier that filled it, then listed with their lines.
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for hole in &elaborated.holes {
                *counts.entry(hole.tier).or_default() += 1;
            }
            if let Some(store) = &store {
                let stats = store.stats();
                writeln!(
                    output,
                    "Locus.lock: {} used, {} found and recorded, {} stale, {} searched, {} unwritable",
                    stats.hits, stats.recorded, stats.stale, stats.searches, stats.unprintable
                )?;
            }
            let mut listed: Vec<String> = Vec::new();
            for tier in TIERS {
                if let Some(count) = counts.remove(tier) {
                    listed.push(format!("{count} {tier}"));
                }
            }
            for (tier, count) in counts {
                listed.push(format!("{count} {tier}"));
            }
            writeln!(
                output,
                "obligations: {} ({})",
                elaborated.holes.len(),
                listed.join(", ")
            )?;
            let mut holes: Vec<&elab::HoleReport> = elaborated.holes.iter().collect();
            holes.sort_by_key(|hole| hole.span.start);
            for hole in holes {
                let location = bundle.span(hole.span);
                let original = sources.get(location.file);
                let (line, column) = original.line_column(location.start).unwrap_or((0, 0));
                if libraries.is_empty() {
                    writeln!(output, "  {line}:{column} {}{}", hole.tier, pairs_of(hole))?;
                } else {
                    writeln!(
                        output,
                        "  {}:{line}:{column} {}{}",
                        original.name,
                        hole.tier,
                        pairs_of(hole)
                    )?;
                }
            }
        } else if command == "check" && flags.contains(&"--holes") {
            for hole in &elaborated.holes {
                let location = bundle.span(hole.span);
                let original = sources.get(location.file);
                let (line, column) = original.line_column(location.start).unwrap_or((0, 0));
                writeln!(
                    output,
                    "{}:{line}:{column}: {} ({}{}, {} proof nodes, {} us)",
                    original.name,
                    if hole.solved { "filled" } else { "unsolved" },
                    hole.tier,
                    pairs_of(hole),
                    hole.proof_size,
                    hole.micros
                )?;
            }
        }
        if !elaborated.is_success() {
            output.flush()?;
            emit_diagnostics(&sources, &pending, format)?;
            return Ok(1);
        }
        if let Some(store) = &store {
            if options.locked {
                if let Some(migration) = &migration {
                    store_notice(
                        &mut output,
                        format,
                        &format!(
                            "{} was read, and is moved into {} by a run without `--locked`",
                            migration.sidecar.to_string_lossy(),
                            lock.display()
                        ),
                    )?;
                }
            } else {
                lockfile.put(&name, store);
                if let Some(text) = lockfile.changed()
                    && let Err(error) = fs::write(&lock, text)
                {
                    output.flush()?;
                    pending.push(Diagnostic::error(
                        "L0401",
                        format!("cannot write {}: {error}", lock.display()),
                        driver_span,
                    ));
                    emit_diagnostics(&sources, &pending, format)?;
                    return Ok(1);
                }
                if let Some(migration) = &migration {
                    let deleted = match fs::remove_file(&migration.sidecar) {
                        Ok(()) => true,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => true,
                        Err(error) => {
                            pending.push(Diagnostic::warning(
                                "L0402",
                                format!(
                                    "cannot delete {}: {error}",
                                    migration.sidecar.to_string_lossy()
                                ),
                                driver_span,
                            ));
                            false
                        }
                    };
                    store_notice(
                        &mut output,
                        format,
                        &format!(
                            "{} proof(s) of {} were moved into {}, {} of them in use, and the file is {}",
                            migration.entries,
                            migration.sidecar.to_string_lossy(),
                            lock.display(),
                            store.used().saturating_sub(store.stats().recorded),
                            if deleted { "deleted" } else { "left in place" }
                        ),
                    )?;
                }
            }
        }
        // Warnings, when the file is accepted with some.
        output.flush()?;
        emit_diagnostics(&sources, &pending, format)?;
        let module = elaborated.session.erased();
        match command {
            "audit" => write!(output, "{}", locus::audit::render(&elaborated))?,
            "check" => writeln!(
                output,
                "Checked {} function(s); {} proof(s) found and accepted by the kernel.",
                elaborated.functions.len(),
                elaborated.holes.len()
            )?,
            "rust" => write!(
                output,
                "{}",
                print_module_with(module, &elaborated.visibilities, Markers::Here)
            )?,
            _ => {
                let name = arguments[2].to_string_lossy();
                let Some(function) = elaborated.function(&name) else {
                    writeln!(
                        io::stderr(),
                        "error: no function `{name}` in {}",
                        source.name
                    )?;
                    return Ok(1);
                };
                // Each argument is read at the type of its parameter: a
                // `bool`, or a machine integer within its type's range.
                let params: Vec<EType> = module
                    .fns
                    .iter()
                    .find(|item| item.reference == function)
                    .map(|item| item.params.iter().map(|(_, _, ty)| ty.clone()).collect())
                    .unwrap_or_default();
                let mut values = Vec::new();
                for (index, argument) in arguments[3..].iter().enumerate() {
                    let text = argument.to_string_lossy();
                    let value = match (params.get(index), text.as_ref()) {
                        (Some(EType::Bool), "true") => Some(Value::Bool(true)),
                        (Some(EType::Bool), "false") => Some(Value::Bool(false)),
                        (Some(EType::Int(ty)), text) => text
                            .parse::<i128>()
                            .ok()
                            .filter(|value| ty.contains(&Integer::from(*value)))
                            .map(|value| Value::Int(*ty, value)),
                        _ => None,
                    };
                    let Some(value) = value else {
                        writeln!(
                            io::stderr(),
                            "error: `{text}` is not a value of the parameter's type; only a `bool` or a machine integer can be given on the command line"
                        )?;
                        return Ok(2);
                    };
                    values.push(value);
                }
                match Interpreter::new(module, FUEL).call(function, values) {
                    Ok(Outcome::Value(value)) => writeln!(output, "{}", value.debug(module))?,
                    // As a Rust program reports a panic: the message on
                    // stderr, and exit status 101.
                    Ok(Outcome::Panic(message)) => {
                        output.flush()?;
                        writeln!(io::stderr(), "`{name}` panicked:\n{message}")?;
                        return Ok(PANICKED);
                    }
                    Ok(Outcome::OutOfFuel) => {
                        output.flush()?;
                        writeln!(
                            io::stderr(),
                            "error: `{name}` did not return within {FUEL} steps"
                        )?;
                        return Ok(1);
                    }
                    Err(error) => {
                        output.flush()?;
                        writeln!(io::stderr(), "error: {error}")?;
                        return Ok(1);
                    }
                }
            }
        }
    } else if command == "ast" {
        writeln!(output, "{:#?}", parsed.program)?;
    } else {
        writeln!(
            output,
            "Parsed {} declaration(s). Syntax only; types and proofs have not been checked.",
            parsed.program.declarations.len()
        )?;
    }
    output.flush()?;
    Ok(0)
}

/// Strip repeatable preview options before each command checks its own
/// arguments. Feature names are validated before reading or writing files.
fn preview_arguments(arguments: Vec<OsString>) -> Result<(Vec<OsString>, Previews), String> {
    let mut arguments = arguments.into_iter();
    let mut remaining = vec![arguments.next().expect("a command was provided")];
    let mut previews = Previews::default();
    while let Some(argument) = arguments.next() {
        if argument == "--preview" {
            let name = arguments.next().ok_or("`--preview` takes a feature name")?;
            let name = name.to_str().ok_or("a preview name must be valid UTF-8")?;
            if name.starts_with("--") {
                return Err("`--preview` takes a feature name".into());
            }
            previews.enable(name).map_err(|error| error.to_string())?;
        } else {
            remaining.push(argument);
        }
    }
    Ok((remaining, previews))
}

/// `locus build <file.lc>... --out <dir> [--name <crate>]`: checks every
/// file, and writes the crate when all of them pass. The exit status is 2
/// for a usage error, 1 when a file is rejected or cannot be read or
/// written, and 0 when the crate is written.
fn build(
    arguments: &[OsString],
    options: &elab::Options,
    libraries: &[OsString],
    format: DiagnosticFormat,
) -> io::Result<u8> {
    let usage = |message: &str| -> io::Result<u8> {
        emit_driver(
            format,
            Level::Error,
            "L0400",
            &format!("{message}\nUse `locus build <file.lc>... --out <dir> [--name <crate>]`."),
        )?;
        Ok(2)
    };
    let mut files: Vec<&OsString> = Vec::new();
    let mut out: Option<&OsString> = None;
    let mut name: Option<&OsString> = None;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--out") => match arguments.next() {
                Some(directory) if out.is_none() => out = Some(directory),
                Some(_) => return usage("`--out` is given twice"),
                None => return usage("`--out` takes a directory"),
            },
            Some("--name") => match arguments.next() {
                Some(crate_name) if name.is_none() => name = Some(crate_name),
                Some(_) => return usage("`--name` is given twice"),
                None => return usage("`--name` takes a crate name"),
            },
            Some(option) if option.starts_with("--") => {
                return usage(&format!("unknown option `{option}`"));
            }
            _ => files.push(argument),
        }
    }
    let Some(out) = out else {
        return usage("`--out <dir>` says where the crate goes");
    };
    if files.is_empty() {
        return usage("at least one file to build");
    }
    let out = std::path::Path::new(out);
    let crate_name = match name {
        Some(name) => name.to_string_lossy().into_owned(),
        None => match locus::build::crate_name_of(out) {
            Ok(name) => name,
            Err(message) => return usage(&message),
        },
    };
    let mut sources = SourceMap::default();
    let mut modules: Vec<(String, String)> = Vec::new();
    let mut rejected = false;
    for path in files {
        let module = match locus::build::module_name(std::path::Path::new(path)) {
            Ok(module) => module,
            Err(message) => return usage(&message),
        };
        if modules.iter().any(|(known, _)| *known == module) {
            return usage(&format!(
                "two files would both be the module `{module}`; a file's name is its module's"
            ));
        }
        if !source_size_allowed(path, 0, format)? {
            return Ok(1);
        }
        let text = match read_source(path) {
            Ok(text) => text,
            Err(error) => {
                emit_driver(
                    format,
                    Level::Error,
                    "L0401",
                    &format!("cannot read {}: {error}", path.to_string_lossy()),
                )?;
                return Ok(1);
            }
        };
        let Some(bundle) = load_bundle(&mut sources, path, text, libraries, format)? else {
            rejected = true;
            continue;
        };
        let source = sources.get(bundle.file);
        let parsed = parser::parse(source);
        if !parsed.is_success() {
            emit_bundle_diagnostics(&sources, &bundle, &parsed.diagnostics, format)?;
            rejected = true;
            continue;
        }
        let elaborated = elab::elaborate_with_options(source, &parsed.program, options);
        if !elaborated.is_success() {
            emit_bundle_diagnostics(&sources, &bundle, &elaborated.diagnostics, format)?;
            rejected = true;
            continue;
        }
        let rust = print_module_with(
            elaborated.session.erased(),
            &elaborated.visibilities,
            Markers::InRoot,
        );
        modules.push((module, rust));
    }
    if rejected {
        return Ok(1);
    }
    let written = match locus::build::write_crate(out, &crate_name, &modules) {
        Ok(written) => written,
        Err(error) => {
            emit_driver(
                format,
                Level::Error,
                "L0401",
                &format!("cannot write the crate under {}: {error}", out.display()),
            )?;
            return Ok(1);
        }
    };
    let mut output = io::stdout().lock();
    writeln!(
        output,
        "Wrote the crate `{crate_name}`, {} module(s), under {}:",
        modules.len(),
        out.display()
    )?;
    for path in written {
        writeln!(output, "  {}", path.display())?;
    }
    output.flush()?;
    Ok(0)
}

fn emit_diagnostics(
    sources: &SourceMap,
    diagnostics: &[Diagnostic],
    format: DiagnosticFormat,
) -> io::Result<()> {
    let mut error_output = io::stderr().lock();
    let color = error_output.is_terminal() && env::var_os("NO_COLOR").is_none();
    for diagnostic in diagnostic::sorted(sources, diagnostics) {
        let rendered = match format {
            DiagnosticFormat::Json => diagnostic.render_json(sources),
            DiagnosticFormat::Text if diagnostic.code.starts_with("L04") => format!(
                "{}: {}",
                if diagnostic.is_error() {
                    "error"
                } else {
                    "warning"
                },
                diagnostic.message
            ),
            DiagnosticFormat::Text => diagnostic.render(sources, color),
        };
        writeln!(error_output, "{rendered}")?;
    }
    Ok(())
}

/// Libraries deliberately share the entry module's namespace. Duplicates are
/// ordinary duplicate declarations, not shadowed imports or trusted preludes.
fn library_arguments(arguments: Vec<OsString>) -> Result<(Vec<OsString>, Vec<OsString>), String> {
    let mut remaining = Vec::new();
    let mut libraries = Vec::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--library" {
            let path = arguments.next().ok_or("`--library` takes a source path")?;
            if path.to_string_lossy().starts_with("--") {
                return Err("`--library` takes a source path".into());
            }
            libraries.push(path);
        } else {
            remaining.push(argument);
        }
    }
    Ok((remaining, libraries))
}
fn load_bundle(
    sources: &mut SourceMap,
    path: &OsString,
    text: String,
    libraries: &[OsString],
    format: DiagnosticFormat,
) -> io::Result<Option<SourceBundle>> {
    let mut files = Vec::new();
    let mut assembled_bytes = text.len();
    for library in libraries {
        if !source_size_allowed(library, assembled_bytes.saturating_add(1), format)? {
            return Ok(None);
        }
        let text = read_source(library).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot read {}: {error}", library.to_string_lossy()),
            )
        })?;
        assembled_bytes = assembled_bytes.saturating_add(text.len()).saturating_add(1);
        let file = sources.add(library.to_string_lossy(), text);
        // Prevent an incomplete library from consuming the next file's tokens.
        let parsed = parser::parse(sources.get(file));
        if !parsed.is_success() {
            emit_diagnostics(sources, &parsed.diagnostics, format)?;
            return Ok(None);
        }
        files.push(file);
    }
    files.push(sources.add(path.to_string_lossy(), text));
    Ok(Some(SourceBundle::join(sources, &files)))
}
fn emit_bundle_diagnostics(
    sources: &SourceMap,
    bundle: &SourceBundle,
    diagnostics: &[Diagnostic],
    format: DiagnosticFormat,
) -> io::Result<()> {
    let mapped: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| bundle.diagnostic(diagnostic))
        .collect();
    emit_diagnostics(sources, &mapped, format)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiagnosticFormat {
    Text,
    Json,
}

fn format_arguments(arguments: Vec<OsString>) -> Result<Vec<OsString>, String> {
    let mut remaining = Vec::new();
    let mut arguments = arguments.into_iter();
    let mut seen = false;
    while let Some(argument) = arguments.next() {
        let text = argument.to_string_lossy();
        let value = if text == "--error-format" {
            Some(
                arguments
                    .next()
                    .ok_or("`--error-format` takes text or json")?,
            )
        } else {
            text.strip_prefix("--error-format=").map(OsString::from)
        };
        if let Some(value) = value {
            if seen {
                return Err("`--error-format` is given twice".into());
            }
            seen = true;
            if value != "text" && value != "json" {
                return Err("`--error-format` takes text or json".into());
            }
        } else {
            remaining.push(argument);
        }
    }
    Ok(remaining)
}
fn emit_driver(
    format: DiagnosticFormat,
    level: Level,
    code: &'static str,
    message: &str,
) -> io::Result<()> {
    let mut sources = SourceMap::default();
    let file = sources.add("<driver>", "");
    let span = Span::new(file, 0, 0);
    let diagnostic = match level {
        Level::Error => Diagnostic::error(code, message, span),
        Level::Warning => Diagnostic::warning(code, message, span),
    };
    emit_diagnostics(&sources, &[diagnostic], format)
}

/// Preflight regular-file size before allocating a source buffer. The parser
/// also checks actual bytes, covering file races and non-regular streams.
fn source_size_allowed(
    path: &OsString,
    prefix_bytes: usize,
    format: DiagnosticFormat,
) -> io::Result<bool> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(true);
    };
    if metadata.len().saturating_add(prefix_bytes as u64) <= locus::limits::MAX_SOURCE_BYTES as u64
    {
        return Ok(true);
    }
    let mut sources = SourceMap::default();
    let file = sources.add(path.to_string_lossy(), "");
    let diagnostic = Diagnostic::error(
        "L0010",
        format!(
            "MAX_SOURCE_BYTES limit of {} was exceeded",
            locus::limits::MAX_SOURCE_BYTES
        ),
        Span::new(file, 0, 0),
    );
    emit_diagnostics(&sources, &[diagnostic], format)?;
    Ok(false)
}

fn read_source(path: &OsString) -> io::Result<String> {
    let mut text = String::new();
    // A metadata race or a stream cannot force an unbounded allocation. The
    // extra byte lets the lexer report the same source-limit diagnostic.
    fs::File::open(path)?
        .take(locus::limits::MAX_SOURCE_BYTES as u64 + 1)
        .read_to_string(&mut text)?;
    Ok(text)
}

/// Informational migration output is not a warning. Keep JSON stderr strictly
/// diagnostic; text mode retains the original P12 `note:` presentation.
fn store_notice(
    output: &mut impl Write,
    format: DiagnosticFormat,
    message: &str,
) -> io::Result<()> {
    match format {
        DiagnosticFormat::Text => writeln!(io::stderr(), "note: {message}"),
        DiagnosticFormat::Json => writeln!(output, "{message}"),
    }
}
fn read_store_text(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    fs::File::open(path)?
        .take(locus::limits::MAX_PROOF_FILE_BYTES as u64 + 1)
        .read_to_string(&mut text)?;
    Ok(text)
}

/// The old flat-file commands remain compatible. Module syntax, an export
/// entry, a directory, or explicit package/output options selects the module driver.
fn uses_project_driver(arguments: &[OsString]) -> bool {
    if arguments.iter().any(|a| {
        matches!(
            a.to_str(),
            Some("--out-dir" | "--manifest-path" | "--check-receipt")
        )
    }) {
        return true;
    }
    let Some(path) = arguments.get(1).map(Path::new) else {
        return false;
    };
    if arguments[0] == "audit" && path.is_dir() && !path.join("export.lc").is_file() {
        return false;
    }
    if path.is_dir() || path.file_name().is_some_and(|n| n == "export.lc") {
        return true;
    }
    let Ok(text) = read_source(&arguments[1]) else {
        return false;
    };
    if text.len() > locus::limits::MAX_SOURCE_BYTES {
        return false;
    }
    let mut sources = SourceMap::default();
    let file = sources.add(path.display().to_string(), text);
    let tokens = lexer::lex(sources.get(file)).tokens;
    tokens.iter().any(|t| {
        t.kind == lexer::TokenKind::Keyword
            && matches!(sources.get(file).slice(t.span), Some("mod" | "use"))
    }) || tokens.windows(2).any(|pair| {
        sources.get(file).slice(pair[0].span) == Some("spec")
            && sources.get(file).slice(pair[1].span) == Some("type")
    })
}
fn project_command(
    arguments: &[OsString],
    options: &elab::Options,
    format: DiagnosticFormat,
) -> io::Result<u8> {
    let usage = |message: &str| -> io::Result<u8> {
        emit_driver(format, Level::Error, "L0400", message)?;
        Ok(2)
    };
    let command = arguments[0].to_string_lossy();
    let mut entry = None;
    let mut function = None;
    let mut values = Vec::new();
    let mut build = locus::project::Build::new(".");
    build.options = options.clone();
    build.use_proofs = env::var("LOCUS_PROOFS").as_deref() != Ok("off");
    build.write_proofs = command == "check";
    build.search_proofs = env::var("LOCUS_SEARCH").as_deref() != Ok("none");
    let mut receipt = false;
    let mut holes = false;
    let mut stats = false;
    let mut args = arguments[1..].iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--out-dir" | "--out") => {
                let Some(value) = args.next() else {
                    return usage("--out-dir takes a directory");
                };
                build.out_dir = Some(value.into());
            }
            Some("--name") => {
                let Some(value) = args.next() else {
                    return usage("--name takes a name");
                };
                build.name = value.to_string_lossy().into();
            }
            Some("--manifest-path") => {
                let Some(value) = args.next() else {
                    return usage("--manifest-path takes a Cargo.toml path");
                };
                build.cargo.manifest_path = Some(value.into());
            }
            Some("--offline") => build.cargo.offline = true,
            Some("--locked") => {
                build.cargo.locked = true;
                build.locked_proofs = command == "check";
            }
            Some("--no-store") => {
                build.use_proofs = false;
                build.write_proofs = false;
            }
            Some("--no-default-features") => build.cargo.no_default_features = true,
            Some("--all-features") => build.cargo.all_features = true,
            Some("--features") => {
                let Some(value) = args.next() else {
                    return usage("--features takes a comma-separated list");
                };
                build.cargo.features = value
                    .to_string_lossy()
                    .split(',')
                    .map(str::to_owned)
                    .collect();
            }
            Some("--target") => {
                let Some(value) = args.next() else {
                    return usage("--target takes a Rust target triple");
                };
                build.cargo.target = Some(value.to_string_lossy().into());
            }
            Some("--check-receipt") => receipt = true,
            Some("--holes") => holes = true,
            Some("--stats") => stats = true,
            Some(flag) if flag.starts_with("--") => {
                return usage(&format!("unknown project option `{flag}`"));
            }
            _ if entry.is_none() => entry = Some(PathBuf::from(arg)),
            _ if command == "run" && function.is_none() => {
                function = Some(arg.to_string_lossy().into_owned())
            }
            _ if command == "run" => values.push(arg.to_string_lossy().into_owned()),
            _ => return usage("module builds accept one entry point"),
        }
    }
    let Some(entry) = entry else {
        return usage("provide an entry .lc file or directory containing export.lc");
    };
    build.entry = entry;
    let result =
        (|| -> Result<u8, locus::project::Error> {
            if receipt {
                let current = build.is_current()?;
                println!("{}", if current { "current" } else { "stale" });
                return Ok(u8::from(!current));
            }
            if command == "build" {
                let built = build.generate()?;
                println!("{}\n{}", built.rust.display(), built.receipt.display());
                return Ok(0);
            }
            if command == "rust" {
                print!("{}", build.rust()?);
                return Ok(0);
            }
            let checked = build.check()?;
            if command == "run" {
                let name = function.as_deref().unwrap_or("main");
                let original =
                    checked.loaded.graph.items.iter().find(|i| {
                        i.original == name && i.module == checked.loaded.graph.export_root
                    });
                let alias = checked
                    .loaded
                    .graph
                    .exports(checked.loaded.graph.export_root)
                    .into_iter()
                    .find(|e| e.path.join("::") == name);
                let canonical = original.map(|i| i.canonical.as_str()).or_else(|| {
                    alias
                        .as_ref()
                        .map(|e| checked.loaded.graph.items[e.item].canonical.as_str())
                });
                let reference = canonical.and_then(|n| checked.checked.function(n));
                let Some(reference) = reference else {
                    return Err(locus::project::command_error(
                        &build.entry,
                        format!("no function `{name}` at this entry"),
                    ));
                };
                let module = checked.checked.session.erased();
                let params: Vec<_> = module
                    .fns
                    .iter()
                    .find(|f| f.reference == reference)
                    .map(|f| f.params.iter().map(|(_, _, t)| t.clone()).collect())
                    .unwrap_or_default();
                let mut inputs = Vec::new();
                for (index, value) in values.iter().enumerate() {
                    let value = match (params.get(index), value.as_str()) {
                        (Some(EType::Bool), "true") => Some(Value::Bool(true)),
                        (Some(EType::Bool), "false") => Some(Value::Bool(false)),
                        (Some(EType::Int(ty)), text) => text
                            .parse::<i128>()
                            .ok()
                            .filter(|n| ty.contains(&Integer::from(*n)))
                            .map(|n| Value::Int(*ty, n)),
                        _ => None,
                    }
                    .ok_or_else(|| {
                        locus::project::command_error(
                            &build.entry,
                            "invalid argument for runtime parameter type".into(),
                        )
                    })?;
                    inputs.push(value);
                }
                match Interpreter::new(module, FUEL).call(reference, inputs) {
                    Ok(Outcome::Value(value)) => println!("{}", value.debug(module)),
                    Ok(Outcome::Panic(message)) => {
                        eprintln!("`{name}` panicked: {message}");
                        return Ok(101);
                    }
                    Ok(Outcome::OutOfFuel) => {
                        return Err(locus::project::command_error(
                            &build.entry,
                            "execution exhausted its step budget".into(),
                        ));
                    }
                    Err(error) => {
                        return Err(locus::project::command_error(
                            &build.entry,
                            error.to_string(),
                        ));
                    }
                };
            } else {
                if holes {
                    for hole in &checked.checked.holes {
                        println!("{:?}: {}", checked.loaded.bundle.span(hole.span), hole.tier);
                    }
                }
                if stats {
                    for item in &checked.checked.items {
                        println!(
                            "{}: {} us elaborate, {} us check",
                            item.name, item.elaborate_micros, item.check_micros
                        );
                    }
                }
            }
            Ok(0)
        })();
    match result {
        Ok(status) => Ok(status),
        Err(error) => {
            emit_diagnostics(&error.sources, &error.diagnostics, format)?;
            Ok(1)
        }
    }
}
