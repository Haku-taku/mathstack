use math_core::{
    AtomClass, FractionRule, Limits, Model, NodeId, NodeKind, Origin, SpaceKind, Span, Variant,
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
        } => {
            out.push_str(&format!("Table({columns},{alignment:?},"));
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
        Some(NodeKind::Text(text)) if text == "a {b} c"
    ));
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
    } = model.kind(content[0])
    else {
        panic!("expected a table");
    };
    assert_eq!(*columns, 2);
    assert!(alignment.is_empty(), "matrix columns default to centred");
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
    assert_eq!(command_shape("begin"), CommandShape::Environment);
    assert_eq!(command_shape("hspace"), CommandShape::Length);
    assert_eq!(command_shape("not"), CommandShape::SymbolArgument);
    assert_eq!(command_shape("genfrac"), CommandShape::Special);
    assert_eq!(command_shape("nope"), CommandShape::Unknown);
}
