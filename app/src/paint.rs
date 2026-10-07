//! CPU raster of one dialogue into premultiplied linear BGRA. The window
//! target is sRGB, so these bytes are the linear form of the theme's sRGB
//! colors, stored B,G,R,A. Markers become chips only while editing; playback
//! and history skip them.

use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight,
};
use dialogue::{DialogueChrome, INPUT_TOOLBAR_PX, ImageAtom, Motion, Piece};

pub const TOOLBAR_PX: f32 = INPUT_TOOLBAR_PX;

#[derive(Clone, Copy)]
pub struct Slot {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub line: u32,
}

#[derive(Clone, Copy)]
pub struct HitRow {
    pub index: usize,
    pub top: f32,
    pub bottom: f32,
}

/// One submitted line, painted with the same dialogue card as the typewriter.
pub struct HistoryLine<'a> {
    pub index: usize,
    pub name: &'a str,
    pub pieces: &'a [Piece],
}

pub struct HistoryPaint {
    pub frame: Frame,
    pub scroll: f32,
    pub at_end: bool,
}

pub struct Frame {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub slots: Vec<Slot>,
    pub caret: (f32, f32),
    pub body: (f32, f32, f32, f32),
    pub rows: Vec<HitRow>,
}

pub struct Scene<'a> {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub chrome: &'a DialogueChrome,
    pub name: &'a str,
    pub pieces: &'a [Piece],
    pub show_markers: bool,
    pub caret: Option<usize>,
    pub selection: Option<(usize, usize)>,
    pub preedit: &'a str,
    pub opacity: f32,
    pub pop: f32,
    pub blink: bool,
    pub elapsed_ms: u64,
}

pub struct Painter {
    fonts: FontSystem,
    cache: SwashCache,
}

/// One dialogue walk shared by the input, the typewriter and history cards.
struct Lay<'a> {
    chrome: &'a DialogueChrome,
    box_w: f32,
    /// Clip when the pen passes this card-local y. History passes a huge value
    /// and uses `body_bottom` as the card height instead.
    box_h: f32,
    scale: f32,
    name: &'a str,
    pieces: &'a [Piece],
    show_markers: bool,
    caret: Option<usize>,
    selection: Option<(usize, usize)>,
    preedit: &'a str,
    elapsed_ms: u64,
}

struct NamePlate<'a> {
    text: &'a str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

enum Draw<'a> {
    Fill {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [u8; 4],
    },
    Preedit {
        x: f32,
        y: f32,
        text: &'a str,
        color: [u8; 4],
    },
    Glyph {
        x: f32,
        y: f32,
        text: &'a str,
        size: f32,
        weight: u16,
        italic: bool,
        underline: bool,
        color: [u8; 4],
        glow: f32,
    },
    Image {
        x: f32,
        y: f32,
        image: &'a ImageAtom,
        w: f32,
        h: f32,
    },
    Marker {
        x: f32,
        y: f32,
        label: &'static str,
    },
}

struct Placement<'a> {
    /// Card-local. The raster adds the card origin.
    slots: Vec<Slot>,
    caret: (f32, f32),
    /// Bottom of the last line, measured from the card top.
    body_bottom: f32,
    name: Option<NamePlate<'a>>,
    ops: Vec<Draw<'a>>,
}

