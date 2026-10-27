//! Repository tasks for Locus, run as `cargo xtask <command>`.
//!
//! The checks here enforce the structure that the design process depends on:
//! litmus examples, spec rules and their coverage and provenance, RFC
//! metadata, trust entries, issues, error-code pages, and links. The site builder
//! renders the same sources as HTML, and `context` gathers them for one rule.

mod check;
mod context;
mod diagnostics;
mod issues;
mod lexer;
mod litmus;
mod markdown;
mod pr;
mod repo;
mod report;
mod rfc;
mod site;
mod spec;
mod trust;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  check                       check litmus examples, spec rules, RFCs, issues, diagnostics and links
  context RULE...             what the repository says about a rule, or every rule under a prefix
  site [--out DIR]            build the website (default: target/site)
  pr-check --base REV [--allow-expectation-changes]
                              check a branch against REV: design and build changes kept
                              apart, no decided expectation changed or removed, and no
                              issue deleted, renamed, or its log rewritten";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = repo_root();
    let result = match args.first().map(String::as_str) {
        Some("check") if args.len() == 1 => check::run(&root),
        Some("context") if args.len() > 1 => context::run(&root, &args[1..]),
        Some("site") => parse_site_args(&args[1..]).and_then(|out| site::run(&root, &out)),
        Some("pr-check") => parse_pr_args(&args[1..]).and_then(|a| pr::run(&root, &a)),
        Some("help" | "-h" | "--help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repository root")
        .to_path_buf()
}

fn parse_site_args(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [] => Ok(repo_root().join("target/site")),
        // Absolute, because mdBook resolves relative paths against the book.
        [flag, dir] if flag == "--out" => std::env::current_dir()
            .map(|cwd| cwd.join(dir))
            .map_err(|e| format!("cannot read the current directory: {e}")),
        _ => Err(USAGE.to_string()),
    }
}

fn parse_pr_args(args: &[String]) -> Result<pr::Args, String> {
    let mut base = None;
    let mut allow_expectation_changes = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--base" => base = rest.next().cloned(),
            "--allow-expectation-changes" => allow_expectation_changes = true,
            _ => return Err(USAGE.to_string()),
        }
    }
    let base = base.ok_or_else(|| USAGE.to_string())?;
    Ok(pr::Args {
        base,
        allow_expectation_changes,
    })
}
