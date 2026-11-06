//! The L5 robustness gate: arbitrary text and every prefix of the corpus must
//! return a tree, never panic and never hang.

mod support;

use math_syntax::parse;

/// A deterministic generator, so a failure can be replayed from its seed.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn arbitrary_text_never_panics() {
    // Syntax the lexer recognises, plus characters it has to reject.
    let alphabet: Vec<char> = "\\{}^_&$%# abcXYZ0123+-=()[],.;:!?/'\"\n\t~"
        .chars()
        .chain("αβγ∑∫→⏞⏟⟨⟩‖\u{0}\u{7}\u{301}\u{ffff}".chars())
        .collect();
    let mut noise = Noise(0x9e37_79b9_7f4a_7c15);
    for _ in 0..3_000 {
        let length = (noise.next() >> 32) as usize % 120;
        let mut source = String::with_capacity(length);
        for _ in 0..length {
            let index = noise.next() as usize % alphabet.len();
            source.push(alphabet[index]);
        }
        let document = parse(&source);
        let _ = document.has_errors();
        let latex = document.to_latex();
        let _ = parse(&latex);
    }
}

#[test]
fn every_prefix_of_the_corpus_parses() {
    for (name, source) in support::CORE {
        for (end, _) in source.char_indices() {
            let prefix = &source[..end];
            let document = parse(prefix);
            let _ = document.has_errors();
            let _ = document.to_latex();
            assert!(
                document.model.is_alive(document.root()),
                "{name}: prefix {prefix:?} lost its root"
            );
        }
    }
}
