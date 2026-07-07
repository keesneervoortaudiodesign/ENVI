//! Dynamic FORCE/TOML case runner: one libtest-mimic `Trial` per discovered
//! case (`harness = false` — see `[[test]]` in Cargo.toml).
//!
//! This is the walking skeleton's end-to-end spine: case files → discovery →
//! capability gate → engine dispatch → comparison → per-case outcome line.

use std::path::{Path, PathBuf};

use envi_harness::Outcome;
use libtest_mimic::{Arguments, Trial};

/// Workspace root, resolved from this crate's manifest dir so the test binary
/// finds `refs/` and `cases/` regardless of the invocation working directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/envi-harness has a workspace root two levels up")
        .to_path_buf()
}

fn main() -> std::process::ExitCode {
    let args = Arguments::from_args();
    let root = workspace_root();

    let discovery = envi_harness::cases::discover(&root.join("refs"), &root.join("cases"));

    let mut trials = Vec::new();

    // Meta-test: the harness must always discover at least one case (the
    // committed synthetic TOML cases guarantee this once the loaders exist).
    // Task 1 RED: discovery is a stub returning nothing, so this fails —
    // the end-to-end failing test that precedes any propagation code.
    let discovered = discovery.cases.len();
    trials.push(Trial::test("harness::discovery", move || {
        if discovered == 0 {
            return Err("no cases discovered under refs/ and cases/".into());
        }
        Ok(())
    }));

    // One Trial per discovered case. A case that failed to load surfaces as a
    // real (non-ignored) failing trial; a runnable case is dispatched through
    // the capability gate — every FORCE road case gates to Skipped in this
    // plan and is marked ignored with its requires-list as the trial kind, so
    // `cargo test` output shows WHY each case is ignored before any propagation
    // code exists.
    for discovered in discovery.cases {
        let name = discovered.id;
        match discovered.case {
            Err(err) => {
                let message = format!("failed to load: {err}");
                trials.push(Trial::test(name, move || Err(message.into())).with_kind("load-error"));
            }
            Ok(case) => match envi_harness::run_case(&case) {
                Outcome::Pass => trials.push(Trial::test(name, || Ok(()))),
                Outcome::Fail(report) => {
                    let table = report.render_table();
                    trials.push(Trial::test(name, move || Err(table.into())));
                }
                Outcome::Skipped(why) => {
                    let reason = why.clone();
                    trials.push(
                        Trial::test(name, move || Err(format!("SKIP (expected): {why}").into()))
                            .with_kind(reason)
                            .with_ignored_flag(true),
                    );
                }
            },
        }
    }

    libtest_mimic::run(&args, trials).exit_code()
}
