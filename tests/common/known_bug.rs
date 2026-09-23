//! Executable expected failures; a fixed bug must remove its marker.
#![allow(dead_code)]
pub fn known_bug(task: &str, reason: &str, expected: &str, result: Result<(), String>) {
    assert!(
        task.starts_with("LOC-") && !reason.trim().is_empty() && !expected.is_empty(),
        "known_bug needs a task, reason, and pinned failure"
    );
    match result {
        Ok(()) => panic!("{task} unexpectedly passed; remove the known-bug marker ({reason})"),
        Err(actual) => assert!(
            actual.contains(expected),
            "{task} failure changed: expected `{expected}`, got `{actual}`"
        ),
    }
}
