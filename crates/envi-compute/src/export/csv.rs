//! Hand-rolled receiver-spectra CSV encoder (D-21/D-22).
//!
//! # Columns (RESEARCH §Export)
//! `band_index, exact_hz, <receiver…>` — one row per 1/12-octave band (105 rows),
//! then two footer rows carrying each receiver's dB(A) and dB(C) total. The
//! **band index is the identity** and the **exact Hz** comes verbatim from
//! [`FreqAxis::centres`] — the nominal 1/3-octave labels (25, 31.5, …) are NEVER
//! written as the identity (RESEARCH Pitfall 3: `f == 31.5` is a bug).
//!
//! The file opens with the [`ExportMeta`] attribution/metadata footer as `#`
//! comment lines (D-22), so a downloaded CSV is self-identifying.

use envi_engine::freq::{FreqAxis, N_BANDS};

use crate::export::ExportMeta;
use crate::readout::ReceiverReadout;

/// Encode per-receiver spectra as a CSV with a band-index column AND an exact-Hz
/// column, plus dB(A)/dB(C) total footer rows and the [`ExportMeta`] header (D-22).
///
/// `labels` names the receiver columns (typically the TS-minted receiver UUIDs, in
/// receiver-major order); it is aligned 1:1 with `receivers`. A label/receiver
/// count mismatch is tolerated by naming any unlabelled column `receiver_<i>` — the
/// encoder never panics on data.
#[must_use]
pub fn encode_spectra_csv(
    labels: &[String],
    receivers: &[ReceiverReadout],
    axis: &FreqAxis,
    meta: &ExportMeta,
) -> String {
    let mut out = String::new();
    out.push_str(&meta.csv_comment_lines());

    // Header row. Receiver labels are untrusted free text (the "TS-minted UUID"
    // contract is documentation, not enforcement), so each is RFC-4180-quoted and
    // formula-injection-guarded (WR-02) before it joins the header.
    out.push_str("band_index,exact_hz");
    for i in 0..receivers.len() {
        out.push(',');
        out.push_str(&csv_field(&column_label(labels, i)));
    }
    out.push('\n');

    // One row per band index; exact Hz from the frozen axis (never nominal).
    for f in 0..N_BANDS {
        out.push_str(&format!("{f},{}", axis.centres[f]));
        for rcv in receivers {
            let v = rcv.band_levels_db.get(f).copied().unwrap_or(f64::NAN);
            out.push(',');
            out.push_str(&fmt_level(v));
        }
        out.push('\n');
    }

    // dB(A) / dB(C) total footer rows (band_index column holds the label, exact_hz
    // left blank — these are broadband totals, not a band).
    out.push_str("dBA_total,");
    for rcv in receivers {
        out.push(',');
        out.push_str(&fmt_level(rcv.total_dba));
    }
    out.push('\n');
    out.push_str("dBC_total,");
    for rcv in receivers {
        out.push(',');
        out.push_str(&fmt_level(rcv.total_dbc));
    }
    out.push('\n');

    out
}

/// The column label for receiver `i` — the provided label, else `receiver_<i>`.
fn column_label(labels: &[String], i: usize) -> String {
    labels
        .get(i)
        .cloned()
        .unwrap_or_else(|| format!("receiver_{i}"))
}

/// RFC-4180-quote a CSV field AND neutralize spreadsheet formula injection (WR-02).
///
/// - **Quoting:** a field containing a comma, `"`, CR, or LF is wrapped in `"…"`
///   with every internal `"` doubled — so a label like `road, north` cannot inject
///   an extra column and an embedded newline cannot break the row structure.
/// - **Formula-injection guard:** a field beginning with `=`, `+`, `-`, `@`, tab, or
///   CR is prefixed with a `'` so a spreadsheet opens the downloaded cell as text,
///   never evaluates it as a formula.
///
/// Only untrusted TEXT fields (receiver labels) pass through here — the numeric
/// level columns are program-formatted finite `f64`s and stay bare so the columns
/// remain parseable (a leading `-` on a number is legitimate numeric data).
fn csv_field(s: &str) -> String {
    // Formula-injection guard first (a leading trigger becomes inert text).
    let guarded = match s.chars().next() {
        Some('=' | '+' | '-' | '@' | '\t' | '\r') => {
            let mut g = String::with_capacity(s.len() + 1);
            g.push('\'');
            g.push_str(s);
            g
        }
        _ => s.to_string(),
    };
    // RFC-4180 quoting when the (guarded) field carries a delimiter/quote/newline.
    if guarded.contains([',', '"', '\n', '\r']) {
        let mut out = String::with_capacity(guarded.len() + 2);
        out.push('"');
        for ch in guarded.chars() {
            if ch == '"' {
                out.push('"'); // double the internal quote
            }
            out.push(ch);
        }
        out.push('"');
        out
    } else {
        guarded
    }
}

