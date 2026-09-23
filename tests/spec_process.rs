//! The documentation and known-bug gates are ordinary fast-suite tests.
#[path = "common/known_bug.rs"]
mod expected_failure;
#[path = "common/corpus.rs"]
mod runner;
use std::process::Command;

#[test]
fn atlas_rules_have_focused_tests_and_every_now_fence_declares_its_mode() {
    let output = Command::new("python3")
        .args(["tools/spec.py", "check"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn specification_gate_rejects_its_adversarial_scratch_atlases() {
    let output = Command::new("python3")
        .arg("tools/test_spec.py")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn known_compiler_rejections_are_pinned_and_an_unexpected_pass_fails_loudly() {
    let bad = "fn value()->u8{true} //~ error: L0220\n//~ known: LOC-91 synthetic runner case\n";
    let result = runner::examine("synthetic-known.lc", bad);
    assert!(result.failures.is_empty(), "{:?}", result.failures);
    let fixed = bad.replace("{true}", "{1}");
    let result = runner::examine("synthetic-known-fixed.lc", &fixed);
    assert!(
        result
            .failures
            .iter()
            .any(|f| f.message.contains("LOC-91 unexpectedly passed")
                && f.message.contains("remove")),
        "{:?}",
        result.failures
    );
    let drifted = bad.replace("{true}", "{missing}");
    let result = runner::examine("synthetic-known-drifted.lc", &drifted);
    assert!(
        !result.failures.is_empty(),
        "a changed failure was silently accepted"
    );
}

#[test]
fn a_known_marker_requires_a_reason_and_a_pinned_error() {
    let missing_reason = runner::examine("reason.lc", "fn f()->u8{true} //~ known: LOC-91\n");
    assert!(
        missing_reason
            .failures
            .iter()
            .any(|f| f.message.contains("and a reason"))
    );
    let unpinned = runner::examine(
        "unpinned.lc",
        "fn f()->u8{true}\n//~ known: LOC-91 synthetic runner case\n",
    );
    assert!(
        unpinned
            .failures
            .iter()
            .any(|f| f.message.contains("pinned error directives"))
    );
}

#[test]
fn rust_known_bug_helper_rejects_unexpected_pass_and_error_drift() {
    // A synthetic helper test, not a claim that LOC-91 is an actual open bug.
    let task = "LOC-91";
    expected_failure::known_bug(
        task,
        "synthetic",
        "pinned failure",
        Err("pinned failure at the call".into()),
    );
    let unexpected = std::panic::catch_unwind(|| {
        expected_failure::known_bug(task, "synthetic", "pinned failure", Ok(()))
    });
    assert!(unexpected.is_err());
    let drifted = std::panic::catch_unwind(|| {
        expected_failure::known_bug(
            task,
            "synthetic",
            "pinned failure",
            Err("another failure".into()),
        )
    });
    assert!(drifted.is_err());
}
