//! Reproducible raw observations; no timing is a logical acceptance criterion.
use crate::{
    elab, measurement,
    source::{SourceBundle, SourceMap},
    store::{Lockfile, ProofStore},
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt::Write,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

mod build {
    include!(concat!(env!("OUT_DIR"), "/bench_build.rs"));
}

fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c <= '\u{1f}' => {
                write!(out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn stages(samples: &[measurement::Sample]) -> String {
    let mut totals = BTreeMap::<&str, u128>::new();
    for sample in samples {
        *totals.entry(sample.stage).or_default() += sample.nanos;
    }
    format!(
        "{{{}}}",
        totals
            .iter()
            .map(|(stage, nanos)| format!("{}:{nanos}", quote(stage)))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn one(
    path: &Path,
    libraries: &[OsString],
    options: &elab::Options,
    pass: &str,
    ordinal: usize,
    store: ProofStore,
) -> Result<(String, ProofStore), String> {
    let mut sources = SourceMap::default();
    let mut files = Vec::new();
    let mut source_bytes = 0usize;
    for path in libraries
        .iter()
        .map(PathBuf::from)
        .chain(std::iter::once(path.to_path_buf()))
    {
        let mut text = String::new();
        let input =
            fs::File::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let limit = crate::limits::MAX_SOURCE_BYTES;
        let oversized = || {
            format!(
                "{}: L0010: MAX_SOURCE_BYTES limit of {limit} was exceeded",
                path.display()
            )
        };
        if input
            .metadata()
            .map_err(|e| e.to_string())?
            .len()
            .saturating_add(source_bytes as u64)
            > limit as u64
        {
            return Err(oversized());
        }
        input
            .take((limit - source_bytes) as u64 + 1)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        source_bytes = source_bytes.saturating_add(text.len());
        if source_bytes > limit {
            return Err(oversized());
        }
        // SourceBundle inserts a separator after each input, including the last.
        source_bytes = source_bytes.saturating_add(1);
        let file = sources.add(path.to_string_lossy(), text);
        let parsed = crate::parser::parse(sources.get(file));
        if !parsed.is_success() {
            return Err(parsed
                .diagnostics
                .iter()
                .map(|d| d.render(&sources, false))
                .collect::<Vec<_>>()
                .join("\n"));
        }
        files.push(file);
    }
    let bundle = SourceBundle::join(&mut sources, &files);
    let source = sources.get(bundle.file);
    let started = Instant::now();
    let parsed = crate::parser::parse(source);
    let parse_ns = started.elapsed().as_nanos();
    if !parsed.is_success() {
        return Err(parsed
            .diagnostics
            .iter()
            .map(|d| bundle.diagnostic(d).render(&sources, false))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let started = Instant::now();
    let ((checked, store), samples) = measurement::capture(|| {
        elab::elaborate_with_store_and_options(source, &parsed.program, store, options)
    });
    let elaborate_total_ns = started.elapsed().as_nanos();
    if !checked.is_success() {
        return Err(checked
            .diagnostics
            .iter()
            .map(|d| bundle.diagnostic(d).render(&sources, false))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let total = |name| {
        samples
            .iter()
            .filter(|s| s.stage == name)
            .map(|s| s.nanos)
            .sum::<u128>()
    };
    let lowering_ns = total("lowering");
    let checking_ns = total("checking");
    let erasure_ns = total("erasure");
    let frontend_ns = elaborate_total_ns.saturating_sub(lowering_ns + checking_ns + erasure_ns);
    let obligations=checked.holes.iter().enumerate().map(|(index,hole)| {
        let span=bundle.span(hole.span);
        let certificate_bytes=hole.found.as_ref().and_then(|found|store.names().and_then(|names|crate::store::text::print_proof(&found.proof,&found.context,names).ok())).map_or("null".into(),|text|text.len().to_string());
        let kernel_recheck_ns=hole.found.as_ref().map_or(0,|found| {
            let mut context=found.context.clone();
            let started=Instant::now();
            crate::kernel::check_proof(&mut context,&found.proof,&found.claim).expect("benchmark only observes already checked evidence");
            started.elapsed().as_nanos()
        });
        format!("{{\"ordinal\":{index},\"file\":{},\"byte_start\":{},\"tier\":{},\"search_ns_by_tier\":{},\"proof_nodes\":{},\"kernel_recheck_ns\":{kernel_recheck_ns},\"certificate_bytes\":{certificate_bytes},\"store_hit\":{}}}",quote(&sources.get(span.file).name),span.start,quote(hole.tier),stages(&hole.measurements.iter().filter(|s|s.stage!="kernel").cloned().collect::<Vec<_>>()),hole.proof_size,hole.tier=="stored")
    }).collect::<Vec<_>>().join(",");
    let stats = store.stats();
    let text = format!(
        "{{\"workload\":{},\"pass\":{},\"sample\":{ordinal},\"parse_ns\":{parse_ns},\"elaboration_ns\":{frontend_ns},\"lowering_ns\":{lowering_ns},\"checking_ns\":{checking_ns},\"erasure_ns\":{erasure_ns},\"total_ns\":{},\"store_hits\":{},\"store_searches\":{},\"certificate_bytes\":{},\"obligations\":[{obligations}]}}",
        quote(&path.to_string_lossy()),
        quote(pass),
        parse_ns + elaborate_total_ns,
        stats.hits,
        stats.searches,
        store.render(&path.to_string_lossy()).len()
    );
    Ok((text, store))
}

/// `bench [file-or-directory ...] [--samples N] [--no-record]`.
/// With no path, the target corpus is the pinned initial workload set.
pub fn command(
    arguments: &[OsString],
    options: &elab::Options,
    libraries: &[OsString],
) -> Result<String, String> {
    let mut paths = Vec::new();
    let mut samples = 5;
    let mut record = true;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--samples") => {
                samples = arguments
                    .next()
                    .and_then(|a| a.to_str())
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|n| *n > 0)
                    .ok_or("--samples requires a positive integer")?;
            }
            Some("--no-record") => record = false,
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unknown bench option {flag}"));
            }
            _ => paths.push(PathBuf::from(argument)),
        }
    }
    if paths.is_empty() {
        paths.push(PathBuf::from("tests/corpus/target"));
    }
    let paths = crate::audit::source_paths(&paths).map_err(|e| e.to_string())?;
    let mut observations = Vec::new();
    for path in &paths {
        for ordinal in 0..samples {
            let (cold, store) = one(
                path,
                libraries,
                options,
                "search",
                ordinal,
                ProofStore::new(),
            )?;
            let name = path.to_string_lossy();
            let (mut lockfile, warnings) = Lockfile::parse(&store.render(&name))?;
            if !warnings.is_empty() {
                return Err(format!(
                    "benchmark emitted an invalid lockfile: {}",
                    warnings.join("; ")
                ));
            }
            let replay = lockfile.take(&name);
            let (warm, _) = one(
                path,
                libraries,
                options,
                "replay",
                ordinal,
                replay.locked(true),
            )?;
            observations.extend([cold, warm]);
        }
    }
    let files = paths
        .iter()
        .map(|p| quote(&p.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(",");
    let libs = libraries
        .iter()
        .map(|p| quote(&p.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(",");
    let previews = options
        .previews
        .iter()
        .map(|feature| quote(feature.name()))
        .collect::<Vec<_>>()
        .join(",");
    let configuration = format!(
        "{{\"check_moves\":{},\"previews\":[{previews}]}}",
        options.check_moves
    );
    let build_fields = [
        ("toolchain", build::TOOLCHAIN),
        ("profile", build::PROFILE),
        ("opt_level", build::OPT_LEVEL),
        ("debug", build::DEBUG),
        ("target", build::TARGET),
        ("host", build::HOST),
        ("rustflags", build::CARGO_ENCODED_RUSTFLAGS),
        ("rustc_wrapper", build::RUSTC_WRAPPER),
        ("rustc_workspace_wrapper", build::RUSTC_WORKSPACE_WRAPPER),
        ("source_git_blob", build::SOURCE_GIT_BLOB),
    ];
    let compiler_build = format!(
        "{{{}}}",
        build_fields
            .iter()
            .map(|(key, value)| format!("{}:{}", quote(key), quote(value)))
            .collect::<Vec<_>>()
            .join(",")
    );
    let executable = quote(
        &std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy(),
    );
    let raw = format!(
        "{{\"schema_version\":1,\"unit\":\"nanoseconds\",\"configuration\":{configuration},\"compiler_build\":{compiler_build},\"executable\":{executable},\"debug_assertions\":{},\"workloads\":[{files}],\"libraries\":[{libs}],\"samples\":[{}]}}",
        cfg!(debug_assertions),
        observations.join(",")
    );
    if !record {
        return Ok(raw);
    }
    use std::{
        io::Write as _,
        process::{Command, Stdio},
    };
    let mut child = Command::new("python3")
        .args(["-c", include_str!("../tools/bench.py"), "record"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(raw.as_bytes())
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim_end().into())
        .map_err(|e| e.to_string())
}