/// Format a level with round-trippable precision; a non-finite value (silence
/// floor / no-data) is written as the literal `NaN` so the column stays parseable.
fn fmt_level(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "NaN".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> ExportMeta {
        ExportMeta {
            epsg: 32631,
            weighting_label: "dB(A)".to_string(),
            engine_version: "envi-test".to_string(),
            tensor_hash: "abc123".to_string(),
            attribution: "© OpenStreetMap contributors; Copernicus".to_string(),
        }
    }

    fn readout(base: f64) -> ReceiverReadout {
        ReceiverReadout {
            band_levels_db: (0..N_BANDS).map(|f| base + f as f64).collect(),
            coherent_energy: vec![0.0; N_BANDS],
            incoherent_energy: vec![0.0; N_BANDS],
            coherent_db: vec![0.0; N_BANDS],
            incoherent_db: vec![0.0; N_BANDS],
            total_dba: base + 90.0,
            total_dbc: base + 92.0,
            total_coherent_db: 0.0,
            total_incoherent_db: 0.0,
        }
    }

    #[test]
    fn csv_has_band_index_and_exact_hz_columns_and_attribution_header() {
        let axis = FreqAxis::new();
        let rcv = [readout(30.0), readout(40.0)];
        let labels = ["rcv-A".to_string(), "rcv-B".to_string()];
        let csv = encode_spectra_csv(&labels, &rcv, &axis, &meta());
        let lines: Vec<&str> = csv.lines().collect();

        // Attribution/metadata header present (D-22).
        assert!(csv.contains("# Attribution: © OpenStreetMap"));
        assert!(csv.contains("# CRS: EPSG:32631"));

        // The header row carries the band_index + exact_hz columns and the labels.
        let header = lines
            .iter()
            .find(|l| l.starts_with("band_index,exact_hz"))
            .expect("header row present");
        assert_eq!(*header, "band_index,exact_hz,rcv-A,rcv-B");

        // Every band-index row's exact Hz equals FreqAxis::centres[f] exactly
        // (round-trip through the default f64 formatting), never a nominal label.
        let data: Vec<&str> = lines
            .iter()
            .filter(|l| l.chars().next().is_some_and(|c| c.is_ascii_digit()))
            .copied()
            .collect();
        assert_eq!(data.len(), N_BANDS);
        for (f, row) in data.iter().enumerate() {
            let cols: Vec<&str> = row.split(',').collect();
            assert_eq!(cols[0].parse::<usize>().unwrap(), f, "band index column");
            let hz: f64 = cols[1].parse().unwrap();
            assert_eq!(
                hz, axis.centres[f],
                "exact Hz equals FreqAxis::centres[{f}]"
            );
            // The 1/3-octave labels (25.0, 31.5, …) are never the identity.
            assert_ne!(hz, 31.5);
        }
    }

    #[test]
    fn csv_carries_dba_and_dbc_total_footer_rows() {
        let axis = FreqAxis::new();
        let rcv = [readout(30.0)];
        let csv = encode_spectra_csv(&["r0".to_string()], &rcv, &axis, &meta());
        let a_row = csv.lines().find(|l| l.starts_with("dBA_total,")).unwrap();
        let c_row = csv.lines().find(|l| l.starts_with("dBC_total,")).unwrap();
        assert!(a_row.ends_with("120"), "dB(A) total = 30 + 90");
        assert!(c_row.ends_with("122"), "dB(C) total = 30 + 92");
    }

    #[test]
    fn labels_are_rfc4180_quoted_and_formula_injection_guarded() {
        // WR-02: an untrusted label with a comma, an embedded quote, or a leading
        // formula trigger must not corrupt the CSV or ride as a live spreadsheet
        // formula. A comma inside a label must not inject a column.
        let axis = FreqAxis::new();
        let rcv = [readout(30.0), readout(40.0), readout(50.0)];
        let labels = [
            "road, north".to_string(), // comma → quoted
            "say \"hi\"".to_string(),  // embedded quote → doubled + quoted
            "=1+2".to_string(),        // formula injection → ' guard
        ];
        let csv = encode_spectra_csv(&labels, &rcv, &axis, &meta());
        let header = csv
            .lines()
            .find(|l| l.starts_with("band_index,exact_hz"))
            .expect("header row present");
        assert_eq!(
            header,
            "band_index,exact_hz,\"road, north\",\"say \"\"hi\"\"\",'=1+2"
        );

        // Every data row still has exactly 2 + 3 = 5 fields under a proper RFC-4180
        // parse (the comma inside the quoted label is NOT a delimiter). A naive
        // comma-split of a data row yields 5 too, since the quoted label sits in the
        // header only; here we assert the header field count via a minimal parser.
        assert_eq!(rfc4180_field_count(header), 5, "header has 5 fields");
    }

    /// A minimal RFC-4180 field counter (quote-aware) for the test above.
    fn rfc4180_field_count(line: &str) -> usize {
        let mut fields = 1;
        let mut in_quotes = false;
        for ch in line.chars() {
            match ch {
                '"' => in_quotes = !in_quotes,
                ',' if !in_quotes => fields += 1,
                _ => {}
            }
        }
        fields
    }

    #[test]
    fn missing_labels_fall_back_to_receiver_index_never_panics() {
        let axis = FreqAxis::new();
        let rcv = [readout(10.0), readout(20.0)];
        let csv = encode_spectra_csv(&[], &rcv, &axis, &meta());
        assert!(csv.contains("band_index,exact_hz,receiver_0,receiver_1"));
    }
}
