//! The L1 parse and L2 structure gates from `docs/research/latex-stack/testing.md`.
//!
//! The curated checklist lives in `conformance.rs`; these tests widen the
//! claim to KaTeX's published function list and check that no source byte is
//! dropped between the lexer and the model.

mod support;

use math_syntax::parse;

/// Every listed command is probed in several positions. Acceptance means one
/// probe parses without diagnostics: parse coverage, not visual fidelity.
#[test]
fn katex_supported_list_does_not_regress() {
    let commands = support::katex_commands();
    let environments = support::katex_environments();
    let mut accepted = Vec::new();
    for command in &commands {
        if support::command_accepted(command) {
            accepted.push(command.clone());
        }
    }
    for environment in &environments {
        if support::environment_accepted(environment) {
            accepted.push(format!("env:{environment}"));
        }
    }
    let total = commands.len() + environments.len();
    println!(
        "KaTeX supported list: {}/{} entries parse through the probe set",
        accepted.len(),
        total
    );

    if let Ok(path) = std::env::var("KATEX_BASELINE") {
        let header = "# Entries from katex_supported.txt / katex_environments.txt that the probe\n\
# set accepts. Regenerate with the command in docs/completeness.md.\n\
# A missing entry here is a regression; a new one is reported, not failed.\n";
        std::fs::write(&path, format!("{header}{}\n", accepted.join("\n")))
            .expect("write the accepted baseline");
    }

    let baseline: Vec<&str> = include_str!("data/katex_accepted.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    let regressed: Vec<&str> = baseline
        .iter()
        .copied()
        .filter(|name| !accepted.iter().any(|candidate| candidate == name))
        .collect();
    assert!(
        regressed.is_empty(),
        "KaTeX entries that stopped parsing: {regressed:?}"
    );
}

/// L2: every non-whitespace byte of a parsed formula is inside some node's
/// span or a diagnostic. Macro definitions are excluded because the model does
/// not retain them; see the completeness note.
#[test]
fn every_corpus_byte_is_covered_by_a_span_or_diagnostic() {
    for (name, source) in support::CORE {
        if source.contains("\\def")
            || source.contains("\\newcommand")
            || source.contains("\\renewcommand")
        {
            continue;
        }
        let document = parse(source);
        let mut covered = vec![false; source.len()];
        for id in document.model.ids() {
            let span = document.model.span(id);
            for byte in span.start..span.end {
                if let Some(slot) = covered.get_mut(byte) {
                    *slot = true;
                }
            }
        }
        for diagnostic in &document.diagnostics {
            for byte in diagnostic.span.start..diagnostic.span.end {
                if let Some(slot) = covered.get_mut(byte) {
                    *slot = true;
                }
            }
        }
        let uncovered: Vec<(usize, char)> = source
            .char_indices()
            .filter(|(byte, character)| !character.is_whitespace() && !covered[*byte])
            .collect();
        assert!(
            uncovered.is_empty(),
            "{name}: bytes {uncovered:?} are outside every span in {source}"
        );
    }
}
