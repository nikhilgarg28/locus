use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use locus::diagnostic::Diagnostic;
use locus::lexer;
use locus::parser;
use locus::source::SourceMap;

const HELP: &str = "Locus syntax frontend

Usage: locus <tokens|parse|ast> <file.loc>

  tokens  Print tokens and their original source spans
  parse   Validate syntax (does not check types or proofs)
  ast     Print the syntax tree of a syntactically valid file

  -h, --help     Show this help
  -V, --version  Show the version

Semantic checking, proof checking, and execution are not implemented yet.
";

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
    if !matches!(command, "tokens" | "parse" | "ast") || arguments.len() != 2 {
        writeln!(
            io::stderr(),
            "error: expected `locus <tokens|parse|ast> <file.loc>`\nUse `locus --help` for available commands."
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
    if command == "ast" {
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
