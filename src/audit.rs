//! Human-readable trust/effect inventory. This is reporting, never a proof rule.
use crate::{
    elab::Elaborated,
    exec::{Block, Stmt, Tail},
    kernel::{Definitions, Term, proof_is_classical},
    typed::FnRef,
};
use std::fmt::Write;

pub fn render(program: &Elaborated) -> String {
    let mut entries = Vec::new();
    let definitions = program.session.program().definitions();
    for (name, reference) in &program.functions {
        match reference {
            FnRef::Math(id) => {
                if definitions.is_classical(*id) {
                    entries.push(format!("classical {name}: depends on excluded middle"));
                }
            }
            FnRef::Exec(id) => {
                if let Some(function) = program.session.program().function(*id) {
                    if !function.promises.terminates {
                        entries.push(format!("divergence {name}: termination is not promised"));
                    }
                    walk(&function.body, name, definitions, &mut entries);
                }
            }
        }
    }
    for contract in program.session.program().trusted_contracts() {
        let name = program
            .functions
            .iter()
            .find(|(_, reference)| *reference == FnRef::Exec(contract.function))
            .map(|(name, _)| name.as_str())
            .unwrap_or("<foreign>");
        entries.push(format!(
            "trusted {name} = {}: {}",
            contract.implementation,
            quote(&contract.reason)
        ));
    }
    for function in program.session.buffer_functions() {
        entries.push(format!(
            "native {} {:?}/{:?}: {}",
            function.name,
            function.storage,
            function.op,
            quote(&function.reason)
        ));
        if let FnRef::Exec(id) = function.reference
            && let Some(native) = program.session.program().function(id)
        {
            walk(&native.body, &function.name, definitions, &mut entries);
        }
    }
    let (mut theory, prelude) = Definitions::with_prelude();
    let names =
        crate::kernel::theory::declare(&mut theory, &prelude).expect("the standard theory checks");
    for (name, id) in names.lemma_names() {
        if theory.is_classical(id) {
            entries.push(format!(
                "classical theory::{name}: depends on excluded middle"
            ));
        }
    }
    entries.sort();
    entries.dedup();
    let mut text = String::from(
        "Locus audit v1\nContracts apply on normal return; trusted specifications and effects require review.\n",
    );
    for entry in &entries {
        let _ = writeln!(text, "{entry}");
    }
    let _ = writeln!(text, "{} review item(s)", entries.len());
    text
}

fn quote(text: &str) -> String {
    format!("{text:?}")
}
fn term(term: &Term, site: &str, definitions: &Definitions, entries: &mut Vec<String>) {
    if term
        .find(&|term| match term {
            Term::Proof(proof) | Term::Absurd(proof, _) => proof_is_classical(definitions, proof),
            Term::Fn(id) => definitions.is_classical(*id),
            _ => false,
        })
        .is_some()
    {
        entries.push(format!("classical {site}: depends on excluded middle"));
    }
}
fn walk(block: &Block, path: &str, definitions: &Definitions, entries: &mut Vec<String>) {
    for (index, statement) in block.stmts.iter().enumerate() {
        let site = format!("{path}/statement[{}]", index + 1);
        match statement {
            Stmt::Let { value, .. } => term(value, &site, definitions, entries),
            Stmt::Have { proof, .. } => {
                if proof_is_classical(definitions, proof) {
                    entries.push(format!("classical {site}: depends on excluded middle"));
                }
            }
            Stmt::Call { arguments, .. } => {
                for argument in arguments {
                    term(argument, &site, definitions, entries);
                }
            }
            Stmt::Match {
                scrutinee, arms, ..
            } => {
                term(scrutinee, &site, definitions, entries);
                for (n, arm) in arms.iter().enumerate() {
                    walk(&arm.body, &format!("{site}/arm[{n}]"), definitions, entries);
                }
            }
            Stmt::Loop { body, .. } => walk(body, &format!("{site}/loop"), definitions, entries),
            Stmt::For(looped) => walk(&looped.body, &format!("{site}/for"), definitions, entries),
            Stmt::Operate(operation) => {
                if operation.fits.is_none() {
                    entries.push(format!(
                        "panic {site}: {}[{}] has no no-panic evidence",
                        operation.op.name(),
                        operation.ty.name()
                    ));
                }
                if let Some(proofs) = &operation.fits
                    && proofs.iter().any(|p| proof_is_classical(definitions, p))
                {
                    entries.push(format!(
                        "classical {site}: operator evidence depends on excluded middle"
                    ));
                }
            }
            Stmt::BoxNew { .. } => entries.push(format!(
                "panic {site}: box allocation failure may prevent return"
            )),
            Stmt::Buffer(operation) => {
                if operation.op == crate::kernel::BufferOp::Push
                    || (operation.op == crate::kernel::BufferOp::Literal
                        && operation.storage == crate::exec::BufferStorage::Vector
                        && !operation.arguments.is_empty())
                {
                    entries.push(format!(
                        "panic {site}: allocation or capacity failure may prevent return"
                    ));
                }
            }
        }
    }
    let site = format!("{path}/end");
    match &block.tail {
        Tail::Value(value) | Tail::Return(value) | Tail::Break(value) => {
            term(value, &site, definitions, entries)
        }
        Tail::Continue(values) => {
            for value in values {
                term(value, &site, definitions, entries);
            }
        }
        Tail::Match { scrutinee, arms } => {
            term(scrutinee, &site, definitions, entries);
            for (n, arm) in arms.iter().enumerate() {
                walk(&arm.body, &format!("{site}/arm[{n}]"), definitions, entries);
            }
        }
        Tail::Panic {
            message,
            unreachable,
        } => {
            if let Some(proof) = unreachable {
                if proof_is_classical(definitions, proof) {
                    entries.push(format!(
                        "classical {site}: unreachable evidence depends on excluded middle"
                    ));
                }
            } else {
                entries.push(format!("panic {site}: {}", quote(message)));
            }
        }
    }
}

/// Expand a crate directory or an explicit file list deterministically. Each
/// source is a compilation unit, as in `build`; module resolution is unchanged.
/// Do not follow symlinks out of the requested tree or audit generated targets.
pub fn source_paths(inputs: &[std::path::PathBuf]) -> std::io::Result<Vec<std::path::PathBuf>> {
    use std::{io, path::Path};
    fn visit(path: &Path, files: &mut Vec<std::path::PathBuf>) -> io::Result<()> {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Ok(());
        }
        if metadata.is_dir() {
            let mut children = std::fs::read_dir(path)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<io::Result<Vec<_>>>()?;
            children.sort();
            for child in children {
                if child
                    .file_name()
                    .is_some_and(|name| name == "target" || name == ".git")
                {
                    continue;
                }
                visit(&child, files)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "lc") {
            files.push(path.to_path_buf());
        }
        Ok(())
    }
    let mut paths = Vec::new();
    for input in inputs {
        visit(input, &mut paths)?;
    }
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "audit input contains no .lc sources",
        ));
    }
    Ok(paths)
}
