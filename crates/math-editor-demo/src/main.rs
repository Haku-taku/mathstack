//! A native host for `math-editor`.
//!
//! The display list is painted as SVG by `math-svg`, rasterised with `resvg`
//! and uploaded as an egui texture. Carets and selections are drawn from the
//! geometry index, so the window exercises the same contract a real host uses.

use eframe::egui;
use math_core::{Point, Rect as MathRect, Style};
use math_editor::{Editor, Key};
use math_font::Face;
use std::error::Error;

const PADDING: f32 = 8.0;
const DEFAULT_FONT: &str = "/usr/share/fonts/wps-office/DejaVuMathTeXGyre.ttf";

/// The formula canvas keeps one id so input can be handled before painting it.
fn canvas_id() -> egui::Id {
    egui::Id::new("mathstack-formula-canvas")
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let font_path = arguments.next().unwrap_or_else(|| DEFAULT_FONT.to_owned());
    let source = arguments
        .next()
        .unwrap_or_else(|| "\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}".to_owned());
    let bytes = std::fs::read(&font_path)
        .map_err(|error| format!("cannot read font {font_path}: {error}"))?;
    // The face borrows the bytes; a demo can keep them for its whole life.
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let face = Face::parse(bytes, 0)?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 640.0])
            .with_min_inner_size([560.0, 360.0])
            .with_title("mathstack editor"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "mathstack editor",
        options,
        Box::new(move |context| Ok(Box::new(Demo::new(context, face, &source)))),
    )?;
    Ok(())
}

struct Demo {
    editor: Editor,
    face: Face<'static>,
    source: String,
    texture: Option<egui::TextureHandle>,
    texture_size: egui::Vec2,
    texture_scale: f32,
    dirty: bool,
    focused_once: bool,
    canvas_focused: bool,
    source_focused: bool,
    status: Option<String>,
}

impl Demo {
    fn new(context: &eframe::CreationContext<'_>, face: Face<'static>, source: &str) -> Self {
        context.egui_ctx.set_visuals(egui::Visuals::light());
        let editor = Editor::new(source, &face, 40.0, Style::DISPLAY);
        Demo {
            editor,
            face,
            source: source.to_owned(),
            texture: None,
            texture_size: egui::Vec2::ZERO,
            texture_scale: 0.0,
            dirty: true,
            focused_once: false,
            canvas_focused: false,
            source_focused: false,
            status: None,
        }
    }

