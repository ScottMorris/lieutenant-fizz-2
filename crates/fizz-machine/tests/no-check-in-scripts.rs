// Fails if a script or workflow could start the interactive check screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `FIZZ2.EXE /CHECK` waits for a key press, so a script or CI job that ran it would hang until its timeout. The non-interactive forms
// are `/PROBE` and `/CHECKCELLS`. This test reads every shell script and python tool under `tools/` and every workflow under
// `.github/`, and rejects any non-comment line that names `/CHECK` other than as the start of `/CHECKCELLS`. Comment lines (those
// starting with `#`) may talk about the screen.

use std::fs;
use std::path::{Path, PathBuf};

/// Whether `line` names `/CHECK` as a switch of its own (case-insensitively, as DOS reads it): after a space, tab or quote, and not
/// followed by more name (`/CHECKCELLS`, or a path such as `tools/check.sh`).
fn runs_check(line: &str) -> bool {
    let upper = line.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let mut from = 0;
    while let Some(at) = upper[from..].find("/CHECK").map(|i| i + from) {
        let before_ok = at == 0 || matches!(bytes[at - 1], b' ' | b'\t' | b'"' | b'\'');
        let after = bytes.get(at + "/CHECK".len()).copied();
        let after_ok =
            after.is_none_or(|c| !(c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_')));
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Test inputs may quote anything.
            if path.file_name().is_some_and(|n| n == "tests") {
                continue;
            }
            collect(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("sh" | "py" | "yml" | "yaml" | "mjs" | "js" | "bat")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn the_detector_tells_check_from_checkcells() {
    assert!(runs_check("tools/dosrun.sh dist/dos FIZZ2.EXE /CHECK"));
    assert!(runs_check("fizz2.exe /check"));
    assert!(runs_check("x /CHECKCELLS /CHECK"));
    assert!(!runs_check(
        "tools/dosrun.sh dist/dos FIZZ2.EXE /CHECKCELLS"
    ));
    assert!(!runs_check("echo nothing here"));
}

#[test]
fn no_script_runs_the_interactive_check() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    collect(&root.join("tools"), &mut files);
    collect(&root.join(".github"), &mut files);
    assert!(
        files.iter().any(|f| f.ends_with("check-dos.sh")),
        "the scan did not find tools/check-dos.sh"
    );
    let mut bad = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with('#') {
                continue;
            }
            if runs_check(code) {
                bad.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "scripts must not run the interactive /CHECK (use /CHECKCELLS or /PROBE):\n{}",
        bad.join("\n")
    );
}
