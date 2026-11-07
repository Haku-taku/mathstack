use math_core::{
    Alignment, AtomClass, Color, ColumnSpacing, DelimiterSize, DiagnosticKind, FractionRule,
    Limits, Model, NodeId, NodeKind, Origin, Size, SpaceKind, Span, Style, Variant,
};
use math_syntax::{
    CommandShape, LossKind, command_shape, command_template, parse, to_latex, to_latex_with_report,
    to_latex_with_spans,
};

fn row(model: &Model, id: NodeId) -> &[NodeId] {
    match model.kind(id) {
        NodeKind::Row(children) => children,
        other => panic!("expected a row, found {other:?}"),
    }
}

/// The structure with spans and ids erased, for comparing two parses.
fn dump(model: &Model, id: NodeId) -> String {
    let mut out = String::new();
    write_dump(model, id, &mut out);
    out
}

fn write_dump(model: &Model, id: NodeId, out: &mut String) {
    match model.kind(id) {
        NodeKind::Atom(atom) => {
            out.push_str(&format!("Atom({:?},{:?})", atom.symbol, atom.variant));
        }
        NodeKind::Row(children) => {
            out.push_str("Row(");
            for child in children {
                write_dump(model, *child, out);
                out.push(',');
            }
            out.push(')');
        }
        NodeKind::Scripts { base, sup, sub } => {
            out.push_str("Scripts(");
            write_dump(model, *base, out);
            for script in [sup, sub] {
                out.push(',');
                match script {
                    Some(script) => write_dump(model, *script, out),
                    None => out.push('-'),
                }
            }
            out.push(')');
        }
        NodeKind::Fraction {
            numerator,
            denominator,
            rule,
            style,
        } => {
            out.push_str(&format!("Fraction({rule:?},{style:?},"));
            write_dump(model, *numerator, out);
            out.push(',');
            write_dump(model, *denominator, out);
            out.push(')');
        }
        NodeKind::Arrow {
            symbol,
            over,
            under,
        } => {
            out.push_str(&format!("Arrow({symbol:?},"));
            match over {
                Some(over) => write_dump(model, *over, out),
                None => out.push_str("none"),
            }
            out.push(',');
            match under {
                Some(under) => write_dump(model, *under, out),
                None => out.push_str("none"),
            }
            out.push(')');
        }
        NodeKind::HorizontalBrace { body, over } => {
            out.push_str(&format!("Brace(over={over})("));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Radical { body, degree } => {
            out.push_str("Radical(");
            write_dump(model, *body, out);
            out.push(',');
            match degree {
                Some(degree) => write_dump(model, *degree, out),
                None => out.push('-'),
            }
            out.push(')');
        }
        NodeKind::Delimited { left, body, right } => {
            out.push_str(&format!("Delimited({left:?},{right:?},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::LargeOp { symbol, limits } => {
            out.push_str(&format!("LargeOp({symbol:?},{limits:?})"));
        }
        NodeKind::Accent { base, symbol } => {
            out.push_str(&format!("Accent({symbol:?},"));
            write_dump(model, *base, out);
            out.push(')');
        }
        NodeKind::Text(text) => out.push_str(&format!("Text({text:?})")),
        NodeKind::Composition(text) => out.push_str(&format!("Composition({text:?})")),
        NodeKind::Function { name, limits } => {
            out.push_str(&format!("Function({name:?},{limits:?})"));
        }
        NodeKind::Space(kind) => out.push_str(&format!("Space({kind:?})")),
        NodeKind::Hspace(length) => out.push_str(&format!("Hspace({length:?})")),
        NodeKind::Cancel { body, up, down } => {
            out.push_str(&format!("Cancel(up={up},down={down})"));
            out.push('(');
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Overline { body } => {
            out.push_str("Overline(");
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Underline { body } => {
            out.push_str("Underline(");
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Middle { symbol } => out.push_str(&format!("Middle({symbol:?})")),
        NodeKind::SizedDelimiter {
            symbol,
            size,
            class,
        } => out.push_str(&format!("Sized({symbol:?},{size:?},{class:?})")),
        NodeKind::Phantom { body, kind } => {
            out.push_str(&format!("Phantom({kind:?},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Smash { body, top, bottom } => {
            out.push_str(&format!("Smash({top},{bottom},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Boxed { body } => {
            out.push_str("Boxed(");
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::OverUnder { base, over, under } => {
            out.push_str("OverUnder(");
            write_dump(model, *base, out);
            for part in [over, under] {
                out.push(',');
                match part {
                    Some(part) => write_dump(model, *part, out),
                    None => out.push('-'),
                }
            }
            out.push(')');
        }
        NodeKind::Table {
            rows,
            columns,
            alignment,
            cell_style,
            column_rules,
            column_spacing,
        } => {
            out.push_str(&format!(
                "Table({columns},{alignment:?},{cell_style:?},{column_rules:?},{column_spacing:?},"
            ));
            for row in rows {
                write_dump(model, *row, out);
                out.push(',');
            }
            out.push(')');
        }
        NodeKind::TableRow { cells } => {
            out.push_str("TableRow(");
            for cell in cells {
                write_dump(model, *cell, out);
                out.push(',');
            }
            out.push(')');
        }
        NodeKind::Styled { size, body } => {
            out.push_str(&format!("Styled({size:?},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Variant { variant, body } => {
            out.push_str(&format!("Variant({variant:?},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Color { color, body } => {
            out.push_str(&format!("Color({color:?},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::ColorBox {
            background,
            border,
            body,
        } => {
            out.push_str(&format!("ColorBox({background:?},{border:?},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::Class {
            class,
            body,
            limits,
        } => {
            out.push_str(&format!("Class({class:?},{limits:?},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::UnderArrow { symbol, body } => {
            out.push_str(&format!("UnderArrow({symbol:?},"));
            write_dump(model, *body, out);
            out.push(')');
        }
        NodeKind::MathChoice {
            display,
            text,
            script,
            script_script,
        } => {
            out.push_str("MathChoice(");
            for choice in [display, text, script, script_script] {
                write_dump(model, *choice, out);
                out.push(',');
            }
            out.push(')');
        }
        NodeKind::Error(text) => out.push_str(&format!("Error({text:?})")),
    }
}

#[test]
fn constructs_keep_their_source_spans() {
    let document = parse("x^2+\\frac{a}{b}");
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let model = &document.model;
    let root = document.root();
    let children = row(model, root);
    assert_eq!(children.len(), 3);
    assert_eq!(model.span(children[0]), Span::new(0, 3));
    assert_eq!(model.span(children[1]), Span::new(3, 4));
    assert_eq!(model.span(children[2]), Span::new(4, 15));
    assert_eq!(model.span(root), Span::new(0, 15));

    let NodeKind::Scripts { base, sup, .. } = model.kind(children[0]) else {
        panic!("expected scripts");
    };
    assert_eq!(model.span(*base), Span::new(0, 1));
    assert_eq!(model.span(sup.expect("superscript")), Span::new(2, 3));
}

#[test]
fn commands_map_to_the_model() {
    let document = parse("\\sqrt[3]{x}");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Radical { body, degree } = model.kind(row(model, root)[0]) else {
        panic!("expected a radical");
    };
    assert!(matches!(model.kind(*body), NodeKind::Atom(atom) if atom.symbol == 'x'));
    let degree = degree.expect("an index");
    assert!(matches!(
        row(model, degree).first().map(|id| model.kind(*id)),
        Some(NodeKind::Atom(atom)) if atom.symbol == '3'
    ));

    let document = parse("\\left[\\frac{a}{b}\\right]");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Delimited { left, right, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected delimiters");
    };
    assert_eq!((*left, *right), (Some('['), Some(']')));

    let document = parse("\\sum_{i=1}^{n}");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Scripts { base, sup, sub } = model.kind(row(model, root)[0]) else {
        panic!("expected scripts");
    };
    assert!(matches!(
        model.kind(*base),
        NodeKind::LargeOp {
            symbol: '∑',
            limits: Limits::Default {
                display_limits: true
            }
        }
    ));
    assert!(sub.is_some());
    assert!(sup.is_some());

    let document = parse("\\alpha\\le\\beta");
    let model = &document.model;
    let root = document.root();
    let symbols: Vec<char> = row(model, root)
        .iter()
        .filter_map(|id| match model.kind(*id) {
            NodeKind::Atom(atom) => Some(atom.symbol),
            _ => None,
        })
        .collect();
    assert_eq!(symbols, vec!['α', '≤', 'β']);

    let document = parse("\\int_0^1");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Scripts { base, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected scripts");
    };
    assert!(matches!(
        model.kind(*base),
        NodeKind::LargeOp {
            symbol: '∫',
            limits: Limits::Default {
                display_limits: false
            }
        }
    ));

    let document = parse("\\text{if }x");
    let model = &document.model;
    let root = document.root();
    let children = row(model, root);
    assert!(matches!(model.kind(children[0]), NodeKind::Text(text) if text == "if "));
    assert!(matches!(model.kind(children[1]), NodeKind::Atom(atom) if atom.symbol == 'x'));

    let document = parse("\\text{a {b} c}");
    let model = &document.model;
    let root = document.root();
    assert!(matches!(
        row(model, root).first().map(|id| model.kind(*id)),
        Some(NodeKind::Text(text)) if text == "a b c"
    ));
}

#[test]
fn text_mode_decodes_escapes_and_keeps_the_source() {
    let source = "\\text{100\\% \\{x\\} \\textbackslash~y}";
    let document = parse(source);
    let NodeKind::Text(text) = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected a text node");
    };
    assert_eq!(text, "100% {x} \\ y");
    assert_eq!(document.to_latex(), source);
    assert_eq!(
        to_latex(&document.model, document.root()),
        "\\text{100\\% \\{x\\} \\textbackslash{} y}"
    );
}

#[test]
fn diagnostics_say_whether_input_is_incomplete_or_refused() {
    let incomplete = parse("\\frac{1}{1+");
    assert!(incomplete.has_errors());
    assert!(
        incomplete
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.kind == DiagnosticKind::Incomplete),
        "{:?}",
        incomplete.diagnostics
    );

    let unknown = parse("\\frc");
    assert!(
        unknown
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::UnknownCommand),
        "{:?}",
        unknown.diagnostics
    );
}

#[test]
fn errors_recover_in_place() {
    let document = parse("a+\\unknown+b");
    assert_eq!(document.diagnostics.len(), 1);
    let model = &document.model;
    let root = document.root();
    let children = row(model, root);
    assert_eq!(children.len(), 5);
    assert!(matches!(model.kind(children[2]), NodeKind::Error(text) if text == "\\unknown"));
    assert_eq!(model.span(children[2]), Span::new(2, 10));
    assert!(matches!(model.kind(children[4]), NodeKind::Atom(atom) if atom.symbol == 'b'));

    let document = parse("\\frac{a}");
    assert!(!document.diagnostics.is_empty());
    let model = &document.model;
    let root = document.root();
    let NodeKind::Fraction { denominator, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected a fraction");
    };
    assert!(matches!(
        model.kind(*denominator),
        NodeKind::Row(children) if children.is_empty()
    ));

    let document = parse("\\left( x");
    assert!(
        document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("\\right"))
    );
    let model = &document.model;
    let root = document.root();
    let NodeKind::Delimited { body, right, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected delimiters");
    };
    assert!(right.is_none());
    assert!(matches!(
        row(model, *body).first().map(|id| model.kind(*id)),
        Some(NodeKind::Atom(atom)) if atom.symbol == 'x'
    ));

    let document = parse("{a");
    assert!(
        document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unclosed"))
    );
    let model = &document.model;
    let root = document.root();
    let group = row(model, root)[0];
    assert!(matches!(
        row(model, group).first().map(|id| model.kind(*id)),
        Some(NodeKind::Atom(atom)) if atom.symbol == 'a'
    ));

    // Brace-free command chains must hit the depth budget, not the stack.
    let deep_commands = "\\sqrt".repeat(1000);
    assert!(
        parse(&deep_commands)
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("nested too deeply"))
    );
    let deep_groups = format!("{}a{}", "{".repeat(1000), "}".repeat(1000));
    assert!(
        parse(&deep_groups)
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("nested too deeply"))
    );
}

#[test]
fn writer_round_trips_the_grammar() {
    let sources = [
        "a+b=c",
        "x^2",
        "a_i",
        "x_1^2",
        "x^{2^3}",
        "\\frac{a}{b}",
        "\\frac{1}{x+1}",
        "\\frac{x^2}{y_1}",
        "\\sqrt{x}",
        "\\sqrt[3]{x}",
        "\\sqrt{\\frac{a}{b}}",
        "\\left(\\frac{a}{b}\\right)",
        "\\left[x^2\\right]",
        "\\left. x \\right|",
        "\\left\\langle x\\right\\rangle",
        "\\left\\{x\\right\\}",
        "\\left\\|x\\right.",
        "\\sum_{i=1}^{n}",
        "\\sum\\limits_{i=1}^{n}",
        "\\int\\nolimits_0^1",
        "\\lim\\limits_{x\\to 0}",
        "\\operatorname{rank}\\limits_x",
        "\\binom{n}{k}",
        "\\dbinom{n}{k}",
        "\\tbinom{n}{k}",
        "\\int_0^1",
        "\\alpha+\\beta\\le\\gamma",
        "x^2+a_i+\\frac{a+b}{2}=\\sqrt{c+d}",
        "{a}b",
        "a{+}b",
        "x^{{a}}",
        "\\frac{{a}}{b}",
        "\\dfrac{1}{2}+\\tfrac{3}{4}",
        "a\\not=b",
        "\\sum",
        "a\\unknown b",
        "\\hat{x}",
        "\\bar{x+y}",
        "\\vec{v}",
        "\\ddot{a}",
        "\\mathbb{R}",
        "\\mathbf{A}+\\mathcal{L}",
        "\\frac{\\hat{x}}{2}",
        "\\def\\RR{\\mathbb{R}}\\RR+1",
        "\\newcommand{\\sq}[1]{#1^2}\\sq{x}",
        "\\text{if }x",
        "\\text{a {b} c}+d",
        "\\text{100\\%}+\\text{a \\{b\\} c}",
        "\\text{\\textbackslash{}~\\textasciitilde{}~\\textasciicircum{}}",
        "\\text{$5 & 6 #7 _8}",
        "\\operatorname{rank} A",
        "a\\,b",
        "a\\;+b",
        "\\quad x",
        "\\bigl( x \\bigr)",
        "\\Biggl[ x \\Biggr]",
        "\\left(a\\middle|b\\right)",
        "\\sin x+\\log_2 y",
        "\\lim_{x\\to 0}",
        "\\operatorname{rank} A",
        "\\operatorname*{argmax}_{x}",
        "\\overline{x+y}",
        "\\underline{x+y}",
        "a \\atop b",
        "{a \\over b}",
        "{n \\choose k}",
        "\\phantom{x}+\\hphantom{y}+\\vphantom{z}",
        "\\smash[t]{x^2}+\\smash[b]{\\frac{a}{b}}",
        "\\boxed{x+y}",
        "\\overset{!}{=}+\\underset{x}{\\max}",
        "\\begin{matrix}a&b\\\\c&d\\end{matrix}",
        "\\begin{pmatrix}1&2\\\\3&4\\end{pmatrix}",
        "\\begin{bmatrix}a\\end{bmatrix}",
        "\\begin{cases}x&x>0\\\\0&x\\le 0\\end{cases}",
        "\\begin{matrix}a&b\\\\\\end{matrix}",
        "\\begin{aligned}a&=b\\\\c&=d\\end{aligned}",
        "\\begin{equation}a+b\\end{equation}",
        "\\begin{equation*}a=b\\end{equation*}",
        "\\begin{gather}a=b\\\\c=d\\end{gather}",
        "\\begin{gather*}a=b\\end{gather*}",
        "\\begin{gathered}a=b\\\\c=d\\end{gathered}",
        "\\begin{split}a&=b\\\\c&=d\\end{split}",
        "\\begin{dcases}a&b\\\\c&d\\end{dcases}",
        "\\begin{rcases}a&b\\end{rcases}",
        "\\begin{drcases}a&b\\end{drcases}",
        "\\begin{matrix*}[l]a&b\\\\c&d\\end{matrix*}",
        "\\begin{pmatrix*}[r]a&b\\end{pmatrix*}",
        "\\begin{bmatrix*}[c]a&b\\end{bmatrix*}",
        "\\begin{Bmatrix*}[l]a&b\\end{Bmatrix*}",
        "\\begin{vmatrix*}[r]a&b\\end{vmatrix*}",
        "\\begin{Vmatrix*}[c]a&b\\end{Vmatrix*}",
        "\\begin{smallmatrix}a&b\\\\c&d\\end{smallmatrix}",
        "\\begin{array}{l|c|r}a&b&c\\\\d&e&f\\end{array}",
        "\\begin{array}{|c|c|}a&b\\end{array}",
        "\\begin{array}{cc}a&b\\end{array}",
        "\\begin{subarray}{c}i<n\\\\n>0\\end{subarray}",
        "\\begin{alignat}{2}a&=b&c&=d\\end{alignat}",
        "\\begin{alignedat}{2}a&=b&c&=d\\end{alignedat}",
        "a\\displaystyle b+c",
        "{\\displaystyle a+b}c",
        "\\mathop{f}_x",
        "\\mathop{f}\\limits_x",
        "\\mathop{f}\\nolimits_x",
        "a\\mathbin{+}b",
        "\\mathrel{x}\\mathord{y}",
        "\\varlimsup_{x}",
        "\\varliminf_{x}",
        "\\varinjlim_{i}",
        "\\varprojlim_{i}",
        "\\underrightarrow{abc}",
        "\\underleftarrow{abc}",
        "\\operatorname{lim}\\displaylimits_x",
        "\\operatorname*{lim}\\displaylimits_x",
        "\\sum\\displaylimits_x",
        "\\int\\displaylimits_0^1",
        "\\stackrel{!}{=}",
        "\\substack{0<i<n\\\\i\\ne 1}",
        "\\cfrac{1}{1+\\cfrac{1}{x}}",
        "a\\equiv b\\pmod{n}",
        "a\\hspace{1em}b",
        "a\\hspace{0.5em}b",
        "a\\hspace{-2mu}b",
        "\\cancel{x}",
        "\\bcancel{x+y}",
        "\\xcancel{\\frac{a}{b}}",
        "\\genfrac{(}{)}{0pt}{}{a}{b}",
        "\\genfrac{}{}{}{2}{a}{b}",
        "\\genfrac{[}{]}{1mu}{0}{x}{y}",
        "\\genfrac{\\{}{\\}}{0.4em}{1}{n}{k}",
        "a\\xrightarrow{f}b",
        "a\\xrightarrow[n\\to\\infty]{f}b",
        "a\\xleftarrow{g}b",
        "\\overbrace{a+b}^{n}",
        "\\underbrace{a+b}_{n}",
        "a\\bmod b",
        "a\\mod n",
        "a\\pod{n}",
        "\\arg x",
        "\\ker A",
        "\\dim V",
        "\\deg p",
        "\\hom(A,B)",
        "\\argmin_{x}",
        "\\argmax_{x}",
        "\\injlim_{i}",
        "\\projlim_{i}",
        "\\plim_{n}",
        "\\operatornamewithlimits{arg}_{x}",
        "\\cdots+\\vdots+\\ddots",
        "\\dotsc\\dotsb\\dotsm\\dotsi\\dotso",
    ];
    for source in sources {
        let first = parse(source);
        // An unknown command round-trips as an error node, which would hide a
        // missing parser arm; every other source must be valid LaTeX.
        if !source.contains("\\unknown") {
            assert!(
                !first.has_errors(),
                "{source} must parse cleanly: {:?}",
                first.diagnostics
            );
        }
        let latex = to_latex(&first.model, first.root());
        let second = parse(&latex);
        assert_eq!(
            dump(&first.model, first.root()),
            dump(&second.model, second.root()),
            "{source} printed as {latex}"
        );
        assert_eq!(
            to_latex(&second.model, second.root()),
            latex,
            "the writer must be idempotent for {source}"
        );
    }
}

#[test]
fn mathchoice_keeps_four_branches_in_style_order() {
    let source = "\\mathchoice{d}{t}{s}{f}";
    let document = parse(source);
    assert!(!document.has_errors(), "{:?}", document.diagnostics);
    let model = &document.model;
    let NodeKind::MathChoice {
        display,
        text,
        script,
        script_script,
    } = model.kind(row(model, document.root())[0])
    else {
        panic!("expected a math choice");
    };
    for (id, expected) in [
        (display, 'd'),
        (text, 't'),
        (script, 's'),
        (script_script, 'f'),
    ] {
        assert!(
            matches!(model.kind(*id), NodeKind::Atom(atom) if atom.symbol == expected),
            "branch {expected} keeps its position"
        );
    }
    let written = document.to_latex();
    assert_eq!(written, "\\mathchoice {d}{t}{s}{f}");
    let round_trip = parse(&written);
    assert_eq!(
        dump(&document.model, document.root()),
        dump(&round_trip.model, round_trip.root())
    );
}

#[test]
fn mathstrut_is_a_vertical_paren_phantom() {
    let document = parse("a\\mathstrut b");
    assert!(!document.has_errors(), "{:?}", document.diagnostics);
    let model = &document.model;
    let root = document.root();
    let NodeKind::Phantom { body, kind } = model.kind(row(model, root)[1]) else {
        panic!("expected a phantom");
    };
    assert_eq!(*kind, math_core::PhantomKind::Vertical);
    assert!(matches!(model.kind(*body), NodeKind::Atom(atom) if atom.symbol == '('));
    assert_eq!(document.to_latex(), "a\\mathstrut b");
    assert_eq!(to_latex(model, root), "a\\vphantom {(}b");
}

#[test]
fn color_declarations_scope_to_the_group_and_nest() {
    let document = parse("\\color{red}a\\textcolor{blue}{b}c");
    let model = &document.model;
    let NodeKind::Color { color, body } = model.kind(row(model, document.root())[0]) else {
        panic!("expected the color declaration");
    };
    assert_eq!(*color, Color::rgb(255, 0, 0));
    let NodeKind::Row(children) = model.kind(*body) else {
        panic!("expected the declaration body");
    };
    assert_eq!(
        children.len(),
        3,
        "the declaration covers `a`, the textcolor and `c`"
    );
    let NodeKind::Color { color, .. } = model.kind(children[1]) else {
        panic!("expected the textcolor wrapper");
    };
    assert_eq!(*color, Color::rgb(0, 0, 255));
    assert_eq!(document.to_latex(), "\\color{red}a\\textcolor {blue}{b}c");
}

#[test]
fn color_and_style_switches_keep_their_source_order() {
    for (source, expected, style_is_outer) in [
        (
            "\\displaystyle\\color{red}a",
            "\\displaystyle \\color{red}a",
            true,
        ),
        (
            "\\color{red}\\displaystyle a",
            "\\color{red}\\displaystyle a",
            false,
        ),
    ] {
        let document = parse(source);
        let model = &document.model;
        let outer = model.kind(row(model, document.root())[0]);
        if style_is_outer {
            let NodeKind::Styled { body, .. } = outer else {
                panic!("expected the style wrapper first for {source}");
            };
            let NodeKind::Row(children) = model.kind(*body) else {
                panic!("expected a body row for {source}");
            };
            assert!(
                matches!(model.kind(children[0]), NodeKind::Color { .. }),
                "the color switch still wraps the atom"
            );
        } else {
            let NodeKind::Color { body, .. } = outer else {
                panic!("expected the color wrapper first for {source}");
            };
            assert!(
                matches!(model.kind(*body), NodeKind::Styled { .. }),
                "the style switch still wraps the atom"
            );
        }
        assert_eq!(document.to_latex(), expected);
    }
}

#[test]
fn textcolor_and_color_write_back_their_own_forms() {
    let textcolor = parse("\\textcolor{red}{x}");
    assert_eq!(textcolor.to_latex(), "\\textcolor {red}{x}");
    assert_eq!(
        to_latex(&textcolor.model, textcolor.root()),
        "\\textcolor {red}{x}"
    );

    let declaration = parse("\\color{red}x");
    assert_eq!(declaration.to_latex(), "\\color{red}x");
    let canonical = to_latex(&declaration.model, declaration.root());
    assert_eq!(canonical, "\\textcolor {red}{x}");
    let reparsed = parse(&canonical);
    assert_eq!(
        dump(&declaration.model, declaration.root()),
        dump(&reparsed.model, reparsed.root())
    );

    let hex = parse("\\textcolor{#1a2b3c}{x}");
    assert!(!hex.has_errors(), "{:?}", hex.diagnostics);
    assert_eq!(hex.to_latex(), "\\textcolor {#1a2b3c}{x}");

    // CSS names are case-insensitive, and xcolor accepts bare six-digit hex.
    let upper = parse("\\textcolor{Red}{x}");
    assert!(!upper.has_errors(), "{:?}", upper.diagnostics);
    assert_eq!(upper.to_latex(), "\\textcolor {red}{x}");
    let bare = parse("\\textcolor{ff0000}{x}");
    assert!(!bare.has_errors(), "{:?}", bare.diagnostics);
    assert_eq!(bare.to_latex(), "\\textcolor {red}{x}");
    let alias = parse("\\colorbox{aqua}{x}");
    assert!(!alias.has_errors(), "{:?}", alias.diagnostics);
    assert_eq!(alias.to_latex(), "\\colorbox {cyan}{x}");

    let unknown = parse("\\textcolor{bogus}{x}");
    assert_eq!(unknown.diagnostics.len(), 1, "an unknown name is reported");
}

#[test]
fn colorbox_arguments_are_text_and_write_back_their_commands() {
    let colorbox = parse("\\colorbox{red}{a+b}");
    assert!(!colorbox.has_errors(), "{:?}", colorbox.diagnostics);
    let NodeKind::ColorBox {
        background,
        border,
        body,
    } = colorbox
        .model
        .kind(row(&colorbox.model, colorbox.root())[0])
    else {
        panic!("expected a colorbox");
    };
    assert_eq!(*background, Color::rgb(255, 0, 0));
    assert_eq!(*border, None);
    assert!(
        matches!(colorbox.model.kind(*body), NodeKind::Text(text) if text == "a+b"),
        "the body is a text argument, not math"
    );
    assert_eq!(colorbox.to_latex(), "\\colorbox {red}{a+b}");
    assert_eq!(
        to_latex(&colorbox.model, colorbox.root()),
        "\\colorbox {red}{a+b}"
    );

    let framed = parse("\\fcolorbox{blue}{red}{x}");
    assert!(!framed.has_errors(), "{:?}", framed.diagnostics);
    let NodeKind::ColorBox {
        background,
        border,
        body,
    } = framed.model.kind(row(&framed.model, framed.root())[0])
    else {
        panic!("expected an fcolorbox");
    };
    assert_eq!(*border, Some(Color::rgb(0, 0, 255)));
    assert_eq!(*background, Color::rgb(255, 0, 0));
    assert!(matches!(framed.model.kind(*body), NodeKind::Text(text) if text == "x"));
    let written = framed.to_latex();
    assert_eq!(written, "\\fcolorbox {blue}{red}{x}");
    let reparsed = parse(&written);
    assert_eq!(
        dump(&framed.model, framed.root()),
        dump(&reparsed.model, reparsed.root())
    );

    let unknown = parse("\\colorbox{bogus}{x}");
    assert_eq!(unknown.diagnostics.len(), 1, "an unknown name is reported");
}

#[test]
fn a_colorbox_without_a_body_writes_an_error_back() {
    let document = parse("\\colorbox{red}");
    assert!(document.has_errors());
    let written = document.to_latex();
    assert_eq!(written, "\\colorbox {red}");
    assert!(
        parse(&written).has_errors(),
        "the missing body must stay an error, not become literal text"
    );
}

#[test]
fn a_no_op_cancel_or_smash_writes_its_body_alone() {
    let (cancel, root) = Model::build(|m| {
        let x = m.symbol('x');
        m.cancel(x, false, false)
    });
    assert_eq!(to_latex(&cancel, root), "x");

    let (smash, root) = Model::build(|m| {
        let x = m.symbol('x');
        m.smash(x, false, false)
    });
    assert_eq!(to_latex(&smash, root), "x");
}

#[test]
fn a_foreign_arrow_keeps_its_symbol_and_labels() {
    let (under, root) = Model::build(|m| {
        let x = m.symbol('x');
        m.alloc(
            NodeKind::UnderArrow {
                symbol: '⇒',
                body: x,
            },
            Span::default(),
        )
    });
    assert_eq!(to_latex(&under, root), "\\underset {⇒}{x}");

    let (arrow, root) = Model::build(|m| {
        let a = m.symbol('a');
        let b = m.symbol('b');
        m.arrow('⇒', Some(a), Some(b))
    });
    let written = to_latex(&arrow, root);
    assert_eq!(written, "\\overset {a}{\\underset {b}{⇒}}");
    assert!(!parse(&written).has_errors(), "{written}");
}

#[test]
fn a_sized_delimiter_outside_the_command_classes_keeps_its_size() {
    let (model, root) =
        Model::build(|m| m.sized_delimiter('(', DelimiterSize::Big, AtomClass::Bin));
    assert_eq!(to_latex(&model, root), "\\big (");
}

#[test]
fn operator_suffixes_select_the_limits_placement() {
    let document = parse("\\int\\limits_0^1");
    let NodeKind::Scripts { base, .. } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected scripts");
    };
    assert!(matches!(
        document.model.kind(*base),
        NodeKind::LargeOp {
            symbol: '∫',
            limits: Limits::AboveBelow
        }
    ));

    let document = parse("\\sum\\nolimits_{i=1}^n");
    let NodeKind::Scripts { base, .. } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected scripts");
    };
    assert!(matches!(
        document.model.kind(*base),
        NodeKind::LargeOp {
            symbol: '∑',
            limits: Limits::Beside
        }
    ));
}

#[test]
fn named_operator_tables_choose_the_placement() {
    for (source, expected_name, expected_limits) in [
        (
            "\\arg",
            "arg",
            Limits::Default {
                display_limits: false,
            },
        ),
        (
            "\\argmin",
            "arg min",
            Limits::Default {
                display_limits: true,
            },
        ),
        ("\\plim", "plim", Limits::AboveBelow),
    ] {
        let document = parse(source);
        let NodeKind::Function { name, limits } = document
            .model
            .kind(row(&document.model, document.root())[0])
        else {
            panic!("expected a function for {source}");
        };
        assert_eq!(name.as_str(), expected_name);
        assert_eq!(*limits, expected_limits);
    }

    // `\plim` is above/below in every style; the command says so itself.
    let document = parse("\\plim_n");
    assert_eq!(to_latex(&document.model, document.root()), "\\plim _{n}");
}

#[test]
fn operatornamewithlimits_is_operatorname_star() {
    let alias = parse("\\operatornamewithlimits{arg}_x");
    let star = parse("\\operatorname*{arg}_x");
    assert_eq!(
        dump(&alias.model, alias.root()),
        dump(&star.model, star.root())
    );
}

#[test]
fn binomial_commands_write_as_binomials() {
    for (source, expected) in [
        ("\\binom{n}{k}", "\\binom {n}{k}"),
        ("\\dbinom{n}{k}", "\\dbinom {n}{k}"),
        ("\\tbinom{n}{k}", "\\tbinom {n}{k}"),
        // The story the commands expand to writes back in binomial form.
        (
            "\\left(\\genfrac{}{}{0pt}{}{a}{b}\\right)",
            "\\binom {{a}}{{b}}",
        ),
    ] {
        let document = parse(source);
        assert!(
            !document.has_errors(),
            "{source} must parse cleanly: {:?}",
            document.diagnostics
        );
        assert_eq!(document.to_latex(), expected, "{source}");
    }
}

#[test]
fn a_live_composition_is_not_written() {
    let (model, root) = Model::build(|m| m.composition("xy"));
    assert_eq!(to_latex(&model, root), "");
    assert!(matches!(model.node(root).origin, Origin::Synthetic));
}

#[test]
fn macro_expansion_marks_its_nodes_and_keeps_arguments_editable() {
    let document = parse("\\def\\R{\\mathbb{R}}\\R+x");
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let model = &document.model;
    let root = document.root();
    let items = row(model, root);
    let Origin::Macro { definition, call } = model.node(items[0]).origin else {
        panic!("the expanded atom must carry its macro origin");
    };
    assert!(
        definition.start < call.start,
        "the definition precedes the call"
    );
    assert_eq!(model.span(items[0]), call);
    assert_eq!(model.node(items[2]).origin, Origin::Source);

    // An argument is substituted with its own token, so it stays editable
    // while the rest of the body is marked.
    let document = parse("\\newcommand{\\sq}[1]{#1^2}\\sq{x}+y");
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let model = &document.model;
    let root = document.root();
    let items = row(model, root);
    let NodeKind::Scripts { base, sup, .. } = model.kind(items[0]) else {
        panic!("expected the expanded script");
    };
    assert_eq!(model.node(*base).origin, Origin::Source);
    let script = sup.expect("the body provides a superscript");
    assert!(matches!(model.node(script).origin, Origin::Macro { .. }));
    assert_eq!(model.node(items[2]).origin, Origin::Source);
}

#[test]
fn macros_expand_to_the_model() {
    let document = parse("\\def\\RR{\\mathbb{R}}\\RR^2");
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let model = &document.model;
    let root = document.root();
    let NodeKind::Scripts { base, sup, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected scripts");
    };
    assert!(matches!(
        model.kind(*base),
        NodeKind::Variant {
            variant: Variant::DoubleStruck,
            body,
        } if matches!(model.kind(*body), NodeKind::Atom(atom) if atom.symbol == 'R')
    ));
    assert!(sup.is_some());

    let document = parse("\\newcommand{\\pair}[2]{\\left(#1,#2\\right)}\\pair{a}{b}");
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let model = &document.model;
    let root = document.root();
    let NodeKind::Delimited { body, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected delimiters");
    };
    let body = row(model, *body);
    assert_eq!(body.len(), 3);
    assert!(matches!(model.kind(body[0]), NodeKind::Atom(atom) if atom.symbol == 'a'));
    assert!(matches!(model.kind(body[2]), NodeKind::Atom(atom) if atom.symbol == 'b'));

    let document = parse("\\def\\sq#1{#1^2}\\sq{x}");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Scripts { base, sup, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected scripts");
    };
    assert!(matches!(model.kind(*base), NodeKind::Atom(atom) if atom.symbol == 'x'));
    assert!(matches!(
        sup.map(|sup| model.kind(sup)),
        Some(NodeKind::Atom(atom)) if atom.symbol == '2'
    ));

    // Text inside a macro body cannot recover its source bytes; it becomes an
    // error node instead of slicing across the call span.
    let document = parse("\\def\\foo{\\text{abc}}\\foo");
    assert!(
        document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("text mode inside"))
    );
    assert!(document.has_errors());

    let document = parse("\\def");
    assert!(
        document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("missing macro name"))
    );

    // Self-recursion ends at the expansion budget with an error node.
    let document = parse("\\def\\a{\\a}\\a");
    assert!(
        document
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("macro expansion limit"))
    );
    assert!(document.has_errors());
}

#[test]
fn matrix_environments_build_rows_and_cells() {
    let document = parse("\\begin{pmatrix}a&b\\\\c&d\\end{pmatrix}");
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let model = &document.model;
    let root = document.root();
    let NodeKind::Delimited { left, right, body } = model.kind(row(model, root)[0]) else {
        panic!("expected the pmatrix fences");
    };
    assert_eq!((*left, *right), (Some('('), Some(')')));
    let NodeKind::Row(content) = model.kind(*body) else {
        panic!("expected a row inside the fences");
    };
    let NodeKind::Table {
        rows,
        columns,
        alignment,
        cell_style,
        ..
    } = model.kind(content[0])
    else {
        panic!("expected a table");
    };
    assert_eq!(*columns, 2);
    assert!(alignment.is_empty(), "matrix columns default to centred");
    // Layout uses this even in display style, where TeX still sets matrix
    // cells in text style.
    assert_eq!(
        *cell_style,
        Some(Style::TEXT),
        "matrix cells are text style"
    );
    assert_eq!(rows.len(), 2);
    let NodeKind::TableRow { cells } = model.kind(rows[0]) else {
        panic!("expected a table row");
    };
    assert_eq!(cells.len(), 2);
    assert!(
        matches!(model.kind(row(model, cells[0])[0]), NodeKind::Atom(atom) if atom.symbol == 'a')
    );

    // A short row is padded so every row has the table's column count.
    let document = parse("\\begin{matrix}a&b\\\\c\\end{matrix}");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Table { rows, columns, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected a table");
    };
    assert_eq!(*columns, 2);
    let NodeKind::TableRow { cells } = model.kind(rows[1]) else {
        panic!("expected a table row");
    };
    assert_eq!(cells.len(), 2);
    assert!(matches!(
        model.kind(cells[1]),
        NodeKind::Row(children) if children.is_empty()
    ));

    // `cases` aligns its columns left.
    let document = parse("\\begin{cases}x&x>0\\end{cases}");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Delimited { left, right, body } = model.kind(row(model, root)[0]) else {
        panic!("expected the cases fence");
    };
    assert_eq!((*left, *right), (Some('{'), None));
    let NodeKind::Row(content) = model.kind(*body) else {
        panic!("expected a row inside the fence");
    };
    let NodeKind::Table {
        columns, alignment, ..
    } = model.kind(content[0])
    else {
        panic!("expected a table");
    };
    assert_eq!(*columns, 2);
    assert_eq!(alignment, &vec![math_core::Alignment::Left; 2]);

    // `aligned` alternates a right- and a left-aligned column.
    let document = parse("\\begin{aligned}a&=b\\\\c&=d\\end{aligned}");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Table {
        columns, alignment, ..
    } = model.kind(row(model, root)[0])
    else {
        panic!("expected a table");
    };
    assert_eq!(*columns, 2);
    assert_eq!(
        alignment,
        &vec![math_core::Alignment::Right, math_core::Alignment::Left]
    );
}

#[test]
fn environment_variants_keep_their_table_and_name() {
    // `equation` unwraps to its formula; the name only survives the writer.
    let document = parse("\\begin{equation}a+b\\end{equation}");
    let first = row(&document.model, document.root())[0];
    assert!(matches!(
        document.model.kind(first),
        NodeKind::Row(children) if children.len() == 3
    ));
    assert!(document.to_latex().starts_with("\\begin {equation}a+b"));

    // `gather` is one centred column; `split` alternates like `aligned`.
    let document = parse("\\begin{gather}a=b\\\\c=d\\end{gather}");
    let NodeKind::Table {
        columns,
        alignment,
        rows,
        cell_style,
        ..
    } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected a gather table");
    };
    assert_eq!(*columns, 1);
    assert!(alignment.is_empty(), "gather columns default to centred");
    assert_eq!(
        *cell_style,
        Some(Style::DISPLAY),
        "gather cells are display style"
    );
    assert_eq!(rows.len(), 2);
    assert!(document.to_latex().starts_with("\\begin {gather}"));

    let document = parse("\\begin{split}a&=b\\\\c&=d\\end{split}");
    let NodeKind::Table { alignment, .. } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected a split table");
    };
    assert_eq!(
        alignment,
        &vec![math_core::Alignment::Right, math_core::Alignment::Left]
    );
    assert!(document.to_latex().starts_with("\\begin {split}"));

    // The cases family differs in which fence it carries and in its cell
    // style; `dcases` keeps its name even though its table is shaped like
    // `cases`.
    for (source, left, right, name, style) in [
        (
            "\\begin{cases}a&b\\end{cases}",
            Some('{'),
            None,
            "cases",
            Style::TEXT,
        ),
        (
            "\\begin{dcases}a&b\\end{dcases}",
            Some('{'),
            None,
            "dcases",
            Style::DISPLAY,
        ),
        (
            "\\begin{rcases}a&b\\end{rcases}",
            None,
            Some('}'),
            "rcases",
            Style::TEXT,
        ),
        (
            "\\begin{drcases}a&b\\end{drcases}",
            Some('{'),
            Some('}'),
            "drcases",
            Style::DISPLAY,
        ),
    ] {
        let document = parse(source);
        let NodeKind::Delimited {
            left: actual_left,
            right: actual_right,
            body,
        } = document
            .model
            .kind(row(&document.model, document.root())[0])
        else {
            panic!("expected the {name} fence");
        };
        assert_eq!((*actual_left, *actual_right), (left, right));
        let NodeKind::Row(content) = document.model.kind(*body) else {
            panic!("expected a row inside the {name} fence");
        };
        let NodeKind::Table {
            alignment,
            cell_style,
            ..
        } = document.model.kind(content[0])
        else {
            panic!("expected the {name} table");
        };
        assert_eq!(alignment, &vec![math_core::Alignment::Left; 2]);
        assert_eq!(*cell_style, Some(style), "{name} cell style");
        assert!(
            document
                .to_latex()
                .starts_with(&format!("\\begin {{{name}}}")),
            "the writer must keep the {name} name"
        );
    }

    // A starred matrix's optional argument applies to every column.
    let document = parse("\\begin{matrix*}[r]a&b\\end{matrix*}");
    let NodeKind::Table {
        columns, alignment, ..
    } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected a starred matrix table");
    };
    assert_eq!(*columns, 2);
    assert_eq!(alignment, &vec![math_core::Alignment::Right; 2]);
    assert!(document.to_latex().starts_with("\\begin {matrix*}[r]"));

    // `smallmatrix` is a matrix whose cells layout in script style.
    let document = parse("\\begin{smallmatrix}a&b\\\\c&d\\end{smallmatrix}");
    let NodeKind::Table {
        columns,
        cell_style,
        ..
    } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected a smallmatrix table");
    };
    assert_eq!(*columns, 2);
    assert_eq!(*cell_style, Some(Style::SCRIPT));
    assert!(document.to_latex().starts_with("\\begin {smallmatrix}"));
}

#[test]
fn substack_writes_back_as_the_command() {
    let document = parse("\\substack{0<i<n\\\\i\\ne 1}");
    let latex = document.to_latex();
    assert_eq!(latex, "\\substack {0<i<n \\\\ i\\ne 1}");
    assert_eq!(parse(&latex).to_latex(), latex);
}

#[test]
fn spacing_and_operators_reach_the_model() {
    let document = parse("a\\,b");
    let model = &document.model;
    let root = document.root();
    let children = row(model, root);
    assert_eq!(children.len(), 3);
    assert!(matches!(
        model.kind(children[1]),
        NodeKind::Space(SpaceKind::Thin)
    ));

    let document = parse("\\bigl(x\\bigr)");
    let model = &document.model;
    let root = document.root();
    let children = row(model, root);
    assert!(matches!(
        model.kind(children[0]),
        NodeKind::SizedDelimiter {
            symbol: '(',
            class: AtomClass::Open,
            ..
        }
    ));
    assert!(matches!(
        model.kind(children[2]),
        NodeKind::SizedDelimiter {
            symbol: ')',
            class: AtomClass::Close,
            ..
        }
    ));

    let document = parse("\\left(a\\middle|b\\right)");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Delimited { body, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected delimiters");
    };
    assert!(matches!(
        model.kind(row(model, *body)[1]),
        NodeKind::Middle { symbol: '|' }
    ));

    let document = parse("\\sin x+\\lim_{x\\to 0}");
    let model = &document.model;
    let root = document.root();
    let children = row(model, root);
    assert!(matches!(
        model.kind(children[0]),
        NodeKind::Function { name, limits: Limits::Default { display_limits: false } } if name == "sin"
    ));
    let NodeKind::Scripts { base, .. } = model.kind(children[3]) else {
        panic!("expected scripts");
    };
    assert!(matches!(
        model.kind(*base),
        NodeKind::Function { name, limits: Limits::Default { display_limits: true } } if name == "lim"
    ));

    let document = parse("\\operatorname*{argmax}_x");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Scripts { base, .. } = model.kind(row(model, root)[0]) else {
        panic!("expected scripts");
    };
    assert!(matches!(
        model.kind(*base),
        NodeKind::Function { name, limits: Limits::Default { display_limits: true } } if name == "argmax"
    ));

    let document = parse("a \\atop b");
    let model = &document.model;
    let root = document.root();
    assert!(matches!(
        model.kind(row(model, root)[0]),
        NodeKind::Fraction {
            rule: FractionRule::None,
            ..
        }
    ));

    let document = parse("{n \\choose k}");
    let model = &document.model;
    let root = document.root();
    let NodeKind::Row(group) = model.kind(row(model, root)[0]) else {
        panic!("expected a group");
    };
    assert!(matches!(
        model.kind(group[0]),
        NodeKind::Delimited {
            left: Some('('),
            right: Some(')'),
            ..
        }
    ));
}

#[test]
fn exported_spans_cover_the_text_each_node_wrote() {
    let document = parse("a+\\frac{b}{c}");
    let (text, spans) = to_latex_with_spans(&document.model, document.root());
    let span_of = |wanted: char| {
        spans
            .iter()
            .find(|(node, _)| {
                matches!(document.model.kind(*node), NodeKind::Atom(atom) if atom.symbol == wanted)
            })
            .map(|(_, span)| *span)
            .unwrap_or_else(|| panic!("no span for {wanted}"))
    };
    let b = span_of('b');
    assert_eq!(&text[b.start..b.end], "b");
    let fraction = spans
        .iter()
        .find(|(node, _)| matches!(document.model.kind(*node), NodeKind::Fraction { .. }))
        .map(|(_, span)| *span)
        .expect("a fraction span");
    let printed = &text[fraction.start..fraction.end];
    assert!(printed.starts_with("\\frac "), "{printed}");
    assert!(printed.ends_with('}'), "{printed}");
}

#[test]
fn the_writer_reports_each_approximation() {
    let (model, root) = Model::build(|m| {
        let math = m.symbol('x');
        let colorbox = m.color_box(Color::rgb(255, 0, 0), None, math);
        let arrow = m.arrow('\u{21D2}', None, None);
        let sized = m.sized_delimiter('(', DelimiterSize::Big, AtomClass::Bin);
        let accent_base = m.symbol('x');
        let accent = m.accent(accent_base, '^');
        let cancel_body = m.symbol('y');
        let cancel = m.cancel(cancel_body, false, false);
        let variant_body = m.symbol('z');
        let variant = m.variant(Variant::Normal, variant_body);
        let color_body = m.symbol('w');
        let color = m.color(Color::rgba(1, 2, 3, 4), color_body);
        m.row(vec![colorbox, arrow, sized, accent, cancel, variant, color])
    });

    let (text, report) = to_latex_with_report(&model, root);
    let kinds: Vec<LossKind> = report.losses.iter().map(|loss| loss.kind).collect();
    assert_eq!(
        kinds,
        vec![
            LossKind::DroppedBody,
            LossKind::LostStretch,
            LossKind::LostClass,
            LossKind::SubstitutedMark,
            LossKind::WrapperDropped,
            LossKind::WrapperDropped,
            LossKind::Portability,
        ]
    );
    assert_eq!(report.visible().count(), 5);
    assert_eq!(to_latex(&model, root), text);
}

#[test]
fn a_document_keeps_the_symbol_spelling_it_was_written_with() {
    let document = parse("±+\\pm+\\le");
    assert_eq!(document.to_latex(), "±+\\pm +\\le ");
    // The canonical writer still exports what the model means, not the token.
    assert_eq!(to_latex(&document.model, document.root()), "±+±+≤");
}

#[test]
fn command_shapes_describe_what_the_parser_expects() {
    assert_eq!(command_shape("alpha"), CommandShape::None);
    assert_eq!(command_shape("sum"), CommandShape::None);
    assert_eq!(command_shape("bmod"), CommandShape::None);
    assert_eq!(command_shape("frac"), CommandShape::TwoMathArguments);
    assert_eq!(command_template("frac").as_deref(), Some("\\frac{}{}"));
    assert_eq!(command_shape("binom"), CommandShape::TwoMathArguments);
    assert_eq!(command_template("dbinom").as_deref(), Some("\\dbinom{}{}"));
    assert_eq!(command_shape("hat"), CommandShape::MathArgument);
    assert_eq!(command_template("hat").as_deref(), Some("\\hat{}"));
    assert_eq!(command_shape("mathbb"), CommandShape::VariantArgument);
    assert_eq!(command_template("mathbb").as_deref(), Some("\\mathbb{}"));
    assert_eq!(command_shape("left"), CommandShape::Delimiter);
    assert_eq!(command_shape("bigl"), CommandShape::Delimiter);
    assert_eq!(command_shape("sqrt"), CommandShape::OptionalMathArgument);
    assert_eq!(command_shape("smash"), CommandShape::OptionalFlagArgument);
    assert_eq!(
        command_shape("xrightarrow"),
        CommandShape::OptionalUnderArgument
    );
    assert_eq!(command_shape("operatorname"), CommandShape::NameArgument);
    assert_eq!(
        command_shape("operatornamewithlimits"),
        CommandShape::NameArgument
    );
    assert_eq!(command_shape("mod"), CommandShape::MathArgument);
    assert_eq!(command_template("pod").as_deref(), Some("\\pod{}"));
    assert_eq!(command_shape("begin"), CommandShape::Environment);
    assert_eq!(command_shape("hspace"), CommandShape::Length);
    assert_eq!(command_shape("not"), CommandShape::SymbolArgument);
    assert_eq!(command_shape("genfrac"), CommandShape::Special);
    assert_eq!(command_shape("mathstrut"), CommandShape::None);
    assert_eq!(command_shape("mathchoice"), CommandShape::Special);
    assert_eq!(command_shape("colorbox"), CommandShape::Special);
    assert_eq!(command_shape("fcolorbox"), CommandShape::Special);
    assert_eq!(command_shape("nope"), CommandShape::Unknown);
}

#[test]
fn array_and_alignat_arguments_shape_the_table() {
    // `array` takes its columns and vertical rules from a preamble and leaves
    // the cells in the surrounding style.
    let document = parse("\\begin{array}{l|c|r}a&b\\\\c&d\\end{array}");
    let NodeKind::Table {
        columns,
        alignment,
        cell_style,
        column_rules,
        column_spacing,
        ..
    } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected an array table");
    };
    assert_eq!(*columns, 3);
    assert_eq!(
        alignment,
        &vec![Alignment::Left, Alignment::Center, Alignment::Right]
    );
    assert_eq!(*cell_style, None, "array inherits the surrounding style");
    assert_eq!(column_rules, &vec![0, 1, 1, 0]);
    assert_eq!(*column_spacing, ColumnSpacing::Spaced);
    assert!(
        document.to_latex().starts_with("\\begin {array}{l|c|r}"),
        "the writer must keep the column specification"
    );

    // `alignat` takes a pair count and sets its columns flush, unlike `aligned`.
    let document = parse("\\begin{alignedat}{2}a&=b&c&=d\\end{alignedat}");
    let NodeKind::Table {
        columns,
        alignment,
        cell_style,
        column_rules,
        column_spacing,
        ..
    } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected an alignedat table");
    };
    assert_eq!(*columns, 4);
    assert_eq!(
        alignment,
        &vec![
            Alignment::Right,
            Alignment::Left,
            Alignment::Right,
            Alignment::Left
        ]
    );
    assert_eq!(*cell_style, Some(Style::DISPLAY));
    assert!(column_rules.is_empty());
    assert_eq!(*column_spacing, ColumnSpacing::Flush);
    assert!(document.to_latex().starts_with("\\begin {alignedat}{2}"));
    assert!(
        to_latex(&document.model, document.root()).starts_with("\\begin {alignedat}{2}"),
        "the canonical writer distinguishes alignedat from aligned"
    );

    let aligned = parse("\\begin{aligned}a&=b\\end{aligned}");
    assert!(to_latex(&aligned.model, aligned.root()).starts_with("\\begin {aligned}"));
}

#[test]
fn style_switch_scopes_to_the_end_of_its_group() {
    // `\displaystyle` governs the rest of the group; `c` is outside it.
    let document = parse("{\\displaystyle a+b}c");
    let outer = row(&document.model, document.root());
    assert_eq!(outer.len(), 2, "the `c` sits outside the group");
    assert!(matches!(document.model.kind(outer[1]), NodeKind::Atom(_)));
    let NodeKind::Row(group) = document.model.kind(outer[0]) else {
        panic!("expected the braced group");
    };
    let NodeKind::Styled { size, body } = document.model.kind(group[0]) else {
        panic!("expected a style wrapper");
    };
    assert_eq!(*size, Size::Display);
    let NodeKind::Row(styled) = document.model.kind(*body) else {
        panic!("expected the styled body row");
    };
    assert_eq!(styled.len(), 3, "the switch covers `a+b`");
    assert!(document.to_latex().starts_with("{\\displaystyle a+b}c"));

    // A second switch nests inside the first: it governs the rest while the
    // earlier switch still wraps it, and the inner level wins where they
    // overlap. A color declaration shares this scope rule, which is why the
    // declarations cannot become siblings.
    let document = parse("\\displaystyle a\\textstyle b");
    let children = row(&document.model, document.root());
    let NodeKind::Styled { size, body } = document.model.kind(children[0]) else {
        panic!("expected the outer style wrapper");
    };
    assert_eq!(*size, Size::Display);
    let NodeKind::Row(inner) = document.model.kind(*body) else {
        panic!("expected the outer body row");
    };
    assert_eq!(inner.len(), 2, "`a` and the inner switch");
    let NodeKind::Styled { size, .. } = document.model.kind(inner[1]) else {
        panic!("expected the inner style wrapper");
    };
    assert_eq!(*size, Size::Text);
    assert_eq!(document.to_latex(), "\\displaystyle a\\textstyle b");
}

#[test]
fn class_wrappers_keep_their_class_and_limits() {
    let document = parse("\\mathop{f}_x");
    let NodeKind::Scripts { base, .. } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected scripts");
    };
    let NodeKind::Class { class, limits, .. } = document.model.kind(*base) else {
        panic!("expected a class wrapper");
    };
    assert_eq!(*class, AtomClass::Op);
    assert_eq!(
        *limits,
        Limits::Default {
            display_limits: true
        },
        "`\\mathop` is a display-limits operator"
    );
    assert!(document.to_latex().starts_with("\\mathop {f}"));

    // The suffix forces limits in every style and is written back.
    let document = parse("\\mathop{f}\\limits_x");
    let latex = document.to_latex();
    assert!(latex.contains("\\limits"), "{latex}");
    assert_eq!(document.diagnostics.len(), 0);

    // Other commands select their own class.
    for (source, expected) in [
        ("\\mathbin{+}", AtomClass::Bin),
        ("\\mathrel{-}", AtomClass::Rel),
        ("\\mathord{x}", AtomClass::Ord),
        ("\\mathopen{(}", AtomClass::Open),
        ("\\mathclose{)}", AtomClass::Close),
        ("\\mathpunct{,}", AtomClass::Punct),
        ("\\mathinner{x}", AtomClass::Inner),
    ] {
        let document = parse(source);
        let NodeKind::Class { class, .. } = document
            .model
            .kind(row(&document.model, document.root())[0])
        else {
            panic!("expected a class wrapper for {source}");
        };
        assert_eq!(*class, expected, "{source}");
    }
}

#[test]
fn varlim_commands_are_display_limit_operators() {
    for (source, left_arrow) in [
        ("\\varlimsup_{x}", None),
        ("\\varliminf_{x}", None),
        ("\\varinjlim_{i}", Some('\u{2192}')),
        ("\\varprojlim_{i}", Some('\u{2190}')),
    ] {
        let document = parse(source);
        let NodeKind::Scripts { base, .. } = document
            .model
            .kind(row(&document.model, document.root())[0])
        else {
            panic!("expected scripts on {source}");
        };
        let NodeKind::Class {
            class,
            body,
            limits,
        } = document.model.kind(*base)
        else {
            panic!("expected the operator wrapper on {source}");
        };
        assert_eq!(*class, AtomClass::Op, "{source}");
        assert_eq!(
            *limits,
            Limits::Default {
                display_limits: true
            },
            "{source} takes limits in display style"
        );
        let inner = match (left_arrow, document.model.kind(*body)) {
            (None, NodeKind::Overline { body }) if source.starts_with("\\varlimsup") => *body,
            (None, NodeKind::Underline { body }) if source.starts_with("\\varliminf") => *body,
            (Some(expected), NodeKind::UnderArrow { symbol, body }) if *symbol == expected => *body,
            (_, other) => panic!("unexpected decoration for {source}: {other:?}"),
        };
        assert!(
            matches!(document.model.kind(inner), NodeKind::Function { name, .. } if name == "lim"),
            "{source} decorates `lim`"
        );
        let command = source.split('_').next().expect("a command");
        assert!(
            document.to_latex().starts_with(command),
            "the writer keeps the command name for {source}"
        );
    }
}

#[test]
fn displaylimits_selects_the_display_placement() {
    // `\operatorname{lim}` sets side scripts; the switch turns limits on.
    let document = parse("\\operatorname{lim}\\displaylimits_x");
    let NodeKind::Scripts { base, .. } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected scripts");
    };
    assert!(
        matches!(
            document.model.kind(*base),
            NodeKind::Function {
                limits: Limits::Default {
                    display_limits: true
                },
                ..
            }
        ),
        "`\\displaylimits` overrides `\\operatorname`'s side default"
    );

    // `\sum` and `\operatorname*` already take display limits: unchanged.
    for source in [
        "\\sum\\displaylimits_x",
        "\\operatorname*{lim}\\displaylimits_x",
    ] {
        let document = parse(source);
        let NodeKind::Scripts { base, .. } = document
            .model
            .kind(row(&document.model, document.root())[0])
        else {
            panic!("expected scripts for {source}");
        };
        let limits = match document.model.kind(*base) {
            NodeKind::LargeOp { limits, .. } | NodeKind::Function { limits, .. } => *limits,
            other => panic!("unexpected base for {source}: {other:?}"),
        };
        assert_eq!(
            limits,
            Limits::Default {
                display_limits: true
            },
            "{source}"
        );
    }

    // An integral's table default is side scripts; the switch overrides it,
    // and the canonical form has to keep the switch.
    let document = parse("\\int\\displaylimits_0^1");
    let NodeKind::Scripts { base, .. } = document
        .model
        .kind(row(&document.model, document.root())[0])
    else {
        panic!("expected scripts");
    };
    assert!(matches!(
        document.model.kind(*base),
        NodeKind::LargeOp {
            limits: Limits::Default {
                display_limits: true
            },
            ..
        }
    ));
    assert!(
        to_latex(&document.model, document.root()).contains("\\displaylimits"),
        "an override of the operator's own default must survive the writer"
    );

    // A switch that repeats the default is normalized away.
    let document = parse("\\sum\\displaylimits_x");
    assert!(!to_latex(&document.model, document.root()).contains("\\displaylimits"));
}