    fn ensure_texture(&mut self, context: &egui::Context) {
        let scale = context.pixels_per_point();
        if !self.dirty && self.texture.is_some() && (self.texture_scale - scale).abs() < 0.01 {
            return;
        }
        let svg = math_svg::to_svg(
            self.editor.layout(),
            &self.face,
            &math_svg::Options { padding: PADDING },
        );
        let tree = match resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()) {
            Ok(tree) => tree,
            Err(error) => {
                self.status = Some(format!("cannot paint: {error}"));
                return;
            }
        };
        let size = tree.size();
        let width = (size.width() * scale).ceil().max(1.0) as u32;
        let height = (size.height() * scale).ceil().max(1.0) as u32;
        let Some(mut pixmap) = tiny_skia::Pixmap::new(width, height) else {
            self.status = Some("formula is too large to paint".to_owned());
            return;
        };
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        let image = egui::ColorImage::from_rgba_premultiplied(
            [width as usize, height as usize],
            pixmap.data(),
        );
        self.texture = Some(context.load_texture("formula", image, egui::TextureOptions::LINEAR));
        self.texture_size = egui::vec2(size.width(), size.height());
        self.texture_scale = scale;
        self.dirty = false;
    }

    fn handle_input(&mut self, context: &egui::Context, focused: bool) -> bool {
        if !focused {
            return false;
        }
        let events = context.input(|input| input.events.clone());
        let mut changed = false;
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    for symbol in text.chars() {
                        changed |= self.editor.key(Key::Character(symbol), false, &self.face);
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if modifiers.ctrl || modifiers.command {
                        match key {
                            egui::Key::Z if modifiers.shift => {
                                changed |= self.editor.redo(&self.face);
                            }
                            egui::Key::Z => changed |= self.editor.undo(&self.face),
                            egui::Key::Y => changed |= self.editor.redo(&self.face),
                            egui::Key::A => changed |= self.editor.select_all(),
                            egui::Key::C => {
                                if let Some(text) = self.editor.selection_latex() {
                                    context.copy_text(text);
                                }
                            }
                            egui::Key::X => {
                                if let Some(text) = self.editor.cut_selection(&self.face) {
                                    context.copy_text(text);
                                    changed = true;
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }
                    let translated = match key {
                        egui::Key::ArrowLeft => Some(Key::Left),
                        egui::Key::ArrowRight => Some(Key::Right),
                        egui::Key::ArrowUp => Some(Key::Up),
                        egui::Key::ArrowDown => Some(Key::Down),
                        egui::Key::Backspace => Some(Key::Backspace),
                        egui::Key::Delete => Some(Key::Delete),
                        egui::Key::Home => Some(Key::Home),
                        egui::Key::End => Some(Key::End),
                        egui::Key::Escape => Some(Key::Escape),
                        _ => None,
                    };
                    if let Some(key) = translated {
                        changed |= self.editor.key(key, modifiers.shift, &self.face);
                    }
                }
                egui::Event::Paste(text) => {
                    changed |= self.editor.insert_latex(&text, &self.face);
                }
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    changed |= self.editor.set_preedit(&text, &self.face);
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                    changed |= self.editor.commit_preedit(&text, &self.face);
                }
                _ => {}
            }
        }
        self.dirty |= changed;
        changed
    }
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        // Input has to run before the texture is built, or an edit would only
        // appear on the next input event; the focus flag comes from last frame
        // because egui clears it on Escape before this point.
        let changed = self.handle_input(&context, self.canvas_focused);
        if !self.source_focused && self.editor.source() != self.source {
            self.source = self.editor.source();
        }
        if changed {
            self.dirty = true;
            context.request_repaint();
        }
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("mathstack");
                ui.separator();
                if ui.button("a/b").on_hover_text("fraction").clicked() {
                    self.dirty |= self.editor.insert_latex("\\frac{}{}", &self.face);
                }
                if ui.button("√").on_hover_text("radical").clicked() {
                    self.dirty |= self.editor.insert_latex("\\sqrt{}", &self.face);
                }
                if ui.button("x²").on_hover_text("superscript").clicked() {
                    self.dirty |= self.editor.insert_superscript(&self.face);
                }
                if ui.button("xₙ").on_hover_text("subscript").clicked() {
                    self.dirty |= self.editor.insert_subscript(&self.face);
                }
                if ui.button("sum").on_hover_text("summation").clicked() {
                    self.dirty |= self.editor.insert_latex("\\sum_{i=1}^{n}", &self.face);
                }
                if ui.button("int").on_hover_text("integral").clicked() {
                    self.dirty |= self.editor.insert_latex("\\int_{0}^{1}", &self.face);
                }
                if ui.button("↶").on_hover_text("undo").clicked() {
                    self.dirty |= self.editor.undo(&self.face);
                }
                if ui.button("↷").on_hover_text("redo").clicked() {
                    self.dirty |= self.editor.redo(&self.face);
                }
                ui.separator();
                let mut display = self.editor.style() == Style::DISPLAY;
                if ui.checkbox(&mut display, "display").changed() {
                    self.editor.set_style(
                        if display { Style::DISPLAY } else { Style::TEXT },
                        &self.face,
                    );
                    self.dirty = true;
                }
                let mut size = self.editor.size();
                if ui
                    .add(
                        egui::DragValue::new(&mut size)
                            .range(12.0..=72.0)
                            .speed(0.5),
                    )
                    .changed()
                {
                    self.editor.set_size(size, &self.face);
                    self.dirty = true;
                }
            });
        });

        egui::Panel::top("source").show(ui, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.source)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .hint_text("LaTeX source"),
                );
                self.source_focused = response.has_focus();
                let apply = ui.button("Apply").clicked()
                    || (response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                if apply {
                    self.editor.set_source(&self.source, &self.face);
                    self.source = self.editor.source();
                    self.dirty = true;
                }
                if ui.button("Copy").clicked() {
                    context.copy_text(self.editor.source());
                }
            });
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.monospace(self.editor.source());
                if let Some(message) = self
                    .editor
                    .message()
                    .map(str::to_owned)
                    .or_else(|| self.status.clone())
                {
                    ui.colored_label(egui::Color32::RED, message);
                }
                if let Some(command) = self.editor.command() {
                    ui.colored_label(egui::Color32::DARK_BLUE, format!("\\{command}"));
                }
                if let Some(preedit) = self.editor.preedit() {
                    ui.colored_label(egui::Color32::DARK_GREEN, format!("preedit: {preedit}"));
                }
                let errors = self.editor.diagnostics().len();
                if errors > 0 {
                    ui.colored_label(egui::Color32::DARK_RED, format!("{errors} diagnostic(s)"));
                }
                ui.label("type math, \\frac…, ^ and _; click to place the caret");
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            self.ensure_texture(ui.ctx());
            let available = ui.available_size();
            let scale = if self.texture_size.x > 0.0 && self.texture_size.y > 0.0 {
                ((available.x - 40.0) / self.texture_size.x)
                    .min((available.y - 40.0) / self.texture_size.y)
                    .clamp(0.25, 1.0)
            } else {
                1.0
            };
            let shown = self.texture_size * scale;
            let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
            let response = ui.interact(rect, canvas_id(), egui::Sense::click_and_drag());
            if !self.focused_once {
                response.request_focus();
                self.focused_once = true;
            }
            if response.clicked() || response.drag_started() {
                response.request_focus();
            }
            self.canvas_focused = response.has_focus();
            // Without this, egui treats arrows, Tab and Escape as focus
            // navigation and moves the keyboard focus to another widget while
            // the editor is also using those keys.
            if self.canvas_focused {
                context.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            tab: true,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: true,
                        },
                    );
                });
            }
            let origin = rect.center() - shown / 2.0;
            let image_rect = egui::Rect::from_min_size(origin, shown);
            let painter = ui.painter();
            painter.rect_filled(image_rect.expand(4.0), 6.0, egui::Color32::WHITE);
            if let Some(texture) = &self.texture {
                painter.image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            let paper = image_rect.min + egui::vec2(PADDING, PADDING) * scale;
            let to_screen =
                |point: Point| egui::pos2(paper.x + point.x * scale, paper.y + point.y * scale);
            let to_rect = |rect: MathRect| {
                egui::Rect::from_min_size(
                    to_screen(Point {
                        x: rect.x,
                        y: rect.y,
                    }),
                    egui::vec2(rect.width, rect.height) * scale,
                )
            };
            let placeholder_fill = egui::Color32::from_rgba_unmultiplied(70, 120, 220, 24);
            let placeholder_stroke = egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(70, 120, 220, 110),
            );
            for (_, placeholder) in self.editor.placeholders() {
                painter.rect(
                    to_rect(placeholder),
                    2.0,
                    placeholder_fill,
                    placeholder_stroke,
                    egui::StrokeKind::Inside,
                );
            }
            for selection in self.editor.selection() {
                painter.rect_filled(
                    to_rect(selection),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(70, 120, 220, 70),
                );
            }
            if response.has_focus()
                && let Some(caret) = self.editor.caret_rect()
            {
                let top = to_screen(Point {
                    x: caret.x,
                    y: caret.top,
                });
                let bottom = to_screen(Point {
                    x: caret.x,
                    y: caret.bottom,
                });
                painter.line_segment([top, bottom], egui::Stroke::new(1.5, egui::Color32::BLACK));
                context.output_mut(|output| {
                    output.ime = Some(egui::output::IMEOutput {
                        purpose: egui::IMEPurpose::Normal,
                        rect: image_rect,
                        cursor_rect: egui::Rect::from_min_max(top, bottom),
                        should_interrupt_composition: false,
                    });
                });
            }
            if let Some(pointer) = response.interact_pointer_pos()
                && image_rect.contains(pointer)
            {
                let point = Point {
                    x: (pointer.x - paper.x) / scale,
                    y: (pointer.y - paper.y) / scale,
                };
                if response.dragged() {
                    self.editor.click(point, true);
                } else if response.clicked() {
                    self.editor.click(point, false);
                }
            }
            if !response.has_focus() {
                ui.painter().text(
                    image_rect.center_bottom() + egui::vec2(0.0, 18.0),
                    egui::Align2::CENTER_TOP,
                    "click the formula to type",
                    egui::FontId::proportional(13.0),
                    egui::Color32::GRAY,
                );
            }
        });
    }
}