impl Painter {
    pub fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            cache: SwashCache::new(),
        }
    }

    pub fn dialogue(&mut self, scene: Scene<'_>) -> Frame {
        let scale = scene.scale.max(1.0);
        let card = scene.chrome.card(scene.width, scene.height, scene.pop);
        let placed = self.place(&Lay {
            chrome: scene.chrome,
            box_w: card.2,
            box_h: card.3,
            scale,
            name: scene.name,
            pieces: scene.pieces,
            show_markers: scene.show_markers,
            caret: scene.caret,
            selection: scene.selection,
            preedit: scene.preedit,
            elapsed_ms: scene.elapsed_ms,
        });
        self.rasterize(
            scene.width,
            scene.height,
            scale,
            scene.chrome,
            scene.opacity,
            scene.blink,
            scene.elapsed_ms,
            card,
            placed,
        )
    }

    /// Stacked dialogue cards. Height comes from the same walk that paints the
    /// input and the typewriter, so a card hugs its line.
    pub fn history(
        &mut self,
        width_l: f32,
        height_l: f32,
        scale: f32,
        chrome: &DialogueChrome,
        lines: &[HistoryLine<'_>],
        scroll: f32,
        elapsed_ms: u64,
    ) -> HistoryPaint {
        let scale = scale.max(1.0);
        let width_l = width_l.max(1.0);
        let height_l = height_l.max(1.0);
        let width = (width_l * scale).round().max(1.0) as u32;
        let height = (height_l * scale).round().max(1.0) as u32;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let gap = chrome.name_gap;
        let box_w = chrome.card(width_l, 1.0, 1.0).2;
        let mut laid = Vec::with_capacity(lines.len());
        for line in lines {
            let placed = self.place(&Lay {
                chrome,
                box_w,
                box_h: f32::MAX,
                scale,
                name: line.name,
                pieces: line.pieces,
                show_markers: false,
                caret: None,
                selection: None,
                preedit: "",
                elapsed_ms,
            });
            let card_h =
                (placed.body_bottom + chrome.content_pad_y + chrome.frame_inset * 2.0).max(1.0);
            laid.push((card_h, placed));
        }
        let content = laid.iter().map(|(card_h, _)| *card_h).sum::<f32>()
            + gap * lines.len().saturating_sub(1) as f32;
        let max_scroll = (content - height_l).max(0.0);
        let scroll = scroll.clamp(0.0, max_scroll);
        let mut y = -scroll;
        let mut rows = Vec::with_capacity(lines.len());
        for (line, (card_h, placed)) in lines.iter().zip(laid) {
            let top = y;
            let bottom = y + card_h;
            if bottom > 0.0 && top < height_l {
                let card = self.rasterize(
                    width_l,
                    card_h,
                    scale,
                    chrome,
                    1.0,
                    false,
                    elapsed_ms,
                    chrome.card(width_l, card_h, 1.0),
                    placed,
                );
                blit_premul(
                    &mut pixels,
                    width,
                    height,
                    &card.pixels,
                    card.width,
                    card.height,
                    0,
                    (top * scale).round() as i32,
                );
            }
            rows.push(HitRow {
                index: line.index,
                top,
                bottom,
            });
            y = bottom + gap;
        }
        HistoryPaint {
            frame: Frame {
                pixels,
                width,
                height,
                slots: Vec::new(),
                caret: (0.0, 0.0),
                body: (0.0, 0.0, width_l, height_l),
                rows,
            },
            scroll,
            at_end: max_scroll - scroll < 1.0,
        }
    }

    fn place<'a>(&mut self, lay: &Lay<'a>) -> Placement<'a> {
        let chrome = lay.chrome;
        let scale = lay.scale;
        let pad_x = chrome.content_pad_x;
        let pad_y = chrome.content_pad_y;
        let mut x = pad_x;
        let mut y = pad_y;
        let right = lay.box_w - pad_x;
        let name = if lay.name.is_empty() {
            None
        } else {
            let name_h = chrome.name_px + chrome.name_pad_y * 2.0;
            let name_w = self
                .measure(lay.name, chrome, chrome.name_px, chrome.font_weight, false)
                .min(lay.box_w - pad_x * 2.0 - chrome.name_pad_x * 2.0)
                + chrome.name_pad_x * 2.0;
            let plate = NamePlate {
                text: lay.name,
                x,
                y,
                w: name_w,
                h: name_h,
            };
            y += name_h + chrome.name_gap;
            Some(plate)
        };
        let line_h = chrome.body_px * 1.35;
        let mut body_bottom = y + line_h;
        let mut slots = Vec::new();
        let mut caret = (x, y);
        let mut line = 0u32;
        let mut ops = Vec::new();
        let pieces = lay.pieces;
        let mut index = 0usize;
        while index <= pieces.len() {
            slots.push(Slot { index, x, y, line });
            if lay.caret == Some(index) {
                caret = (x, y);
                if !lay.preedit.is_empty() {
                    let color = chrome.body_fg;
                    let faded = [color[0], color[1], color[2], color[3] / 2];
                    let advance = self.text_advance(
                        lay.preedit,
                        chrome,
                        chrome.body_px,
                        chrome.font_weight,
                        false,
                        scale,
                    );
                    ops.push(Draw::Preedit {
                        x,
                        y,
                        text: lay.preedit,
                        color: faded,
                    });
                    x += advance;
                    if x > right {
                        x = pad_x;
                        y += line_h;
                        line += 1;
                        body_bottom = y + line_h;
                    }
                }
            }
            if index == pieces.len() {
                break;
            }
            let selected = lay
                .selection
                .is_some_and(|(start, end)| index >= start && index < end);
            if selected {
                ops.push(Draw::Fill {
                    x,
                    y,
                    w: 8.0,
                    h: line_h,
                    color: [186, 224, 255, 120],
                });
            }
            match &pieces[index] {
                Piece::Grapheme { cluster, .. } if cluster == "\n" => {
                    x = pad_x;
                    y += line_h;
                    line += 1;
                    body_bottom = y + line_h;
                }
                Piece::Grapheme { cluster, style } => {
                    let size = style.size_px.unwrap_or(chrome.body_px);
                    let weight = if style.bold { 700 } else { chrome.font_weight };
                    let color = motion_color(
                        style.color.unwrap_or(chrome.body_fg),
                        style.motion,
                        index,
                        lay.elapsed_ms,
                    );
                    let (dx, dy) = motion_offset(style.motion, index, lay.elapsed_ms);
                    let drawn = self
                        .text_advance(cluster, chrome, size, weight, style.italic, scale)
                        .max(size * 0.4);
                    ops.push(Draw::Glyph {
                        x: x + dx,
                        y: y + dy,
                        text: cluster.as_str(),
                        size,
                        weight,
                        italic: style.italic,
                        underline: style.underline,
                        color,
                        glow: if chrome.glow { 1.0 } else { 0.0 },
                    });
                    if selected {
                        ops.push(Draw::Fill {
                            x,
                            y,
                            w: drawn,
                            h: line_h,
                            color: [186, 224, 255, 90],
                        });
                    }
                    x += drawn;
                    body_bottom = y + line_h;
                    if x > right {
                        x = pad_x;
                        y += line_h;
                        line += 1;
                        body_bottom = y + line_h;
                    }
                }
                Piece::Image(image) => {
                    let image_h = chrome.body_px * image.height_em.max(0.4);
                    let image_w = image_width(image, image_h);
                    ops.push(Draw::Image {
                        x,
                        y,
                        image,
                        w: image_w,
                        h: image_h,
                    });
                    x += image_w + 4.0;
                    body_bottom = body_bottom.max(y + image_h.max(line_h));
                    if x > right {
                        x = pad_x;
                        y += line_h;
                        line += 1;
                        body_bottom = body_bottom.max(y + image_h.max(line_h));
                    }
                }
                marker if lay.show_markers && marker.is_marker() => {
                    ops.push(Draw::Marker {
                        x,
                        y,
                        label: marker_label(marker),
                    });
                    x += 42.0;
                }
                _ => {}
            }
            index += 1;
            if y > lay.box_h {
                break;
            }
        }
        Placement {
            slots,
            caret,
            body_bottom,
            name,
            ops,
        }
    }

    fn rasterize(
        &mut self,
        width_l: f32,
        height_l: f32,
        scale: f32,
        chrome: &DialogueChrome,
        opacity: f32,
        blink: bool,
        elapsed_ms: u64,
        card: (f32, f32, f32, f32),
        placed: Placement<'_>,
    ) -> Frame {
        let (box_x, box_y, box_w, box_h) = card;
        let width = (width_l * scale).round().max(1.0) as u32;
        let height = (height_l * scale).round().max(1.0) as u32;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        fill_round(
            &mut pixels,
            width,
            height,
            scale,
            box_x,
            box_y,
            box_w,
            box_h,
            chrome.radius,
            chrome.body_bg,
            opacity,
        );
        stroke_round(
            &mut pixels,
            width,
            height,
            scale,
            box_x,
            box_y,
            box_w,
            box_h,
            chrome.radius,
            chrome.body_border,
            opacity,
        );
        if let Some(name) = placed.name {
            fill_round(
                &mut pixels,
                width,
                height,
                scale,
                name.x + box_x,
                name.y + box_y,
                name.w,
                name.h,
                name.h * 0.5,
                chrome.name_bg,
                opacity,
            );
            self.draw_string(
                &mut pixels,
                width,
                height,
                scale,
                name.text,
                name.x + box_x + chrome.name_pad_x,
                name.y + box_y + chrome.name_pad_y,
                chrome,
                chrome.name_px,
                chrome.font_weight,
                false,
                false,
                chrome.name_fg,
                opacity,
                0.0,
            );
        }
        let line_h = chrome.body_px * 1.35;
        for op in &placed.ops {
            match op {
                Draw::Fill { x, y, w, h, color } => fill_rect(
                    &mut pixels,
                    width,
                    height,
                    scale,
                    x + box_x,
                    y + box_y,
                    *w,
                    *h,
                    *color,
                    opacity,
                ),
                Draw::Preedit { x, y, text, color } => {
                    self.draw_string(
                        &mut pixels,
                        width,
                        height,
                        scale,
                        text,
                        x + box_x,
                        y + box_y,
                        chrome,
                        chrome.body_px,
                        chrome.font_weight,
                        false,
                        false,
                        *color,
                        opacity,
                        0.0,
                    );
                }
                Draw::Glyph {
                    x,
                    y,
                    text,
                    size,
                    weight,
                    italic,
                    underline,
                    color,
                    glow,
                } => {
                    self.draw_string(
                        &mut pixels,
                        width,
                        height,
                        scale,
                        text,
                        x + box_x,
                        y + box_y,
                        chrome,
                        *size,
                        *weight,
                        *italic,
                        *underline,
                        *color,
                        opacity,
                        *glow,
                    );
                }
                Draw::Image { x, y, image, w, h } => blit_image(
                    &mut pixels,
                    width,
                    height,
                    scale,
                    image,
                    x + box_x,
                    y + box_y,
                    *w,
                    *h,
                    opacity,
                    elapsed_ms,
                ),
                Draw::Marker { x, y, label } => {
                    fill_round(
                        &mut pixels,
                        width,
                        height,
                        scale,
                        x + box_x,
                        y + box_y + 4.0,
                        36.0,
                        line_h - 8.0,
                        8.0,
                        chrome.name_bg,
                        opacity,
                    );
                    self.draw_string(
                        &mut pixels,
                        width,
                        height,
                        scale,
                        label,
                        x + box_x + 6.0,
                        y + box_y + 6.0,
                        chrome,
                        14.0,
                        600,
                        false,
                        false,
                        chrome.name_fg,
                        opacity,
                        0.0,
                    );
                }
            }
        }
        let caret = (placed.caret.0 + box_x, placed.caret.1 + box_y);
        if blink {
            fill_rect(
                &mut pixels,
                width,
                height,
                scale,
                caret.0,
                caret.1,
                2.0,
                line_h,
                chrome.body_fg,
                opacity,
            );
        }
        let slots = placed
            .slots
            .into_iter()
            .map(|slot| Slot {
                x: slot.x + box_x,
                y: slot.y + box_y,
                ..slot
            })
            .collect();
        Frame {
            pixels,
            width,
            height,
            slots,
            caret,
            body: card,
            rows: Vec::new(),
        }
    }

    fn text_advance(
        &mut self,
        text: &str,
        chrome: &DialogueChrome,
        size: f32,
        weight: u16,
        italic: bool,
        scale: f32,
    ) -> f32 {
        shape_line(&mut self.fonts, text, chrome, size, weight, italic, scale).1
    }

    fn measure(
        &mut self,
        text: &str,
        chrome: &DialogueChrome,
        size: f32,
        weight: u16,
        italic: bool,
    ) -> f32 {
        let metrics = Metrics::new(size, size * 1.2);
        let mut buffer = Buffer::new(&mut self.fonts, metrics);
        buffer.set_size(Some(8_000.0), Some(size * 3.0));
        let attrs = attrs(chrome, size, weight, italic);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);
        buffer
            .layout_runs()
            .map(|run| run.line_w)
            .next()
            .unwrap_or(size * 0.5)
    }

    fn draw_string(
        &mut self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        scale: f32,
        text: &str,
        x: f32,
        y: f32,
        chrome: &DialogueChrome,
        size: f32,
        weight: u16,
        italic: bool,
        underline: bool,
        color: [u8; 4],
        opacity: f32,
        glow: f32,
    ) -> f32 {
        let (mut buffer, advance) =
            shape_line(&mut self.fonts, text, chrome, size, weight, italic, scale);
        let origin_x = x * scale;
        let origin_y = y * scale;
        let mut draw = |pixels: &mut [u8], ox: f32, oy: f32, color: [u8; 4], opacity: f32| {
            buffer.draw(
                &mut self.fonts,
                &mut self.cache,
                Color::rgba(color[0], color[1], color[2], color[3]),
                |glyph_x, glyph_y, w, h, color| {
                    blit_glyph(
                        pixels,
                        width,
                        height,
                        ox + glyph_x as f32,
                        oy + glyph_y as f32,
                        w,
                        h,
                        color,
                        opacity,
                    );
                },
            );
        };
        if glow > 0.0 {
            let glow_color = [color[0], color[1], color[2], 90];
            draw(
                pixels,
                origin_x + scale,
                origin_y + scale,
                glow_color,
                opacity,
            );
        }
        draw(pixels, origin_x, origin_y, color, opacity);
        if underline {
            fill_rect(
                pixels,
                width,
                height,
                scale,
                x,
                y + size + 1.0,
                advance.max(4.0),
                1.5,
                color,
                opacity,
            );
        }
        advance
    }
}

