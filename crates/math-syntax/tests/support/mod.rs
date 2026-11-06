//! Shared corpus and the KaTeX-list probes used by the completeness gates.
//! Each test binary compiles the module separately and uses a subset.
#![allow(dead_code)]

use math_syntax::parse;

/// The curated construct checklist. Each entry must parse without a
/// diagnostic; the completeness gates probe the wider KaTeX list separately.
pub const CORE: &[(&str, &str)] = &[
    ("row", r"a+b=c"),
    ("greek", r"\alpha+\beta=\gamma"),
    ("greek uppercase", r"\Gamma\Delta\Theta"),
    ("relations", r"x \le y \ne z"),
    ("negated relation", r"a \not\subset b"),
    ("arrows", r"A \to B \Rightarrow C \leftrightarrow D"),
    (
        "symbols",
        r"\infty \partial \nabla \forall \exists \emptyset",
    ),
    ("dots", r"a_1, \ldots, a_n"),
    ("scripts", r"x^2+y_1"),
    ("both scripts", r"x_i^2"),
    ("nested script", r"x^{a^b}"),
    ("sum with limits", r"\sum_{i=1}^{n} i"),
    ("fraction", r"\frac{a}{b}"),
    ("dfrac", r"\dfrac{1}{2}"),
    ("tfrac", r"\tfrac{1}{2}"),
    ("nested fraction", r"\frac{1}{1+\frac{1}{x}}"),
    ("over", r"a \over b"),
    ("atop", r"{n \atop k}"),
    ("choose", r"{n \choose k}"),
    ("brace", r"{n \brace k}"),
    ("brack", r"{n \brack k}"),
    ("square root", r"\sqrt{x}"),
    ("indexed root", r"\sqrt[3]{x}"),
    ("left right", r"\left(\frac{a}{b}\right)"),
    ("null delimiter", r"\left. \frac{df}{dx} \right|_{x=0}"),
    ("middle", r"\left\{ x \middle| x > 0 \right\}"),
    ("big family", r"\big( \Big[ \bigg\{ \Bigg)"),
    ("integral", r"\int_0^1 f(x)\,dx"),
    ("product", r"\prod_{i=1}^{n} a_i"),
    ("limit", r"\lim_{x \to 0} \frac{\sin x}{x}"),
    ("named function", r"\sin x + \cos y"),
    ("operatorname", r"\operatorname{rank}(A)"),
    ("operatorname star", r"\operatorname*{arg\,max}_{x} f(x)"),
    ("hat", r"\hat{x}"),
    ("wide accent", r"\widehat{xy}"),
    ("accent set", r"\bar{x} \vec{v} \dot{x} \ddot{x} \tilde{x}"),
    ("text", r"\text{if } x > 0"),
    ("text in script", r"x_{\text{max}}"),
    ("overline", r"\overline{AB}"),
    ("underline", r"\underline{x+y}"),
    ("spacing", r"a\,b\;c\quad d\qquad e\!f"),
    ("mathbb", r"\mathbb{R}"),
    ("mathbf", r"\mathbf{v}"),
    ("mathcal", r"\mathcal{L}"),
    ("mathfrak", r"\mathfrak{g}"),
    ("boldsymbol", r"\boldsymbol{\alpha}"),
    ("mathrm", r"\mathrm{d}x"),
    ("def", r"\def\RR{\mathbb{R}}\RR"),
    (
        "newcommand",
        r"\newcommand{\pair}[2]{\left(#1,#2\right)}\pair{a}{b}",
    ),
    (
        "renewcommand",
        r"\newcommand{\vvec}[1]{#1}\renewcommand{\vvec}[1]{\mathbf{#1}}\vvec{v}",
    ),
    ("not", r"a \not= b"),
    ("phantom", r"\phantom{xx}"),
    ("hphantom vphantom", r"\hphantom{x}\vphantom{y}"),
    ("smash", r"\smash{x}"),
    ("boxed", r"\boxed{E=mc^2}"),
    ("overset", r"\overset{!}{=}"),
    ("underset", r"\underset{x}{\lim}"),
    ("fraction with script", r"\frac{a}{b}^2"),
    ("escaped braces", r"\{x\}"),
    ("max over a set", r"\max_{x\in A} f(x)"),
    ("matrix", r"\begin{matrix} a & b \\ c & d \end{matrix}"),
    ("pmatrix", r"\begin{pmatrix} 1 & 0 \\ 0 & 1 \end{pmatrix}"),
    ("bmatrix", r"\begin{bmatrix} a \\ b \end{bmatrix}"),
    (
        "cases",
        r"f(x) = \begin{cases} x & x > 0 \\ 0 & x \le 0 \end{cases}",
    ),
    ("aligned", r"\begin{aligned} a &= b \\ c &= d \end{aligned}"),
    ("stackrel", r"\stackrel{a}{=}"),
    ("substack", r"\sum_{\substack{0<i<n}} i"),
    ("cfrac", r"\cfrac{1}{2}"),
    ("pmod", r"a \equiv b \pmod{n}"),
    ("bmod", r"a \bmod b"),
    ("hspace", r"a\hspace{1em}b"),
    ("cancel", r"\cancel{x}"),
    ("genfrac", r"\genfrac{(}{)}{0pt}{}{a}{b}"),
    ("xrightarrow", r"a \xrightarrow{f} b"),
    ("xleftarrow", r"a \xleftarrow{g} b"),
    ("underbrace", r"\underbrace{a+b}_{n}"),
    ("overbrace", r"\overbrace{a+b}^{n}"),
];

pub fn katex_commands() -> Vec<String> {
    include_str!("../data/katex_supported.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

pub fn katex_environments() -> Vec<String> {
    include_str!("../data/katex_environments.txt")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// Ways a command may be used. A command is accepted when one of them parses
/// without diagnostics; that is the L1 parse gate, not a fidelity claim.
fn probes(command: &str) -> Vec<String> {
    let mut probes = vec![
        command.to_owned(),
        format!("{command}{{a}}"),
        format!("{command}{{a}}{{b}}"),
        format!("{command}[1]{{a}}"),
        format!("{command}*{{a}}"),
        format!("{{a {command} b}}"),
        format!("{command}{{1em}}"),
        format!("{command}{{a}}{{b}}{{c}}"),
    ];
    // Commands whose grammar no generic template spells.
    probes.extend(match command {
        "\\left" | "\\right" => vec!["\\left( a \\right)".to_owned()],
        "\\middle" => vec!["\\left( a \\middle| b \\right)".to_owned()],
        "\\genfrac" => vec!["\\genfrac{(}{)}{0pt}{}{a}{b}".to_owned()],
        "\\def" | "\\gdef" => vec![format!("{command}\\a{{a}}\\a")],
        "\\newcommand" | "\\renewcommand" => {
            vec![format!("{command}{{\\a}}{{a}}\\a")]
        }
        _ => Vec::new(),
    });
    probes
}

fn environment_probes(name: &str) -> Vec<String> {
    vec![
        format!("\\begin{{{name}}}a\\end{{{name}}}"),
        format!("\\begin{{{name}}}[r]a\\end{{{name}}}"),
        format!("\\begin{{{name}}}{{cc}}a\\end{{{name}}}"),
    ]
}

fn clean(source: &str) -> bool {
    !parse(source).has_errors()
}

pub fn command_accepted(command: &str) -> bool {
    probes(command).iter().any(|source| clean(source))
}

pub fn environment_accepted(name: &str) -> bool {
    environment_probes(name).iter().any(|source| clean(source))
}
