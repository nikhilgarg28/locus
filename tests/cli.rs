use std::path::{Path, PathBuf};
use std::process::Command;

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

#[test]
fn parse_reports_syntax_only_and_ast_is_available() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("parse")
        .arg(example("increment.lc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Parsed 2 declaration(s)"));
    assert!(stdout.contains("types and proofs have not been checked"));
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("ast")
        .arg(example("preserve.lc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("Hole"));
}

#[test]
fn tokens_have_spans_and_unsupported_commands_fail() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("tokens")
        .arg(example("increment.lc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("At"));
    assert!(stdout.contains("Underscore"));
    assert!(!stdout.contains("Hash"));
    assert!(stdout.contains("Eof"));
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("compile")
        .arg(example("increment.lc"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn missing_files_are_reported_without_panicking() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .args(["parse", "does-not-exist.lc"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot read")
    );
}

#[test]
#[doc = "spec: 1.22:3"]
fn a_file_is_checked_run_and_printed_as_rust() {
    let locus = || Command::new(env!("CARGO_BIN_EXE_locus"));
    let output = locus()
        .arg("check")
        .arg(example("lock.lc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Checked 6 function(s); 8 proof(s) found and accepted by the kernel.")
    );
    // Without the proofs file, so that the tiers themselves are seen.
    let output = locus()
        .arg("check")
        .arg(example("lock.lc"))
        .arg("--holes")
        .env("LOCUS_PROOFS", "off")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("filled (computed,"), "{stdout}");
    assert!(stdout.contains("filled (evaluation,"), "{stdout}");
    assert!(!stdout.contains("256"), "{stdout}");
    let output = locus()
        .arg("run")
        .arg(example("lock.lc"))
        .args(["attempts_left", "2", "9"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "1\n");
    let output = locus()
        .arg("rust")
        .arg(example("lock.lc"))
        .output()
        .unwrap();
    // Nothing in the example is marked `pub`, so nothing is exported: the
    // functions are printed as written, private.
    let rust = String::from_utf8(output.stdout).unwrap();
    assert!(
        rust.contains("\nfn attempts_left(attempts: u8, correct: u8) -> u8 {"),
        "{rust}"
    );
    assert!(!rust.contains("pub fn"), "{rust}");
    let output = locus()
        .arg("run")
        .arg(example("lock.lc"))
        .args(["no_such_function"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
}

#[test]
#[doc = "spec: 1.19:1"]
fn build_writes_a_crate_that_compiles_and_is_the_same_twice() {
    let locus = || Command::new(env!("CARGO_BIN_EXE_locus"));
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli_build_lock");
    let build = || {
        let output = locus()
            .arg("build")
            .arg(example("lock.lc"))
            .arg(example("increment.lc"))
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("Wrote the crate `cli_build_lock`, 2 module(s), under"),
            "{stdout}"
        );
        [
            "Cargo.toml",
            "src/lib.rs",
            "src/increment.rs",
            "src/lock.rs",
        ]
        .map(|path| std::fs::read_to_string(out.join(path)).unwrap())
    };
    let first = build();
    assert!(
        first[1].contains("pub mod increment;\n\npub mod lock;\n"),
        "{}",
        first[1]
    );
    assert!(
        first[3].contains("\nfn attempts_left(attempts: u8, correct: u8) -> u8 {"),
        "{}",
        first[3]
    );
    // The crate compiles as a library under `-D warnings`, with the edition
    // its manifest names.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let output = Command::new(rustc)
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "-D",
            "warnings",
            "-o",
        ])
        .arg(out.join("libcli_build_lock.rlib"))
        .arg(out.join("src/lib.rs"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(build(), first);
    // Usage errors, and a file that is refused, write no crate.
    let output = locus()
        .arg("build")
        .arg(example("lock.lc"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("`--out <dir>` says where the crate goes")
    );
    let refused = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli_build_refused");
    let output = locus()
        .arg("build")
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/corpus/reject/pub_evidence_fn.lc"),
        )
        .arg("--out")
        .arg(&refused)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("error[L0244]")
    );
    assert!(!refused.exists());
    // A file whose name is no module name.
    let odd = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("Odd-Name.lc");
    std::fs::write(&odd, "fn f() -> u8 { 1 }\n").unwrap();
    let output = locus()
        .arg("build")
        .arg(&odd)
        .arg("--out")
        .arg(&refused)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("a module's name is a lower-case identifier")
    );
}

/// The `--holes` listing without its timings, which are the one thing in it
/// that may differ between runs.
fn holes_without_timings(stdout: &str) -> String {
    stdout
        .lines()
        .map(|line| match line.rfind(", ") {
            Some(cut) if line.ends_with(" us)") => format!("{})", &line[..cut]),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn checking_is_deterministic() {
    // Which tier fills each hole, and how large the proof is, is a function
    // of the file alone: two runs over every example agree byte for byte.
    // Without the proofs file, which would make every second run `stored`.
    let examples = std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "lc"));
    let mut seen = 0;
    for path in examples {
        let run = || {
            let output = Command::new(env!("CARGO_BIN_EXE_locus"))
                .arg("check")
                .arg(&path)
                .arg("--holes")
                .env("LOCUS_PROOFS", "off")
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", path.display());
            holes_without_timings(&String::from_utf8(output.stdout).unwrap())
        };
        let (first, second) = (run(), run());
        assert_eq!(first, second, "{}", path.display());
        assert!(first.contains("filled ("), "{}: {first}", path.display());
        seen += 1;
    }
    assert!(seen >= 5);
}

/// The lines of `--stats` that do not carry a timing: the counts of the
/// obligations by tier, and the list of them.
fn stats_without_timings(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .skip_while(|line| !line.starts_with("obligations:"))
        .map(str::to_string)
        .collect()
}

#[test]
#[doc = "spec: 1.8:1, 1.8:2"]
fn stats_count_the_obligations_by_tier() {
    // The target examples of the atlas predict, over the whole 32-bit lock
    // and midpoint, three exact, four computed, and seven arithmetic
    // obligations. What the tiers count differs, and the difference is
    // recorded rather than adjusted: `0 <= 3` and `2 != 0` on literals are
    // decided by evaluation, which the prediction folds into computed;
    // and an overflow row of the table has two premises, `min <= e` and
    // `e <= max`, where the prediction counts the operator once, so each
    // of the four operators `+` and `-` adds an arithmetic obligation for
    // the bound the ranges of the views give.
    let corpus = |directory: &str, name: &str| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus")
            .join(directory)
            .join(name)
    };
    let stats = |path: PathBuf| {
        let output = Command::new(env!("CARGO_BIN_EXE_locus"))
            .arg("check")
            .arg(&path)
            .arg("--stats")
            // The tiers themselves, not the proofs file.
            .env("LOCUS_PROOFS", "off")
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", path.display());
        stats_without_timings(&String::from_utf8(output.stdout).unwrap())
    };
    assert_eq!(
        stats(corpus("target", "midpoint.lc")),
        [
            "obligations: 6 (1 evaluation, 5 arithmetic)",
            "  5:20 arithmetic, 1 pairs",
            "  5:20 arithmetic, 2 pairs",
            "  5:26 evaluation",
            "  6:18 arithmetic, 2 pairs",
            "  6:18 arithmetic, 8 pairs",
            "  7:11 arithmetic, 18 pairs",
            "Checked 1 function(s); 6 proof(s) found and accepted by the kernel.",
        ]
    );
    assert_eq!(
        stats(corpus("accept", "lock32_step.lc")),
        [
            "obligations: 9 (2 computed, 1 evaluation, 6 arithmetic)",
            "  33:40 evaluation",
            "  37:28 arithmetic, 1 pairs",
            "  38:59 arithmetic, 1 pairs",
            "  38:59 computed",
            "  39:44 arithmetic, 4 pairs",
            "  41:65 computed",
            "  50:18 arithmetic, 1 pairs",
            "  50:18 arithmetic, 1 pairs",
            "  51:12 arithmetic, 4 pairs",
            "Checked 3 function(s); 9 proof(s) found and accepted by the kernel.",
        ]
    );
}

/// A scratch directory of its own for a test that writes files.
fn scratch(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

/// `locus check <source> <flags>` under `env`: the exit code, stdout, and
/// stderr.
fn check(source: &Path, flags: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_locus"));
    command.arg("check").arg(source).args(flags);
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn check_stores_the_proofs_in_a_lockfile_and_locked_never_searches() {
    let directory = scratch("cli_store");
    let source = directory.join("lock.lc");
    std::fs::copy(example("lock.lc"), &source).unwrap();
    let lock = directory.join("Locus.lock");
    let check_lock = |flags: &[&str], env: &[(&str, &str)]| check(&source, flags, env);

    // `--locked` with no file: an error naming the function, the line, and
    // the claim, and no file is written.
    let (code, _, stderr) = check_lock(&["--locked"], &[]);
    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("`step` needs a proof of `lock.failures < 3` at line 33"),
        "{stderr}"
    );
    assert!(stderr.contains("the proofs file has none"), "{stderr}");
    assert!(!lock.exists());

    // A plain check writes the file, in the file's directory; a second run
    // uses every entry, searches for nothing, and leaves the bytes as they
    // are.
    let (code, stdout, _) = check_lock(&["--stats"], &[]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(
        stdout.contains("Locus.lock: 0 used, 8 found and recorded, 0 stale, 8 searched"),
        "{stdout}"
    );
    let written = read(&lock);
    assert!(
        written.starts_with(
            "version = 2\n\n[[file]]\npath = \"lock.lc\"\n\n  [[file.obligation]]\n  key = \""
        ),
        "{written}"
    );
    assert_eq!(
        written.matches("\n  [[file.obligation]]\n").count(),
        8,
        "{written}"
    );
    assert_eq!(written.matches("\n[[file]]\n").count(), 1, "{written}");
    assert_eq!(written.matches("\n  claim = \"").count(), 8, "{written}");
    assert_eq!(written.matches("\n  steps = '''\n").count(), 8, "{written}");
    assert!(written.contains("\n  at = \"run 2\"\n"), "{written}");
    assert!(written.contains("\ns1 = "), "{written}");
    let (code, stdout, _) = check_lock(&["--stats", "--locked"], &[]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(
        stdout.contains("Locus.lock: 8 used, 0 found and recorded, 0 stale, 0 searched"),
        "{stdout}"
    );
    assert!(stdout.contains("obligations: 8 (8 stored)"), "{stdout}");
    assert_eq!(read(&lock), written);
    let (code, stdout, _) = check_lock(&["--holes"], &[]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout.matches("filled (stored,").count(), 8, "{stdout}");
    assert_eq!(read(&lock), written);

    // Surviving an upgrade: with the search disabled altogether, the file
    // still checks; without an entry, nothing does, and no entry is
    // written for the file that failed.
    let (code, stdout, _) = check_lock(&["--stats"], &[("LOCUS_SEARCH", "none")]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(stdout.contains("0 searched"), "{stdout}");
    let fresh = directory.join("fresh.lc");
    std::fs::copy(example("lock.lc"), &fresh).unwrap();
    let (code, _, stderr) = check(&fresh, &[], &[("LOCUS_SEARCH", "none")]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("cannot show"), "{stderr}");
    assert_eq!(read(&lock), written);

    // `--no-store` and `LOCUS_PROOFS=off` neither read nor write.
    let (code, stdout, _) = check_lock(&["--holes", "--no-store"], &[]);
    assert_eq!(code, Some(0));
    assert!(!stdout.contains("stored"), "{stdout}");
    let (code, stdout, _) = check_lock(&["--holes"], &[("LOCUS_PROOFS", "off")]);
    assert_eq!(code, Some(0));
    assert!(!stdout.contains("stored"), "{stdout}");
    assert_eq!(read(&lock), written);

    // A second file in the directory gets a table of its own, in path
    // order, and each file's check leaves the other's entries as they are.
    let other = directory.join("increment.lc");
    std::fs::copy(example("increment.lc"), &other).unwrap();
    let (code, stdout, _) = check(&other, &["--stats"], &[]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(
        stdout.contains("Locus.lock: 0 used, 1 found and recorded, 0 stale, 1 searched"),
        "{stdout}"
    );
    let both = read(&lock);
    assert_eq!(both.matches("\n[[file]]\n").count(), 2, "{both}");
    assert!(
        both.starts_with("version = 2\n\n[[file]]\npath = \"increment.lc\"\n"),
        "{both}"
    );
    assert!(both.ends_with(&written["version = 2\n".len()..]), "{both}");
    let (code, stdout, _) = check_lock(&["--stats", "--locked"], &[]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(stdout.contains("Locus.lock: 8 used, "), "{stdout}");
    assert_eq!(read(&lock), both);
    let (code, stdout, _) = check(&other, &["--stats", "--locked"], &[]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(stdout.contains("Locus.lock: 1 used, "), "{stdout}");
    assert_eq!(read(&lock), both);
    std::fs::write(&lock, &written).unwrap();

    // A stale entry: the file is a hint. The keys of two entries swapped,
    // each is a proof of the other's claim: the kernel refuses both, they
    // are searched for again, and the file is rewritten; under `--locked`
    // they are errors, which say what the entry proves and what is wanted.
    let keys: Vec<&str> = written
        .lines()
        .filter(|line| line.starts_with("  key = "))
        .collect();
    assert_eq!(keys.len(), 8);
    let swapped = written
        .replacen(keys[1], "  key = SWAP", 1)
        .replacen(keys[2], keys[1], 1)
        .replacen("  key = SWAP", keys[2], 1);
    assert_ne!(swapped, written);
    std::fs::write(&lock, &swapped).unwrap();
    let (code, _, stderr) = check_lock(&["--locked"], &[]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("the proofs file has none"), "{stderr}");
    assert!(
        stderr.contains("the stored proof concludes `")
            && stderr.contains("the obligation wants `"),
        "{stderr}"
    );
    let (code, stdout, _) = check_lock(&["--stats"], &[]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(
        stdout.contains("Locus.lock: 6 used, 2 found and recorded, 2 stale, 2 searched"),
        "{stdout}"
    );
    assert_eq!(read(&lock), written);

    // A file that is not a lockfile at all is reported and replaced; one
    // of another version is refused; an entry that does not read is
    // skipped with a warning and the rest are used.
    std::fs::write(&lock, "not a lockfile\n").unwrap();
    let (code, _, stderr) = check_lock(&[], &[]);
    assert_eq!(code, Some(0));
    assert!(stderr.contains("not a TOML lockfile"), "{stderr}");
    assert!(
        stderr.contains("every proof of `lock.lc` is searched for"),
        "{stderr}"
    );
    assert_eq!(read(&lock), written);
    std::fs::write(&lock, "version = 7\n").unwrap();
    let (code, _, stderr) = check_lock(&["--locked"], &[]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("version 7"), "{stderr}");
    let damaged = written.replacen("  key = \"", "  key = \"x", 1);
    std::fs::write(&lock, &damaged).unwrap();
    let (code, stdout, stderr) = check_lock(&["--stats"], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stderr.contains("obligation 1: the key is not sixteen hex digits; skipped"),
        "{stderr}"
    );
    assert!(
        stdout.contains("Locus.lock: 7 used, 1 found and recorded, 0 stale, 1 searched"),
        "{stdout}"
    );
    assert_eq!(read(&lock), written);
}

/// The proofs of `examples/lock.lc` and `examples/increment.lc` as version
/// 1 wrote them, beside the source.
const LOCK_V1: &str = r#"locus-proofs 1

obligation 35247a4dabb8c29f remaining 1
transport(transport(literal(view[u8](3)), (#0 ==[Int] view[u8](3)), refl(view[u8](3))), int_le(view[u8]($0), #0), implies_elim(axiom(cmp_reflect[true], int_le_b(view[u8]($0), 3i)), of_term(proof(transport(definition(fn:within_limit(view[u8]($0))), #0, of_term($1))))))

obligation b8664f10c935e577 remaining 2
implies_elim(axiom(cmp_reify[true], int_le_b(view[u8](wrapping_sub[u8](3, $0)), 3i)), transport(literal(view[u8](3)), int_le(view[u8](wrapping_sub[u8](3, $0)), #0), of_term(proof(of_term(fn:u8_sub_le(3, $0, proof(transport(transport(literal(view[u8](3)), (#0 ==[Int] view[u8](3)), refl(view[u8](3))), int_le(view[u8]($0), #0), implies_elim(axiom(cmp_reflect[true], int_le_b(view[u8]($0), 3i)), of_term(proof(transport(definition(fn:within_limit(view[u8]($0))), #0, of_term($1)))))))))))))

obligation caccac166ef12c6d run 1
evaluate(int_le_b(view[u8](0), 3i))

obligation f33fdca3e029212e run 2
transport(transport(h4, (#0 ==[struct:Lock] $11), refl($11)), fn:within_limit(view[u8](#0.0)), transport(transport(h3, (#0 ==[struct:Lock] $9), refl($9)), fn:within_limit(view[u8](#0.0)), transport(h3, fn:within_limit(view[u8](#0.0)), of_term(proof(of_term($10))))))

obligation dc7d5edece1e24a6 step 1
implies_elim(axiom(cmp_reify[true], int_lt_b(view[u8]($0.0), 3i)), transport(literal(view[u8](3)), int_le(int_add(view[u8]($0.0), 1i), #0), implies_elim(axiom(cmp_reflect[true], lt[u8]($0.0, 3)), h1)))

obligation 31cc4b9cf32e493f step 2
implies_elim(axiom(cmp_reflect[true], lt[u8]($0.0, 3)), h1)

obligation 6e6562c5899c19df step 3
transport(transport(projection(struct:Lock { $0.0, false }.0), (#0 ==[u8] struct:Lock { $0.0, false }.0), refl(struct:Lock { $0.0, false }.0)), fn:within_limit(view[u8](#0)), of_term(proof(of_term($1))))

obligation 92c9d08d31eddf30 step 4
evaluate(int_le_b(view[u8](0), 3i))
"#;
const INCREMENT_V1: &str = r#"locus-proofs 1

obligation 94ca9d97d558fcd0 increment 1
transport(transport(h0, (#0 ==[u8] $1), refl($1)), (int_eq_b(view[u8](#0), view[u8](wrapping_add[u8]($0, 1))) ==[bool] true), implies_elim(axiom(cmp_reify[true], int_eq_b(view[u8](wrapping_add[u8]($0, 1)), view[u8](wrapping_add[u8]($0, 1)))), refl(view[u8](wrapping_add[u8]($0, 1)))))
"#;

#[test]
fn a_version_1_proofs_file_is_moved_into_the_lockfile_once() {
    let directory = scratch("cli_migrate");
    let source = directory.join("lock.lc");
    std::fs::copy(example("lock.lc"), &source).unwrap();
    let sidecar = directory.join("lock.lc.proofs");
    std::fs::write(&sidecar, LOCK_V1).unwrap();
    let other = directory.join("increment.lc");
    std::fs::copy(example("increment.lc"), &other).unwrap();
    let other_sidecar = directory.join("increment.lc.proofs");
    std::fs::write(&other_sidecar, INCREMENT_V1).unwrap();
    let lock = directory.join("Locus.lock");

    // Under `--locked` the file is read, and nothing is moved.
    let (code, stdout, stderr) = check(&other, &["--locked", "--stats"], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stdout.contains("Locus.lock: 1 used, 0 found and recorded, 0 stale, 0 searched"),
        "{stdout}"
    );
    assert!(
        stderr.contains("increment.lc.proofs was read, and is moved into")
            && stderr.contains("by a run without `--locked`"),
        "{stderr}"
    );
    assert!(other_sidecar.exists());
    assert!(!lock.exists());

    // A plain check moves it: every entry is used and gets its claim, the
    // lockfile is written, and the file is deleted. The next run reads the
    // lockfile alone. A fresh search establishes the same claims.
    let (code, stdout, stderr) = check(&source, &["--stats"], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stdout.contains("Locus.lock: 8 used, 0 found and recorded, 0 stale, 0 searched"),
        "{stdout}"
    );
    assert!(
        stderr.contains("note: 8 proof(s) of ")
            && stderr.contains("lock.lc.proofs were moved into ")
            && stderr.contains("Locus.lock, 8 of them in use, and the file is deleted"),
        "{stderr}"
    );
    assert!(!sidecar.exists());
    let written = read(&lock);
    assert!(written.contains("path = \"lock.lc\"") && !written.contains("increment.lc"));
    assert_eq!(written.matches("\n  [[file.obligation]]\n").count(), 8);
    assert!(
        written.contains("\n  claim = ") && written.contains("fn:within_limit("),
        "{written}"
    );
    assert!(written.contains("\nt1 = "), "{written}");
    let (code, stdout, stderr) = check(&source, &["--stats", "--locked"], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stdout.contains("Locus.lock: 8 used, "), "{stdout}");
    assert!(!stderr.contains("moved"), "{stderr}");
    assert_eq!(read(&lock), written);
    let fresh = scratch("cli_migrate_fresh");
    let fresh_source = fresh.join("lock.lc");
    std::fs::copy(example("lock.lc"), &fresh_source).unwrap();
    let (code, _, stderr) = check(&fresh_source, &[], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    // A newer proof search may choose a different checked derivation for
    // the same claim. Migration preserves valid old proofs; it need not
    // replace them with the proof today's search would choose.
    let fresh_bytes = read(&fresh.join("Locus.lock"));
    let claims = |text: &str| {
        let (mut lock, warnings) = locus::store::Lockfile::parse(text).unwrap();
        assert!(warnings.is_empty());
        let store = lock.take("lock.lc");
        store
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.key.to_string(),
                    entry.label.to_string(),
                    entry.claim.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(claims(&fresh_bytes), claims(&written));
    let (code, _, stderr) = check(&fresh_source, &["--locked"], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert_eq!(read(&fresh.join("Locus.lock")), fresh_bytes);

    // The second file's own check moves its file into the same lockfile,
    // which keeps the first file's entries.
    let (code, _, stderr) = check(&other, &[], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stderr.contains("note: 1 proof(s) of ")
            && stderr.contains("increment.lc.proofs were moved"),
        "{stderr}"
    );
    assert!(!other_sidecar.exists());
    let both = read(&lock);
    assert_eq!(both.matches("\n[[file]]\n").count(), 2, "{both}");
    assert!(both.ends_with(&written["version = 2\n".len()..]), "{both}");

    // A version-1 file beside a source the lockfile already holds is
    // ignored, and left where it is.
    std::fs::write(&sidecar, LOCK_V1).unwrap();
    let (code, _, stderr) = check(&source, &["--stats"], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stderr.contains("lock.lc.proofs is ignored")
            && stderr.contains("already holds `lock.lc`; delete it"),
        "{stderr}"
    );
    assert!(sidecar.exists());
    assert_eq!(read(&lock), both);
    std::fs::remove_file(&sidecar).unwrap();

    // One that does not read is reported and left, and the proofs are
    // searched for.
    let third = directory.join("preserve.lc");
    std::fs::copy(example("preserve.lc"), &third).unwrap();
    let third_sidecar = directory.join("preserve.lc.proofs");
    std::fs::write(&third_sidecar, "locus-proofs 7\n").unwrap();
    let (code, stdout, stderr) = check(&third, &["--stats"], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stderr.contains("version 7") && stderr.contains("it is not read"),
        "{stderr}"
    );
    assert!(
        stdout.contains("Locus.lock: 0 used, 4 found and recorded, 0 stale, 4 searched"),
        "{stdout}"
    );
    assert!(third_sidecar.exists());
    assert_eq!(read(&lock).matches("\n[[file]]\n").count(), 3);

    // One whose entries are all of other obligations: none is used, the
    // proofs are searched for, and the file is still deleted.
    let fourth = directory.join("propositions.lc");
    std::fs::copy(example("propositions.lc"), &fourth).unwrap();
    let fourth_sidecar = directory.join("propositions.lc.proofs");
    std::fs::write(&fourth_sidecar, LOCK_V1).unwrap();
    let (code, stdout, stderr) = check(&fourth, &["--stats"], &[]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stderr.contains("note: 8 proof(s) of ")
            && stderr.contains("Locus.lock, 0 of them in use, and the file is deleted"),
        "{stderr}"
    );
    assert!(
        stdout.contains("Locus.lock: 0 used, 5 found and recorded, 0 stale, 5 searched"),
        "{stdout}"
    );
    assert!(!fourth_sidecar.exists());
    assert_eq!(read(&lock).matches("\n[[file]]\n").count(), 4);
}