fn shape_line(
    fonts: &mut FontSystem,
    text: &str,
    chrome: &DialogueChrome,
    size: f32,
    weight: u16,
    italic: bool,
    scale: f32,
) -> (Buffer, f32) {
    let metrics = Metrics::new(size * scale, size * scale * 1.25);
    let mut buffer = Buffer::new(fonts, metrics);
    buffer.set_size(Some(8_000.0), Some(size * scale * 3.0));
    let attrs = attrs(chrome, size, weight, italic);
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(fonts, false);
    let advance = buffer
        .layout_runs()
        .map(|run| run.line_w / scale)
        .next()
        .unwrap_or(0.0);
    (buffer, advance)
}

fn attrs<'a>(chrome: &'a DialogueChrome, _size: f32, weight: u16, italic: bool) -> Attrs<'a> {
    let mut attrs = Attrs::new()
        .family(Family::Name(chrome.font_family))
        .weight(Weight(weight));
    if italic {
        attrs = attrs.style(Style::Italic);
    }
    attrs
}

fn marker_label(piece: &Piece) -> &'static str {
    match piece {
        Piece::Wait { .. } => "等",
        Piece::Speed { .. } => "速",
        Piece::Instant => "瞬",
        Piece::Pause => "停",
        _ => "",
    }
}

