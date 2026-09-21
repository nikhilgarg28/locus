use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use locus::diagnostic::Diagnostic;
use locus::elab;
use locus::erased::{Interpreter, Value, print_module};
use locus::lexer;
use locus::parser;
use locus::source::SourceMap;

const HELP: &str = "Locus

Usage: locus <command> <file.loc> [arguments]

  check   Check types and proofs; --holes lists every `_` and how it was filled
  run     Check, then interpret a function: locus run <file.loc> <function> [u8|true|false]...
  rust    Check, then print the generated Rust
  tokens  Print tokens and their original source spans
  parse   Validate syntax only
  ast     Print the syntax tree of a syntactically valid file

  -h, --help     Show this help
  -V, --version  Show the version
";

/// Steps the interpreter may take before it reports that it ran out.
const FUEL: u64 = 10_000_000;

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
    let well_formed = match command {
        "tokens" | "parse" | "ast" | "rust" => arguments.len() == 2,
        "check" => arguments.len() == 2 || (arguments.len() == 3 && arguments[2] == "--holes"),
        "run" => arguments.len() >= 3,
        _ => false,
    };
    if !well_formed {
        writeln!(
            io::stderr(),
            "error: expected `locus <check|run|rust|tokens|parse|ast> <file.loc>`\nUse `locus --help` for available commands."
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
        let elaborated = elab::elaborate(source, &parsed.program);
        if command == "check" && arguments.len() == 3 {
            for hole in &elaborated.holes {
                let (line, column) = source.line_column(hole.span.start).unwrap_or((0, 0));
                writeln!(
                    output,
                    "{}:{line}:{column}: {} ({}, {} bytes of proof, {} us)",
                    source.name,
                    if hole.solved { "filled" } else { "unsolved" },
                    hole.tier,
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
        let module = elaborated.session.erased();
        match command {
            "check" => writeln!(
                output,
                "Checked {} function(s); {} proof(s) found and accepted by the kernel.",
                elaborated.functions.len(),
                elaborated.holes.len()
            )?,
            "rust" => write!(output, "{}", print_module(module))?,
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
                let mut values = Vec::new();
                for argument in &arguments[3..] {
                    let text = argument.to_string_lossy();
                    values.push(match (text.as_ref(), text.parse::<u8>()) {
                        ("true", _) => Value::Bool(true),
                        ("false", _) => Value::Bool(false),
                        (_, Ok(byte)) => Value::U8(byte),
                        _ => {
                            writeln!(
                                io::stderr(),
                                "error: `{text}` is not a `u8` or a `bool`; other arguments cannot be given on the command line"
                            )?;
                            return Ok(2);
                        }
                    });
                }
                match Interpreter::new(module, FUEL).call(function, values) {
                    Ok(value) => writeln!(output, "{}", value.debug(module))?,
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

fn emit_diagnostics(sources: &SourceMap, diagnostics: &[Diagnostic]) -> io::Result<()> {
    let mut error_output = io::stderr().lock();
    let color = error_output.is_terminal() && env::var_os("NO_COLOR").is_none();
    for diagnostic in diagnostics {
        writeln!(error_output, "{}", diagnostic.render(sources, color))?;
    }
    Ok(())
}
