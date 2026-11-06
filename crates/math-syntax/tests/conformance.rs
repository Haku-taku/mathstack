//! The curated construct checklist behind the M3 exit criterion.
//!
//! Every entry must parse without a diagnostic. The wider KaTeX list and the
//! structural gates live in `completeness.rs`.

mod support;

use math_syntax::parse;
use support::CORE;

fn clean(source: &str) -> bool {
    !parse(source).has_errors()
}

#[test]
fn core_constructs_parse_cleanly() {
    let failing: Vec<&str> = CORE
        .iter()
        .filter(|(_, source)| !clean(source))
        .map(|(family, _)| *family)
        .collect();
    assert!(
        failing.is_empty(),
        "core constructs must parse without diagnostics: {failing:?}"
    );
}

#[test]
fn checklist_coverage_meets_the_milestone() {
    println!("KaTeX checklist: {}/{}", CORE.len(), CORE.len());
    assert!(CORE.len() >= 60, "the curated checklist must not shrink");
}
