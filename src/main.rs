use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use locus::diagnostic::Diagnostic;
use locus::elab;
use locus::erased::{EType, Interpreter, Markers, Outcome, Value, print_module_with};
use locus::kernel::Integer;
use locus::lexer;
use locus::parser;
use locus::source::SourceMap;
use locus::store::ProofStore;

const HELP: &str = "Locus

Usage: locus <command> <file.lc> [arguments]

  check   Check types and proofs; --holes lists every `_` and how it was filled,
          --stats what each function cost to elaborate and to check, and the
          obligations counted by the tier that filled them. The proofs found
          are stored in <file.lc>.proofs beside the source and used again on
          the next run; --locked never searches, so a missing or stale entry
          is an error, and --no-store neither reads nor writes the file
  run     Check, then interpret a function: locus run <file.lc> <function> [u8|true|false]...
  rust    Check, then print the generated Rust
  build   Check, then write a Rust crate: locus build <file.lc>... --out <dir> [--name <crate>]
          The crate's root defines the markers and holds one module per file
  tokens  Print tokens and their original source spans
  parse   Validate syntax only
  ast     Print the syntax tree of a syntactically valid file

  -h, --help     Show this help
  -V, --version  Show the version
";

/// Steps the interpreter may take before it reports that it ran out.
const FUEL: u64 = 10_000_000;

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

