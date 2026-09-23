//! Measurements are raw observations, never acceptance heuristics.
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn raw_benchmark_separates_search_replay_and_compilation_phases() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .args([
            "bench",
            "tests/corpus/target/midpoint.lc",
            "--samples",
            "2",
            "--no-record",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut python = Command::new("python3")
        .args([
            "-c",
            r#"
import json,sys
record=json.load(sys.stdin)
assert record['schema_version']==1 and record['unit']=='nanoseconds'
assert record['compiler_build']['toolchain'].startswith('rustc ')
assert record['compiler_build']['source_git_blob'] != 'unavailable'
assert len(record['samples'])==4
for sample in record['samples']:
    for phase in ['parse_ns','elaboration_ns','lowering_ns','checking_ns','erasure_ns','total_ns']:
        assert isinstance(sample[phase],int) and sample[phase]>=0,phase
    assert sample['obligations']
    for obligation in sample['obligations']:
        assert isinstance(obligation['kernel_recheck_ns'],int)
        assert isinstance(obligation['proof_nodes'],int) and obligation['proof_nodes']>0
        assert isinstance(obligation['certificate_bytes'],int) and obligation['certificate_bytes']>0
        assert all(isinstance(x,int) and x>=0 for x in obligation['search_ns_by_tier'].values())
        assert obligation['store_hit']==(sample['pass']=='replay')
    if sample['pass']=='replay':
        assert sample['store_hits']==len(sample['obligations']) and sample['store_searches']==0
    else:
        assert sample['store_hits']==0 and sample['store_searches']>0
"#,
        ])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    python
        .stdin
        .take()
        .unwrap()
        .write_all(&output.stdout)
        .unwrap();
    assert!(python.wait().unwrap().success());
}
#[test]
fn measurement_capture_restores_nested_and_panicking_scopes() {
    let (_, samples) = locus::measurement::capture(|| {
        {
            let _timer = locus::measurement::start("outer");
        }
        let (_, inner) = locus::measurement::capture(|| {
            let _timer = locus::measurement::start("inner");
        });
        assert_eq!(inner.len(), 1);
        let caught = std::panic::catch_unwind(|| locus::measurement::capture(|| panic!("probe")));
        assert!(caught.is_err());
        let _timer = locus::measurement::start("restored");
    });
    assert_eq!(
        samples.iter().map(|s| s.stage).collect::<Vec<_>>(),
        ["outer", "restored"]
    );
    assert!(locus::measurement::checkpoint().is_none());
}
#[test]
fn benchmark_history_epochs_noise_and_immutability_are_tested() {
    let output = Command::new("python3")
        .arg("tools/test_bench.py")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_status_refuses_partial_logs_and_stale_measurements() {
    let output = Command::new("python3")
        .arg("tools/test_metrics.py")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn benchmark_driver_refuses_oversized_sparse_sources_before_reading() {
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bench_oversized.lc");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(locus::limits::MAX_SOURCE_BYTES as u64 + 1)
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("bench")
        .arg(&path)
        .args(["--samples", "1", "--no-record"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("MAX_SOURCE_BYTES") && error.contains("L0010"),
        "{error}"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
#[doc = "spec: 1.20:1"]
fn benchmark_search_and_replay_do_not_read_or_rewrite_disk_lockfiles() {
    let directory =
        std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bench_lockfile_isolation");
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("sample.lc");
    let lock = directory.join("Locus.lock");
    let legacy = directory.join("sample.lc.proofs");
    std::fs::write(
        &source,
        "fn run() -> u8 { let proof = prove!(1 == 1); 7 }\n",
    )
    .unwrap();
    std::fs::write(&lock, "deliberately invalid on-disk lockfile\n").unwrap();
    std::fs::write(&legacy, "legacy sentinel\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("bench")
        .arg(&source)
        .args(["--samples", "1", "--no-record"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"pass\":\"search\""), "{text}");
    assert!(text.contains("\"pass\":\"replay\""), "{text}");
    assert!(text.contains("\"store_hit\":true"), "{text}");
    assert_eq!(
        std::fs::read_to_string(&lock).unwrap(),
        "deliberately invalid on-disk lockfile\n"
    );
    assert_eq!(
        std::fs::read_to_string(&legacy).unwrap(),
        "legacy sentinel\n"
    );
}
