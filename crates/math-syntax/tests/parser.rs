use math_core::{
    Alignment, AtomClass, ColumnSpacing, FractionRule, Limits, Model, NodeId, NodeKind, Origin,
    Size, SpaceKind, Span, Style, Variant,
};
use math_syntax::{
    CommandShape, command_shape, command_template, parse, to_latex, to_latex_with_spans,
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
        NodeKind::Class {
            class,
            body,
            limits,
        } => {
            out.push_str(&format!("Class({class:?},{limits:?},"));
            write_dump(model, *body, out);
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
        NodeKind::Atom(atom) if atom.symbol == 'R' && atom.variant == Variant::DoubleStruck
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
    assert_eq!(command_template("mathbb"), None);
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

    // A second switch closes the first one and takes over the rest.
    let document = parse("\\displaystyle a\\textstyle b");
    let children = row(&document.model, document.root());
    let sizes: Vec<Size> = children
        .iter()
        .map(|child| match document.model.kind(*child) {
            NodeKind::Styled { size, .. } => *size,
            other => panic!("expected a style wrapper, found {other:?}"),
        })
        .collect();
    assert_eq!(sizes, vec![Size::Display, Size::Text]);
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
