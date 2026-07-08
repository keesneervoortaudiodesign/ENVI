#!/bin/sh
# refs/fetch.sh — download the FORCE/Nord2000 reference artifacts into refs/.
#
# The FORCE test-case workbooks and the AV/DELTA reports are freely
# downloadable but COPYRIGHTED: they must never be committed to git
# (refs/* is git-ignored; only this script and refs.sha256 are tracked).
#
# Integrity (threat T-01-03): SHA-256 sums are pinned in refs/refs.sha256.
# On first successful fetch of a file with no manifest entry, its sum is
# recorded; on every later run the download is verified against the pin.
#
# Behaviour:
#   - idempotent: files already present and verified are not re-downloaded
#   - fail-soft:  network/download failures WARN and exit 0 — the test
#                 harness degrades to Skipped(missing reference data)
#   - a checksum MISMATCH on an existing pin is a hard error (exit 1)
#
# Usage: sh refs/fetch.sh   (from the repository root or refs/)

set -u

# Resolve refs/ as the directory containing this script.
REFS_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
MANIFEST="$REFS_DIR/refs.sha256"

MST_BASE="https://www2.mst.dk/udgiv/publications/2009/978-87-7052-938-9"

# name|url  (POSIX sh: no arrays)
ARTIFACTS="
TestStraightRoad.xls|$MST_BASE/TestStraightRoad/TestStraightRoad.xls
TestCurvedRoad.xls|$MST_BASE/TestCurvedRoad/TestCurvedRoad.xls
TestCityStreet.xls|$MST_BASE/TestCityStreet/TestCityStreet.xls
TestYearlyAverage.xls|$MST_BASE/TestYearlyAverage/TestYearlyAverage.xls
AV1106-07-rev4.pdf|http://web.archive.org/web/20240221070539/https://forcetechnology.com/-/media/force-technology-media/pdf-files/projects/nord2000/nord2000-nordtestproposal-rev4.pdf
AV1849-00-part1.pdf|http://www.magasbakony.hu/Val/Nord2000_homogeneous_atmosphere_Part_1.pdf
EnvProject1335-2010.pdf|https://mst.dk/media/ecyi5sso/revised_test_cases_for_updated_version_of_nord2000.pdf
Users_Guide_Nord2000_Road.pdf|https://egra.cedex.es/EGRA-ingles/I-Documentacion/National_Methods/Users_Guide_Nord2000_Road.pdf
"

# Manual-drop-only artifacts (no stable public URL): drop the file into refs/
# by hand and its sha256 will be recorded/verified on the next run.
#   SP Rapport 2006:12 (Jonasson) — the road-emission coefficient tables. NOT
#   freely downloadable (04-RESEARCH Open Q1); the emission coefficients stay
#   [ASSUMED] until this is dropped in. No URL row on purpose.

sha256_of() {
    # portable-ish sha256 (Git Bash / Linux: sha256sum; macOS: shasum)
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        echo ""
    fi
}

pinned_sum() {
    # $1 = file name; prints the pinned sum or nothing
    [ -f "$MANIFEST" ] || return 0
    awk -v f="$1" '$2 == f { print $1 }' "$MANIFEST"
}

record_sum() {
    # $1 = sum, $2 = file name — append if not already pinned
    if [ -z "$(pinned_sum "$2")" ]; then
        printf '%s  %s\n' "$1" "$2" >>"$MANIFEST"
        echo "  pinned: $2 -> $1"
    fi
}

FAILED=0
for entry in $ARTIFACTS; do
    name=${entry%%|*}
    url=${entry#*|}
    dest="$REFS_DIR/$name"
    pin=$(pinned_sum "$name")

    if [ -f "$dest" ]; then
        sum=$(sha256_of "$dest")
        if [ -n "$pin" ]; then
            if [ "$sum" = "$pin" ]; then
                echo "ok      : $name (present, checksum verified)"
                continue
            fi
            echo "MISMATCH: $name — pinned $pin, got $sum" >&2
            echo "          refusing to trust it; delete the file and re-run." >&2
            exit 1
        fi
        record_sum "$sum" "$name"
        echo "ok      : $name (present, checksum recorded)"
        continue
    fi

    echo "fetching: $name"
    if curl -fsSL --retry 2 --max-time 300 -o "$dest.part" "$url"; then
        mv "$dest.part" "$dest"
        sum=$(sha256_of "$dest")
        if [ -n "$pin" ]; then
            if [ "$sum" != "$pin" ]; then
                echo "MISMATCH: $name — pinned $pin, downloaded $sum" >&2
                rm -f "$dest"
                exit 1
            fi
            echo "ok      : $name (downloaded, checksum verified)"
        else
            record_sum "$sum" "$name"
            echo "ok      : $name (downloaded, checksum recorded)"
        fi
    else
        rm -f "$dest.part"
        echo "WARN    : could not download $name from $url" >&2
        FAILED=1
    fi
done

if [ "$FAILED" -ne 0 ]; then
    echo "WARN: some reference artifacts are missing (offline?)." >&2
    echo "      The test harness degrades gracefully: .xls-backed cases" >&2
    echo "      report as Skipped(missing reference data). Re-run later." >&2
fi
exit 0
