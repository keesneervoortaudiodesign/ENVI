//! `envi-harness` CLI — human-readable per-case outcome report.
//!
//! Usage: `cargo run -p envi-harness -- report`
//!
//! This is the walking skeleton's human-readable "write" end: it runs the same
//! discovery → capability-gate → dispatch → comparison pipeline the test target
//! drives, and prints a fixed-width per-case outcome table.

use std::path::{Path, PathBuf};

use envi_harness::cases::{CaseKind, discover};
use envi_harness::{Outcome, run_case};

/// Workspace root, resolved from this crate's manifest dir so the binary finds
/// `refs/` and `cases/` regardless of the invocation working directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/envi-harness has a workspace root two levels up")
        .to_path_buf()
}

/// Short, stable label for a case kind (report column).
fn kind_label(kind: CaseKind) -> &'static str {
    match kind {
        CaseKind::FreeField => "free-field",
        CaseKind::Geometry => "geometry",
        CaseKind::ForceStraightRoad => "straight-road",
        CaseKind::ForceCurvedRoad => "curved-road",
        CaseKind::ForceCityStreet => "city-street",
        CaseKind::ForceYearlyAverage => "yearly-average",
    }
}

fn main() -> std::process::ExitCode {
    let cmd = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "report".to_string());
    if cmd != "report" {
        eprintln!("usage: envi-harness report");
        return std::process::ExitCode::from(2);
    }

    let root = workspace_root();
    let discovery = discover(&root.join("refs"), &root.join("cases"));

    for note in &discovery.notes {
        println!("note: {note}");
    }

    println!(
        "{:<26} {:<15} {:<11} {:<8} detail",
        "case", "kind", "reference", "outcome"
    );
    println!("{}", "-".repeat(90));

    for discovered in &discovery.cases {
        let (kind, reference, outcome, detail) = match &discovered.case {
            Err(err) => ("-", "-", "LoadErr", format!("{err}")),
            Ok(case) => {
                let (outcome, detail) = match run_case(case) {
                    Outcome::Pass => ("Pass", String::new()),
                    Outcome::Skipped(why) => ("Skipped", why),
                    Outcome::Fail(report) => (
                        "Fail",
                        format!("max |dev| = {:.3} dB", report.max_abs_dev_db),
                    ),
                };
                (
                    kind_label(case.kind),
                    case.reference_version.as_str(),
                    outcome,
                    detail,
                )
            }
        };
        println!(
            "{:<26} {:<15} {:<11} {:<8} {}",
            discovered.id, kind, reference, outcome, detail
        );
    }

    std::process::ExitCode::SUCCESS
}
