//! Parses the M2 corpus from LaTeX, lays it out with this stack and with
//! formulary, then prints the extents side by side.
//!
//! cargo run -p math-diff -- <font.ttf> [--markdown] [--items]
//!
//! This is a report, not a gate: the two stacks implement different rule sets
//! (TeX Appendix G scripts vs MathML Core scripts), so a nonzero delta is only
//! a bug when it does not correspond to a documented rule choice.

use math_core::{Style, layout};
use math_font::Face;
use std::error::Error;

struct Case {
    name: &'static str,
    display: bool,
    latex: &'static str,
    mathml: &'static str,
}

fn corpus() -> Vec<Case> {
    vec![
        Case {
            name: "row spacing",
            display: false,
            latex: "a+b=c",
            mathml: "<math><mi>a</mi><mo>+</mo><mi>b</mi><mo>=</mo><mi>c</mi></math>",
        },
        Case {
            name: "superscript",
            display: false,
            latex: "x^2",
            mathml: "<math><msup><mi>x</mi><mn>2</mn></msup></math>",
        },
        Case {
            name: "subscript",
            display: false,
            latex: "a_i",
            mathml: "<math><msub><mi>a</mi><mi>i</mi></msub></math>",
        },
        Case {
            name: "both scripts",
            display: false,
            latex: "x_1^2",
            mathml: "<math><msubsup><mi>x</mi><mn>1</mn><mn>2</mn></msubsup></math>",
        },
        Case {
            name: "nested script",
            display: false,
            latex: "x^{2^3}",
            mathml: "<math><msup><mi>x</mi><msup><mn>2</mn><mn>3</mn></msup></msup></math>",
        },
        Case {
            name: "fraction",
            display: false,
            latex: "\\frac{a}{b}",
            mathml: "<math><mfrac><mi>a</mi><mi>b</mi></mfrac></math>",
        },
        Case {
            name: "fraction row",
            display: false,
            latex: "\\frac{1}{x+1}",
            mathml: "<math><mfrac><mn>1</mn><mrow><mi>x</mi><mo>+</mo><mn>1</mn></mrow></mfrac></math>",
        },
        Case {
            name: "fraction scripts",
            display: false,
            latex: "\\frac{x^2}{y_1}",
            mathml: "<math><mfrac><msup><mi>x</mi><mn>2</mn></msup><msub><mi>y</mi><mn>1</mn></msub></mfrac></math>",
        },
        Case {
            name: "radical",
            display: false,
            latex: "\\sqrt{x}",
            mathml: "<math><msqrt><mi>x</mi></msqrt></math>",
        },
        Case {
            name: "indexed radical",
            display: false,
            latex: "\\sqrt[3]{x}",
            mathml: "<math><mroot><mi>x</mi><mn>3</mn></mroot></math>",
        },
        Case {
            name: "radical of fraction",
            display: false,
            latex: "\\sqrt{\\frac{a}{b}}",
            mathml: "<math><msqrt><mfrac><mi>a</mi><mi>b</mi></mfrac></msqrt></math>",
        },
        Case {
            name: "parenthesized fraction",
            display: false,
            latex: "\\left(\\frac{a}{b}\\right)",
            mathml: "<math><mrow><mo>(</mo><mfrac><mi>a</mi><mi>b</mi></mfrac><mo>)</mo></mrow></math>",
        },
        Case {
            name: "bracketed square",
            display: false,
            latex: "\\left[x^2\\right]",
            mathml: "<math><mrow><mo>[</mo><msup><mi>x</mi><mn>2</mn></msup><mo>]</mo></mrow></math>",
        },
        Case {
            name: "sum with limits",
            display: true,
            latex: "\\sum_{i=1}^{n}",
            mathml: "<math display=\"block\"><munderover><mo>∑</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mi>n</mi></munderover></math>",
        },
        Case {
            name: "integral with side scripts",
            display: true,
            latex: "\\int_0^1",
            mathml: "<math display=\"block\"><msubsup><mo>∫</mo><mn>0</mn><mn>1</mn></msubsup></math>",
        },
        Case {
            name: "matrix 2x2",
            display: false,
            latex: "\\begin{matrix}a&b\\\\c&d\\end{matrix}",
            mathml: "<math><mtable><mtr><mtd><mi>a</mi></mtd><mtd><mi>b</mi></mtd></mtr><mtr><mtd><mi>c</mi></mtd><mtd><mi>d</mi></mtd></mtr></mtable></math>",
        },
        Case {
            name: "display style switch",
            display: false,
            latex: "a\\displaystyle b+c",
            mathml: "<math><mi>a</mi><mstyle displaystyle=\"true\"><mi>b</mi><mo>+</mo><mi>c</mi></mstyle></math>",
        },
        Case {
            name: "mathop under limits",
            display: true,
            latex: "\\mathop{f}_x",
            mathml: "<math display=\"block\"><munder><mo>f</mo><mi>x</mi></munder></math>",
        },
        Case {
            name: "combined",
            display: true,
            latex: "x^2+a_i+\\frac{a+b}{2}=\\sqrt{c+d}",
            mathml: "<math display=\"block\"><msup><mi>x</mi><mn>2</mn></msup><mo>+</mo><msub><mi>a</mi><mi>i</mi></msub><mo>+</mo><mfrac><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow><mn>2</mn></mfrac><mo>=</mo><msqrt><mrow><mi>c</mi><mo>+</mo><mi>d</mi></mrow></msqrt></math>",
        },
    ]
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let font_path = arguments
        .next()
        .ok_or("usage: math-diff <font.ttf> [--markdown] [--items]")?;
    let mut markdown = false;
    let mut print_items = false;
    let mut gate = false;
    for argument in arguments {
        match argument.as_str() {
            "--markdown" => markdown = true,
            "--items" => print_items = true,
            "--gate" => gate = true,
            other => return Err(format!("unknown option {other}").into()),
        }
    }
    let mut violations: Vec<String> = Vec::new();
    let bytes = std::fs::read(&font_path)?;
    let face = Face::parse(&bytes, 0)?;
    let reference = formulary::MathFont::new(&bytes, 0)?;

    let cases = corpus();

    if markdown {
        println!(
            "| Formula | Ours width/ascent/descent | formulary | Δ width | Δ ascent | Δ descent |"
        );
        println!("| --- | --- | --- | --- | --- | --- |");
    }
    for case in &cases {
        let style = if case.display {
            Style::DISPLAY
        } else {
            Style::TEXT
        };
        let document = math_syntax::parse(case.latex);
        for diagnostic in &document.diagnostics {
            eprintln!(
                "{}: {}..{}: {}",
                case.name, diagnostic.span.start, diagnostic.span.end, diagnostic.message
            );
            if gate {
                violations.push(format!("{}: parse diagnostic", case.name));
            }
        }
        let (ours, diagnostics) = layout(&face, &document.model, document.root(), 10.0, style);
        let tree = formulary::parse(case.mathml)?;
        let theirs = formulary::layout(
            &tree,
            &reference,
            &formulary::LayoutOptions { font_size: 10.0 },
        );
        let width = ours.width - theirs.width;
        let ascent = ours.ascent - theirs.ascent;
        let descent = ours.descent - theirs.descent;
        if gate {
            let (allowed_width, allowed_ascent, allowed_descent) =
                documented_delta(case.name).unwrap_or((0.0, 0.0, 0.0));
            for (axis, delta, allowed) in [
                ("width", width, allowed_width),
                ("ascent", ascent, allowed_ascent),
                ("descent", descent, allowed_descent),
            ] {
                if delta.abs() > allowed + 0.01 {
                    violations.push(format!(
                        "{}: {axis} delta {delta:+.3} exceeds the documented {allowed:.3}",
                        case.name
                    ));
                }
            }
        }
        if markdown {
            println!(
                "| {} | {:.3} / {:.3} / {:.3} | {:.3} / {:.3} / {:.3} | {:+.3} | {:+.3} | {:+.3} |",
                case.name,
                ours.width,
                ours.ascent,
                ours.descent,
                theirs.width,
                theirs.ascent,
                theirs.descent,
                width,
                ascent,
                descent
            );
        } else {
            println!(
                "{:<28} ours {:>7.3} {:>7.3} {:>7.3} | ref {:>7.3} {:>7.3} {:>7.3} | delta {:>+7.3} {:>+7.3} {:>+7.3} | items {} / {}",
                case.name,
                ours.width,
                ours.ascent,
                ours.descent,
                theirs.width,
                theirs.ascent,
                theirs.descent,
                width,
                ascent,
                descent,
                ours.items.len(),
                theirs.items.len()
            );
        }
        for diagnostic in diagnostics {
            eprintln!(
                "{}: layout {}..{}: {}",
                case.name, diagnostic.span.start, diagnostic.span.end, diagnostic.message
            );
            if gate {
                violations.push(format!("{}: layout diagnostic", case.name));
            }
        }
        if print_items {
            println!("  ours:");
            for item in &ours.items {
                println!("    {}", describe_ours(item));
            }
            println!("  ref:");
            for item in &theirs.items {
                println!("    {}", describe_theirs(item, theirs.ascent));
            }
        }
    }
    if gate {
        if violations.is_empty() {
            println!("gate: all deltas are within the documented allowances");
        } else {
            for violation in &violations {
                eprintln!("gate: {violation}");
            }
            return Err(format!("{} differential violation(s)", violations.len()).into());
        }
    }
    Ok(())
}