/// The path of the proofs file: the source's path with `.proofs` appended.
fn proofs_path(source: &OsString) -> OsString {
    let mut path = source.clone();
    path.push(".proofs");
    path
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
    match run(env::args_os().skip(1).collect()) {
        Ok(code) => ExitCode::from(code),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<OsString>) -> io::Result<u8> {
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
    let command = arguments[0].to_str().unwrap_or("");
    if command == "build" {
        return build(&arguments[1..]);
    }
    let flags: Vec<&str> = arguments
        .iter()
        .skip(2)
        .map(|flag| flag.to_str().unwrap_or(""))
        .collect();
    let well_formed = match command {
        "tokens" | "parse" | "ast" | "rust" => arguments.len() == 2,
        "check" => arguments.len() >= 2 && flags.iter().all(|flag| CHECK_FLAGS.contains(flag)),
        "run" => arguments.len() >= 3,
        _ => false,
    };
    if !well_formed {
        writeln!(
            io::stderr(),
            "error: expected `locus <check|run|rust|tokens|parse|ast> <file.lc>`\nUse `locus --help` for available commands."
        )?;
        return Ok(2);
    }
    let path = &arguments[1];
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            writeln!(
                io::stderr(),
                "error: cannot read {}: {error}",
                path.to_string_lossy()
            )?;
            return Ok(1);
        }
    };
    let mut sources = SourceMap::default();
    let file = sources.add(path.to_string_lossy(), text);
    let source = sources.get(file);
    if command == "tokens" {
        let lexed = lexer::lex(source);
        for token in lexed.tokens {
            writeln!(
                output,
                "{:>6}..{:<6} {:<14} {:?}",
                token.span.start,
                token.span.end,
                format!("{:?}", token.kind),
                source.slice(token.span).unwrap()
            )?;
        }
        output.flush()?;
        emit_diagnostics(&sources, &lexed.diagnostics)?;
        return Ok(u8::from(!lexed.diagnostics.is_empty()));
    }
    let parsed = parser::parse(source);
    if !parsed.is_success() {
        emit_diagnostics(&sources, &parsed.diagnostics)?;
        return Ok(1);
    }
    if matches!(command, "check" | "run" | "rust") {
        // The proofs file beside the source, for `check`: read before
        // elaboration, written after it when it changed, unless `--locked`,
        // under which nothing is searched and nothing is written.
        let options = StoreOptions::from(&flags);
        let store_path = proofs_path(path);
        let store = if command == "check" && options.enabled {
            let store = match fs::read_to_string(&store_path) {
                Ok(text) => match ProofStore::parse(&text) {
                    Ok((store, warnings)) => {
                        for warning in warnings {
                            writeln!(
                                io::stderr(),
                                "warning: {}: {warning}",
                                store_path.to_string_lossy()
                            )?;
                        }
                        store
                    }
                    Err(problem) => {
                        writeln!(
                            io::stderr(),
                            "warning: {}: {problem}; every proof is searched for and the file is rewritten",
                            store_path.to_string_lossy()
                        )?;
                        ProofStore::new()
                    }
                },
                Err(error) if error.kind() == io::ErrorKind::NotFound => ProofStore::new(),
                Err(error) => {
                    writeln!(
                        io::stderr(),
                        "error: cannot read {}: {error}",
                        store_path.to_string_lossy()
                    )?;
                    return Ok(1);
                }
            };
            Some(store.locked(options.locked).searching(options.search))
        } else {
            None
        };
        let (elaborated, store) = match store {
            Some(store) => {
                let (elaborated, store) =
                    elab::elaborate_with_store(source, &parsed.program, store);
                (elaborated, Some(store))
            }
            None => (elab::elaborate(source, &parsed.program), None),
        };
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
                    "proofs file: {} used, {} found and recorded, {} stale, {} searched, {} unwritable",
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
                let (line, column) = source.line_column(hole.span.start).unwrap_or((0, 0));
                writeln!(output, "  {line}:{column} {}{}", hole.tier, pairs_of(hole))?;
            }
        } else if command == "check" && flags.contains(&"--holes") {
            for hole in &elaborated.holes {
                let (line, column) = source.line_column(hole.span.start).unwrap_or((0, 0));
                writeln!(
                    output,
                    "{}:{line}:{column}: {} ({}{}, {} proof nodes, {} us)",
                    source.name,
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
            emit_diagnostics(&sources, &elaborated.diagnostics)?;
            return Ok(1);
        }
        if let Some(store) = &store
            && !options.locked
            && let Some(text) = store.changed()
            && let Err(error) = fs::write(&store_path, text)
        {
            output.flush()?;
            writeln!(
                io::stderr(),
                "error: cannot write {}: {error}",
                store_path.to_string_lossy()
            )?;
            return Ok(1);
        }
        let module = elaborated.session.erased();
        match command {
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

/// `locus build <file.lc>... --out <dir> [--name <crate>]`: checks every
/// file, and writes the crate when all of them pass. The exit status is 2
/// for a usage error, 1 when a file is rejected or cannot be read or
/// written, and 0 when the crate is written.
fn build(arguments: &[OsString]) -> io::Result<u8> {
    let usage = |message: &str| -> io::Result<u8> {
        writeln!(
            io::stderr(),
            "error: {message}\nUse `locus build <file.lc>... --out <dir> [--name <crate>]`."
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
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                writeln!(
                    io::stderr(),
                    "error: cannot read {}: {error}",
                    path.to_string_lossy()
                )?;
                return Ok(1);
            }
        };
        let file = sources.add(path.to_string_lossy(), text);
        let source = sources.get(file);
        let parsed = parser::parse(source);
        if !parsed.is_success() {
            emit_diagnostics(&sources, &parsed.diagnostics)?;
            rejected = true;
            continue;
        }
        let elaborated = elab::elaborate(source, &parsed.program);
        if !elaborated.is_success() {
            emit_diagnostics(&sources, &elaborated.diagnostics)?;
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
            writeln!(
                io::stderr(),
                "error: cannot write the crate under {}: {error}",
                out.display()
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

fn emit_diagnostics(sources: &SourceMap, diagnostics: &[Diagnostic]) -> io::Result<()> {
    let mut error_output = io::stderr().lock();
    let color = error_output.is_terminal() && env::var_os("NO_COLOR").is_none();
    for diagnostic in diagnostics {
        writeln!(error_output, "{}", diagnostic.render(sources, color))?;
    }
    Ok(())
}