fn motion_offset(motion: Motion, index: usize, elapsed: u64) -> (f32, f32) {
    let t = elapsed as f32 / 180.0 + index as f32 * 0.6;
    match motion {
        Motion::Shake => ((t * 3.0).sin() * 1.2, (t * 5.0).cos() * 1.0),
        Motion::Wave => (0.0, (t).sin() * 3.0),
        Motion::Jump => (0.0, -((t).sin().abs()) * 4.0),
        _ => (0.0, 0.0),
    }
}

fn motion_color(color: [u8; 4], motion: Motion, index: usize, elapsed: u64) -> [u8; 4] {
    if motion != Motion::Rainbow {
        return color;
    }
    let hue = (elapsed as f32 / 12.0 + index as f32 * 24.0) % 360.0;
    let (r, g, b) = hsv(hue, 0.75, 1.0);
    [r, g, b, color[3]]
}

fn hsv(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn image_width(image: &ImageAtom, height: f32) -> f32 {
    let data = &image.data;
    if data.height == 0 {
        return height;
    }
    height * (data.width as f32 / data.height as f32)
}

fn blit_image(
    pixels: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    scale: f32,
    image: &ImageAtom,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    opacity: f32,
    elapsed: u64,
) {
    let Some(frame) = image.data.frame_at(elapsed) else {
        return;
    };
    let src_w = image.data.width;
    let src_h = image.data.height;
    if src_w == 0 || src_h == 0 {
        return;
    }
    let x0 = (x * scale).round() as i32;
    let y0 = (y * scale).round() as i32;
    let pw = (w * scale).round().max(1.0) as i32;
    let ph = (h * scale).round().max(1.0) as i32;
    for row in 0..ph {
        for col in 0..pw {
            let dx = x0 + col;
            let dy = y0 + row;
            if dx < 0 || dy < 0 || dx >= dest_w as i32 || dy >= dest_h as i32 {
                continue;
            }
            let sx = (col as u32 * src_w) / pw as u32;
            let sy = (row as u32 * src_h) / ph as u32;
            let src = ((sy * src_w + sx) * 4) as usize;
            if src + 3 >= frame.rgba.len() {
                continue;
            }
            blend(
                pixels,
                dest_w,
                dx as u32,
                dy as u32,
                [
                    frame.rgba[src],
                    frame.rgba[src + 1],
                    frame.rgba[src + 2],
                    frame.rgba[src + 3],
                ],
                opacity,
                false,
            );
        }
    }
}

fn blit_glyph(
    pixels: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    x: f32,
    y: f32,
    w: u32,
    h: u32,
    color: Color,
    opacity: f32,
) {
    let x0 = x.round() as i32;
    let y0 = y.round() as i32;
    for row in 0..h {
        for col in 0..w {
            let dx = x0 + col as i32;
            let dy = y0 + row as i32;
            if dx < 0 || dy < 0 || dx >= dest_w as i32 || dy >= dest_h as i32 {
                continue;
            }
            let coverage = ((color.a() as f32) / 255.0 * opacity).clamp(0.0, 1.0);
            blend(
                pixels,
                dest_w,
                dx as u32,
                dy as u32,
                [color.r(), color.g(), color.b(), (coverage * 255.0) as u8],
                1.0,
                true,
            );
        }
    }
}

fn fill_rect(
    pixels: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    scale: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let x0 = (x * scale).floor() as i32;
    let y0 = (y * scale).floor() as i32;
    let x1 = ((x + w) * scale).ceil() as i32;
    let y1 = ((y + h) * scale).ceil() as i32;
    for dy in y0..y1 {
        for dx in x0..x1 {
            if dx < 0 || dy < 0 || dx >= dest_w as i32 || dy >= dest_h as i32 {
                continue;
            }
            blend(pixels, dest_w, dx as u32, dy as u32, color, opacity, false);
        }
    }
}

fn fill_round(
    pixels: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    scale: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let x0 = (x * scale).floor() as i32;
    let y0 = (y * scale).floor() as i32;
    let x1 = ((x + w) * scale).ceil() as i32;
    let y1 = ((y + h) * scale).ceil() as i32;
    let radius = radius * scale;
    for dy in y0..y1 {
        for dx in x0..x1 {
            if dx < 0 || dy < 0 || dx >= dest_w as i32 || dy >= dest_h as i32 {
                continue;
            }
            if !inside_round(
                dx as f32, dy as f32, x0 as f32, y0 as f32, x1 as f32, y1 as f32, radius,
            ) {
                continue;
            }
            blend(pixels, dest_w, dx as u32, dy as u32, color, opacity, false);
        }
    }
}

fn stroke_round(
    pixels: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    scale: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: [u8; 4],
    opacity: f32,
) {
    let x0 = (x * scale).floor() as i32;
    let y0 = (y * scale).floor() as i32;
    let x1 = ((x + w) * scale).ceil() as i32;
    let y1 = ((y + h) * scale).ceil() as i32;
    let radius = radius * scale;
    let inset = 1.5 * scale;
    for dy in y0..y1 {
        for dx in x0..x1 {
            if dx < 0 || dy < 0 || dx >= dest_w as i32 || dy >= dest_h as i32 {
                continue;
            }
            let px = dx as f32;
            let py = dy as f32;
            let outer = inside_round(px, py, x0 as f32, y0 as f32, x1 as f32, y1 as f32, radius);
            let inner = inside_round(
                px,
                py,
                x0 as f32 + inset,
                y0 as f32 + inset,
                x1 as f32 - inset,
                y1 as f32 - inset,
                (radius - inset).max(0.0),
            );
            if outer && !inner {
                blend(pixels, dest_w, dx as u32, dy as u32, color, opacity, false);
            }
        }
    }
}

fn inside_round(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32) -> bool {
    if px < x0 || py < y0 || px >= x1 || py >= y1 {
        return false;
    }
    let radius = radius.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    let cx = if px < x0 + radius {
        x0 + radius
    } else if px > x1 - radius {
        x1 - radius
    } else {
        px
    };
    let cy = if py < y0 + radius {
        y0 + radius
    } else if py > y1 - radius {
        y1 - radius
    } else {
        py
    };
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= radius * radius + 0.5
}

fn blit_premul(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dx: i32,
    dy: i32,
) {
    for row in 0..src_h as i32 {
        let y = dy + row;
        if y < 0 || y >= dst_h as i32 {
            continue;
        }
        for col in 0..src_w as i32 {
            let x = dx + col;
            if x < 0 || x >= dst_w as i32 {
                continue;
            }
            let si = ((row as u32 * src_w + col as u32) * 4) as usize;
            if si + 3 >= src.len() {
                continue;
            }
            let sa = src[si + 3];
            if sa == 0 {
                continue;
            }
            let di = ((y as u32 * dst_w + x as u32) * 4) as usize;
            if di + 3 >= dst.len() {
                continue;
            }
            if sa == 255 {
                dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
                continue;
            }
            composite_premul(
                dst,
                di,
                [
                    src[si] as f32 / 255.0,
                    src[si + 1] as f32 / 255.0,
                    src[si + 2] as f32 / 255.0,
                ],
                sa as f32 / 255.0,
            );
        }
    }
}

fn blend(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    color: [u8; 4],
    opacity: f32,
    coverage_is_alpha: bool,
) {
    let alpha = if coverage_is_alpha {
        color[3] as f32 / 255.0
    } else {
        (color[3] as f32 / 255.0) * opacity.clamp(0.0, 1.0)
    };
    if alpha <= 0.0 {
        return;
    }
    let i = ((y * width + x) * 4) as usize;
    if i + 3 >= pixels.len() {
        return;
    }
    // Theme colors are sRGB. The composition target encodes linear fragments,
    // and its texel order is BGRA. Straight sRGB here would show Halo blue
    // as gold.
    composite_premul(
        pixels,
        i,
        [
            srgb_to_linear(color[2]) * alpha,
            srgb_to_linear(color[1]) * alpha,
            srgb_to_linear(color[0]) * alpha,
        ],
        alpha,
    );
}

fn composite_premul(pixels: &mut [u8], index: usize, src: [f32; 3], src_a: f32) {
    let dst_a = pixels[index + 3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return;
    }
    for channel in 0..3 {
        let dst = pixels[index + channel] as f32 / 255.0;
        let out = src[channel] + dst * (1.0 - src_a);
        pixels[index + channel] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    pixels[index + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn srgb_to_linear(channel: u8) -> f32 {
    let s = channel as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

impl Default for Painter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use dialogue::{DialogueDocument, ThemePack};

    use super::{HistoryLine, Painter, Scene};

    fn pieces(text: &str) -> DialogueDocument {
        let mut document = DialogueDocument::new();
        document.insert_text(text);
        document
    }

    fn has_halo_blue(pixels: &[u8]) -> bool {
        pixels.chunks(4).any(|pixel| {
            pixel.len() == 4
                && pixel[3] > 200
                && pixel[0] > pixel[2].saturating_add(40)
                && pixel[0] > 80
        })
    }

    #[test]
    fn history_and_dialogue_share_the_halo_card() {
        let mut painter = Painter::new();
        let chrome = ThemePack::halo().chrome;
        let document = pieces("你好");
        let name = "初音";
        let lines = [HistoryLine {
            index: 0,
            name,
            pieces: document.pieces(),
        }];
        let painted = painter.history(420.0, 800.0, 1.0, &chrome, &lines, 0.0, 0);
        let frame = &painted.frame;
        assert!(frame.height > 400, "history viewport stays tall");
        assert!(
            has_halo_blue(&frame.pixels),
            "name capsule stays blue in BGRA"
        );
        assert!(
            frame.pixels.chunks(4).any(|pixel| {
                pixel[3] > 200 && pixel[0] > 230 && pixel[1] > 230 && pixel[2] > 230
            }),
            "dialogue card stays white"
        );
        let row = &frame.rows[0];
        assert!(row.bottom - row.top < 220.0, "one line hugs its text");
        let clear_from = frame.height as usize * 3 / 4;
        assert!(
            frame.pixels[(clear_from * frame.width as usize)..]
                .chunks(4)
                .all(|pixel| pixel[3] == 0),
            "the card does not fill the history window"
        );

        let dialogue = painter.dialogue(Scene {
            width: 420.0,
            height: 200.0,
            scale: 1.0,
            chrome: &chrome,
            name,
            pieces: document.pieces(),
            show_markers: false,
            caret: None,
            selection: None,
            preedit: "",
            opacity: 1.0,
            pop: 1.0,
            blink: false,
            elapsed_ms: 0,
        });
        assert!(has_halo_blue(&dialogue.pixels));
    }
}