/// The deltas the differential note records as rule choices rather than bugs.
/// Values are absolute allowances; the gate adds a small float tolerance.
fn documented_delta(name: &str) -> Option<(f32, f32, f32)> {
    Some(match name {
        "subscript" => (0.0, 0.01, 0.0),
        "both scripts" => (0.0, 0.35, 1.02),
        "nested script" => (0.0, 0.68, 0.0),
        "fraction" => (0.0, 0.01, 0.0),
        "fraction row" => (3.56, 0.0, 0.0),
        "fraction scripts" => (0.0, 0.0, 0.67),
        "radical of fraction" => (0.0, 0.01, 0.01),
        "sum with limits" => (7.78, 0.0, 0.0),
        "integral with side scripts" => (1.81, 3.17, 1.50),
        // MathML's table pads every cell and adds a row strut; TeX's array
        // keeps its column and row gaps outside the cell content.
        "matrix 2x2" => (11.0, 8.08, 2.70),
        // MathML's munder reserves lspace/rspace around the under script and
        // places it deeper than TeX's operator-script clearance.
        "mathop under limits" => (4.542, 0.0, 1.58),
        _ => return None,
    })
}

fn describe_ours(item: &math_core::Item) -> String {
    match item {
        math_core::Item::Glyph(glyph) => format!(
            "glyph {:>5} x {:>7.3} baseline {:>7.3} size {:.3}",
            glyph.glyph.0, glyph.x, glyph.baseline, glyph.size
        ),
        math_core::Item::Rule(rule) => format!(
            "rule x {:>7.3} y {:>7.3} w {:>7.3} h {:.3}",
            rule.x, rule.y, rule.width, rule.height
        ),
        math_core::Item::Line(line) => format!(
            "line ({:>7.3}, {:>7.3}) to ({:>7.3}, {:>7.3}) width {:.3}",
            line.x1, line.y1, line.x2, line.y2, line.thickness
        ),
    }
}

fn describe_theirs(item: &formulary::Item, ascent: f32) -> String {
    match item {
        formulary::Item::Glyph { id, x, y, size, .. } => format!(
            "glyph {:>5} x {:>7.3} baseline {:>7.3} size {:.3}",
            id.0,
            x,
            y + ascent,
            size
        ),
        formulary::Item::Rule { x, y, w, h, .. } => {
            format!(
                "rule x {:>7.3} y {:>7.3} w {:>7.3} h {:.3}",
                x,
                y + ascent,
                w,
                h
            )
        }
        _ => "other".to_owned(),
    }
}
