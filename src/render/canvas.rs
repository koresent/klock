use std::collections::HashMap;

use crate::config::{Config, parse_color};
use crate::render::animations::{CursorGlideState, ShakeAnimation};
use crate::render::text::FontRenderer;
use tiny_skia::{
    Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, PixmapMut, PixmapPaint, Rect, Stroke,
    Transform,
};

pub struct CardUiState<'a> {
    pub display_name: &'a str,
    pub password_len: usize,
    pub password_text: &'a str,
    pub reveal_password: bool,
    pub hover_peek: bool,
    pub hover_submit: bool,
    pub capslock: bool,
    pub error: bool,
    pub glide: &'a CursorGlideState,
    pub shake: &'a ShakeAnimation,
}

pub struct UiRenderer {
    font: FontRenderer,
    source_wallpaper: Option<image::DynamicImage>,
    cached_wallpapers: HashMap<(u32, u32), Pixmap>,
    cached_avatar: Option<Pixmap>,
}

impl UiRenderer {
    pub fn new() -> anyhow::Result<Self> {
        let font = FontRenderer::load_default()?;
        Ok(Self {
            font,
            source_wallpaper: None,
            cached_wallpapers: HashMap::new(),
            cached_avatar: None,
        })
    }

    pub fn set_source_wallpaper(&mut self, img: image::DynamicImage) {
        self.source_wallpaper = Some(img);
        self.cached_wallpapers.clear();
    }

    pub fn set_avatar(&mut self, pixmap: Pixmap) {
        self.cached_avatar = Some(pixmap);
    }

    pub fn render_frame(
        &mut self,
        target: &mut PixmapMut,
        cfg: &Config,
        state: &CardUiState,
        is_primary: bool,
    ) {
        let screen_w = target.width() as f32;
        let screen_h = target.height() as f32;
        let target_w = target.width();
        let target_h = target.height();

        // 1. Fast wallpaper blit via SIMD memcpy (zero software alpha-blending overhead)
        if let Some(ref wall_img) = self.source_wallpaper {
            let key = (target_w, target_h);
            if !self.cached_wallpapers.contains_key(&key)
                && let Ok(pix) = crate::render::image::render_wallpaper(
                    wall_img,
                    target_w,
                    target_h,
                    cfg.wallpaper.blur_sigma,
                    cfg.appearance.backdrop_dim,
                )
            {
                self.cached_wallpapers.insert(key, pix);
            }
            if let Some(wall) = self.cached_wallpapers.get(&key) {
                if wall.width() == target_w && wall.height() == target_h {
                    target.data_mut().copy_from_slice(wall.data());
                } else {
                    let solid = parse_color(&cfg.wallpaper.solid_color, 1.0);
                    target.fill(solid);
                }
            } else {
                let solid = parse_color(&cfg.wallpaper.solid_color, 1.0);
                target.fill(solid);
            }
        } else {
            let solid = parse_color(&cfg.wallpaper.solid_color, 1.0);
            target.fill(solid);
        }

        if !is_primary {
            return;
        }

        if cfg.clock.enabled {
            self.draw_clock(target, cfg, screen_w, screen_h);
        }

        let shake_x = state.shake.offset_x();
        let card_w = cfg.card.width;
        let card_h = compute_card_height(cfg, state.capslock);
        let card_x = ((screen_w - card_w) / 2.0 + shake_x).round();
        let card_y = ((screen_h - card_h) / 2.0).round();

        if let Some(card_rect) = Rect::from_xywh(card_x, card_y, card_w, card_h) {
            self.draw_card(target, cfg, state, card_rect);
        }
    }

    fn draw_clock(&self, target: &mut PixmapMut, cfg: &Config, screen_w: f32, screen_h: f32) {
        let time_str = format_local_time(&cfg.clock.time_format);
        let date_str = format_local_time(&cfg.clock.date_format);
        let subtitle_str = cfg.clock.resolve_subtitle(&cfg.user.default);

        let time_font_size = cfg.clock.time_font_size.max(12.0);
        let date_font_size = cfg.clock.date_font_size.max(10.0);
        let sub_font_size = cfg.clock.subtitle_font_size.max(9.0);

        let time_color = if let Some(ref c) = cfg.clock.time_color {
            parse_color(c, 1.0)
        } else {
            parse_color(&cfg.appearance.text_color, 0.92)
        };

        let date_color = if let Some(ref c) = cfg.clock.date_color {
            parse_color(c, 1.0)
        } else {
            parse_color(&cfg.appearance.muted_color, 0.85)
        };

        let sub_color = if let Some(ref c) = cfg.clock.subtitle_color {
            parse_color(c, 1.0)
        } else {
            parse_color(&cfg.appearance.muted_color, 0.65)
        };

        let (tw, th) = if cfg.clock.time_bold {
            self.font.measure_text_bold(&time_str, time_font_size)
        } else {
            self.font.measure_text(&time_str, time_font_size)
        };
        let (dw, dh) = self.font.measure_text(&date_str, date_font_size);
        let (sw, _) = subtitle_str
            .as_deref()
            .map(|s| self.font.measure_text(s, sub_font_size))
            .unwrap_or((0.0, 0.0));

        let (tx, ty, dx, dy, sx, sy) = match cfg.clock.position.as_str() {
            "top-left" => {
                let x = 48.0;
                let y = 48.0;
                let dy = y + th + 14.0;
                let sy = dy + dh + 10.0;
                (x, y, x, dy, x, sy)
            }
            "top-right" => {
                let tx = screen_w - tw - 48.0;
                let dx = screen_w - dw - 48.0;
                let sx = screen_w - sw - 48.0;
                let y = 48.0;
                let dy = y + th + 14.0;
                let sy = dy + dh + 10.0;
                (tx, y, dx, dy, sx, sy)
            }
            "center" => {
                let card_h = compute_card_height(cfg, false);
                let center_y = (screen_h - card_h) / 2.0;
                let extra_sub = if subtitle_str.is_some() {
                    sub_font_size + 10.0
                } else {
                    0.0
                };
                let total_clock_h = th + 16.0 + date_font_size + extra_sub;
                let ty = (center_y - total_clock_h - 32.0).max(32.0);
                let tx = (screen_w - tw) / 2.0;
                let dx = (screen_w - dw) / 2.0;
                let dy = ty + th + 14.0;
                let sx = (screen_w - sw) / 2.0;
                let sy = dy + dh + 10.0;
                (tx, ty, dx, dy, sx, sy)
            }
            _ => {
                let tx = (screen_w - tw) / 2.0;
                let ty = (screen_h * 0.18).round();
                let dx = (screen_w - dw) / 2.0;
                let dy = ty + th + 18.0;
                let sx = (screen_w - sw) / 2.0;
                let sy = dy + dh + 10.0;
                (tx, ty, dx, dy, sx, sy)
            }
        };

        if cfg.clock.time_bold {
            self.font
                .draw_text_bold(target, &time_str, tx, ty, time_font_size, time_color);
        } else {
            self.font
                .draw_text(target, &time_str, tx, ty, time_font_size, time_color);
        }
        self.font
            .draw_text(target, &date_str, dx, dy, date_font_size, date_color);

        if let Some(ref sub) = subtitle_str {
            self.font
                .draw_text(target, sub, sx, sy, sub_font_size, sub_color);
        }
    }

    fn draw_card(
        &self,
        target: &mut PixmapMut,
        cfg: &Config,
        state: &CardUiState,
        card_rect: Rect,
    ) {
        let cx = card_rect.x();
        let cy = card_rect.y();
        let cw = card_rect.width();

        let card_bg = parse_color(&cfg.card.background_color, cfg.card.opacity);
        let mut bg_paint = Paint::default();
        bg_paint.set_color(card_bg);
        bg_paint.anti_alias = true;

        let card_path = rounded_rect_path(card_rect, cfg.card.border_radius);
        target.fill_path(
            &card_path,
            &bg_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let border_color = parse_color(&cfg.card.border_color, cfg.card.border_opacity);
        let mut border_paint = Paint::default();
        border_paint.set_color(border_color);
        border_paint.anti_alias = true;
        let stroke = Stroke {
            width: 1.0,
            ..Default::default()
        };
        target.stroke_path(
            &card_path,
            &border_paint,
            &stroke,
            Transform::identity(),
            None,
        );

        let mut content_y = cy + cfg.card.padding_y;

        if cfg.card.show_avatar
            && let Some(ref avatar) = self.cached_avatar
        {
            let av_size = cfg.card.avatar_size;
            let av_x = (cx + (cw - av_size) / 2.0).round();
            let av_y = content_y;
            target.draw_pixmap(
                av_x as i32,
                av_y as i32,
                avatar.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );

            let border_width = cfg.card.avatar_border_width.max(1.0);
            let av_border_col = if let Some(ref c) = cfg.card.avatar_border_color {
                parse_color(c, 1.0)
            } else {
                parse_color(&cfg.appearance.accent_color, 1.0)
            };
            let mut pb = PathBuilder::new();
            pb.push_circle(
                av_x + av_size / 2.0,
                av_y + av_size / 2.0,
                (av_size - border_width) / 2.0,
            );
            if let Some(circle_path) = pb.finish() {
                let mut circle_paint = Paint::default();
                circle_paint.set_color(av_border_col);
                circle_paint.anti_alias = true;
                let stroke = Stroke {
                    width: border_width,
                    ..Default::default()
                };
                target.stroke_path(
                    &circle_path,
                    &circle_paint,
                    &stroke,
                    Transform::identity(),
                    None,
                );
            }

            content_y += av_size + 14.0;
        }

        let name = state.display_name;
        let (nw, _) = self.font.measure_text_bold(name, 19.0);
        let nx = (cx + (cw - nw) / 2.0).round();
        let text_color = parse_color(&cfg.appearance.text_color, 1.0);
        self.font
            .draw_text_bold(target, name, nx, content_y, 19.0, text_color);
        content_y += 34.0;

        let (input_rect, peek_rect, submit_rect) =
            compute_input_row_geometry(card_rect, cfg, content_y);

        self.draw_input_box(target, cfg, state, input_rect, peek_rect);
        self.draw_submit_button(target, cfg, state, submit_rect);

        content_y += cfg.input.height + 14.0;

        if state.capslock && cfg.keyboard.show_capslock_warning {
            let caps_text = "CAPS LOCK";
            let (cw_len, _) = self.font.measure_text(caps_text, 11.0);
            let pill_w = cw_len + 16.0;
            let pill_h = 22.0;
            let pill_x = cx + (cw - pill_w) / 2.0;
            let pill_y = content_y;

            if let Some(pill_rect) = Rect::from_xywh(pill_x, pill_y, pill_w, pill_h) {
                let pill_bg = parse_color(&cfg.appearance.error_color, 0.22);
                let mut pill_paint = Paint::default();
                pill_paint.set_color(pill_bg);
                let path = rounded_rect_path(pill_rect, 11.0);
                target.fill_path(
                    &path,
                    &pill_paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );

                let caps_color = parse_color(&cfg.appearance.error_color, 0.95);
                self.font.draw_text(
                    target,
                    caps_text,
                    pill_x + 8.0,
                    pill_y + 4.0,
                    11.0,
                    caps_color,
                );
            }
        }
    }

    fn draw_submit_button(
        &self,
        target: &mut PixmapMut,
        cfg: &Config,
        state: &CardUiState,
        submit_rect: Rect,
    ) {
        let cx = submit_rect.x() + submit_rect.width() / 2.0;
        let cy = submit_rect.y() + submit_rect.height() / 2.0;
        let radius = submit_rect.width() / 2.0;

        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, radius);
        if let Some(circle_path) = pb.finish() {
            let mut bg_color = parse_color(&cfg.appearance.accent_color, 1.0);
            if state.hover_submit {
                bg_color = Color::from_rgba(
                    (bg_color.red() * 1.12).min(1.0),
                    (bg_color.green() * 1.12).min(1.0),
                    (bg_color.blue() * 1.12).min(1.0),
                    1.0,
                )
                .unwrap();
            }
            let mut bg_paint = Paint::default();
            bg_paint.set_color(bg_color);
            bg_paint.anti_alias = true;
            target.fill_path(
                &circle_path,
                &bg_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );

            if state.hover_submit {
                let mut ring_paint = Paint::default();
                ring_paint.set_color(parse_color(&cfg.appearance.text_color, 0.35));
                ring_paint.anti_alias = true;
                let ring_stroke = Stroke {
                    width: 1.5,
                    ..Default::default()
                };
                target.stroke_path(
                    &circle_path,
                    &ring_paint,
                    &ring_stroke,
                    Transform::identity(),
                    None,
                );
            }
        }

        let icon_color = parse_color(&cfg.appearance.accent_text_color, 1.0);
        let mut icon_pb = PathBuilder::new();
        icon_pb.move_to(cx - 3.0, cy - 6.0);
        icon_pb.line_to(cx + 3.5, cy);
        icon_pb.line_to(cx - 3.0, cy + 6.0);

        if let Some(icon_path) = icon_pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(icon_color);
            paint.anti_alias = true;
            let stroke = Stroke {
                width: 2.2,
                line_cap: LineCap::Round,
                ..Default::default()
            };
            target.stroke_path(&icon_path, &paint, &stroke, Transform::identity(), None);
        }
    }

    fn draw_input_box(
        &self,
        target: &mut PixmapMut,
        cfg: &Config,
        state: &CardUiState,
        input_rect: Rect,
        peek_rect: Rect,
    ) {
        let input_x = input_rect.x();
        let input_y = input_rect.y();
        let input_h = input_rect.height();

        let bg_color = parse_color(&cfg.input.background_color, cfg.input.opacity);
        let mut bg_paint = Paint::default();
        bg_paint.set_color(bg_color);
        bg_paint.anti_alias = true;

        let path = rounded_rect_path(input_rect, cfg.input.border_radius);
        target.fill_path(
            &path,
            &bg_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        // Matching kgreet: input has accent_color (#b8bb26) border when focused
        let border_color = if state.error {
            parse_color(&cfg.appearance.error_color, 0.95)
        } else if let Some(ref c) = cfg.input.focus_border_color {
            parse_color(c, 1.0)
        } else {
            parse_color(&cfg.appearance.accent_color, 1.0)
        };
        let mut border_paint = Paint::default();
        border_paint.set_color(border_color);
        border_paint.anti_alias = true;
        let stroke = Stroke {
            width: if state.error { 2.0 } else { 1.5 },
            ..Default::default()
        };
        target.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);

        let text_pad_x = 16.0;
        let text_start_x = input_x + text_pad_x;
        let text_end_x = if cfg.input.show_peek_icon {
            peek_rect.x() - 8.0
        } else {
            input_x + input_rect.width() - text_pad_x
        };
        let max_text_w = (text_end_x - text_start_x).max(40.0);
        let baseline_y = input_y + (input_h - 16.0) / 2.0 - 2.0;

        let content_w = self.compute_dot_x_offset(
            cfg,
            state.password_len,
            state.password_text,
            state.reveal_password,
        );
        let scroll_x = (content_w - max_text_w).max(0.0);

        if state.password_len == 0 {
            let placeholder_color = parse_color(&cfg.appearance.muted_color, 0.88);
            self.font.draw_text_clipped(
                target,
                &cfg.input.placeholder,
                text_start_x + 8.0,
                baseline_y,
                14.0,
                placeholder_color,
                (text_start_x, text_end_x),
            );
        } else if state.reveal_password {
            let text_color = parse_color(&cfg.appearance.text_color, 0.95);
            self.font.draw_text_clipped(
                target,
                state.password_text,
                text_start_x - scroll_x,
                baseline_y,
                14.0,
                text_color,
                (text_start_x, text_end_x),
            );
        } else {
            let dot_color = parse_color(&cfg.appearance.text_color, 0.95);
            let (dw, _) = self.font.measure_text(&cfg.input.dot_char, 16.0);
            let step = dw + cfg.input.dot_spacing;
            let mut dot_x = text_start_x - scroll_x;
            for _ in 0..state.password_len {
                if dot_x + dw >= text_start_x && dot_x <= text_end_x {
                    self.font.draw_text_clipped(
                        target,
                        &cfg.input.dot_char,
                        dot_x,
                        baseline_y + 1.0,
                        16.0,
                        dot_color,
                        (text_start_x, text_end_x),
                    );
                }
                dot_x += step;
            }
        }

        if cfg.input.cursor_glide.enabled {
            self.draw_cursor_glide(
                target,
                cfg,
                state,
                text_start_x,
                scroll_x,
                max_text_w,
                input_y,
                input_h,
            );
        }

        if cfg.input.show_peek_icon {
            if state.hover_peek {
                let pcx = peek_rect.x() + peek_rect.width() / 2.0;
                let pcy = peek_rect.y() + peek_rect.height() / 2.0;
                let mut pb = PathBuilder::new();
                pb.push_circle(pcx, pcy, peek_rect.width() * 0.72);
                if let Some(bg_path) = pb.finish() {
                    let mut bg_paint = Paint::default();
                    bg_paint.set_color(parse_color(&cfg.appearance.text_color, 0.14));
                    bg_paint.anti_alias = true;
                    target.fill_path(
                        &bg_path,
                        &bg_paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }

            let peek_color = if state.reveal_password {
                parse_color(&cfg.appearance.accent_color, 0.98)
            } else if state.hover_peek {
                parse_color(&cfg.appearance.text_color, 0.95)
            } else if state.password_len > 0 {
                parse_color(&cfg.appearance.muted_color, 0.85)
            } else {
                parse_color(&cfg.appearance.muted_color, 0.50)
            };
            draw_adwaita_eye_icon(target, peek_rect, state.reveal_password, peek_color);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_cursor_glide(
        &self,
        target: &mut PixmapMut,
        cfg: &Config,
        state: &CardUiState,
        base_x: f32,
        scroll_x: f32,
        max_text_w: f32,
        input_y: f32,
        input_h: f32,
    ) {
        let cursor_h = input_h * 0.44;
        let cursor_y = input_y + (input_h - cursor_h) / 2.0;

        let rel_cursor = (state.glide.cur_x - scroll_x).clamp(0.0, max_text_w);
        let glide_x = base_x + rel_cursor;
        let cursor_color = parse_color(&cfg.input.cursor_glide.color, 0.95);

        if let Some((smear_left, smear_w)) = state.glide.current_smear {
            let rel_left = (smear_left - scroll_x).max(0.0);
            let rel_right = (smear_left + smear_w - scroll_x).min(max_text_w + cfg.input.cursor_glide.width);
            let clamped_w = (rel_right - rel_left).max(0.0);
            if clamped_w > cfg.input.cursor_glide.width
                && let Some(smear_rect) =
                    Rect::from_xywh(base_x + rel_left, cursor_y, clamped_w, cursor_h)
            {
                let smear_color = parse_color(
                    &cfg.input.cursor_glide.color,
                    0.35 * cfg.input.cursor_glide.trail_smear,
                );
                let mut smear_paint = Paint::default();
                smear_paint.set_color(smear_color);
                let smear_path =
                    rounded_rect_path(smear_rect, cfg.input.cursor_glide.corner_radius);
                target.fill_path(
                    &smear_path,
                    &smear_paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }

        if let Some(cursor_rect) =
            Rect::from_xywh(glide_x, cursor_y, cfg.input.cursor_glide.width, cursor_h)
        {
            let mut cursor_paint = Paint::default();
            cursor_paint.set_color(cursor_color);
            let cursor_path = rounded_rect_path(cursor_rect, cfg.input.cursor_glide.corner_radius);
            target.fill_path(
                &cursor_path,
                &cursor_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    pub fn compute_dot_x_offset(
        &self,
        cfg: &Config,
        password_len: usize,
        password_text: &str,
        reveal: bool,
    ) -> f32 {
        if password_len == 0 {
            return 0.0;
        }
        if reveal {
            let (tw, _) = self.font.measure_text(password_text, 14.0);
            tw
        } else {
            let (dw, _) = self.font.measure_text(&cfg.input.dot_char, 16.0);
            let step = dw + cfg.input.dot_spacing;
            step * password_len as f32
        }
    }
}

pub fn compute_card_height(cfg: &Config, capslock: bool) -> f32 {
    let mut h = cfg.card.padding_y * 2.0;
    if cfg.card.show_avatar {
        h += cfg.card.avatar_size + 14.0;
    }
    h += 34.0;
    h += cfg.input.height;
    if capslock && cfg.keyboard.show_capslock_warning {
        h += 28.0;
    }
    h.round()
}

pub fn compute_input_row_geometry(
    card_rect: Rect,
    cfg: &Config,
    content_y: f32,
) -> (Rect, Rect, Rect) {
    let row_x = card_rect.x() + cfg.card.padding_x;
    let row_w = card_rect.width() - (cfg.card.padding_x * 2.0);
    let row_h = cfg.input.height;

    let btn_size = row_h;
    let gap = 10.0;
    let input_w = (row_w - btn_size - gap).max(100.0);

    let input_rect = Rect::from_xywh(row_x, content_y, input_w, row_h).unwrap();
    let submit_rect =
        Rect::from_xywh(row_x + input_w + gap, content_y, btn_size, btn_size).unwrap();

    let peek_size = 20.0;
    let peek_x = input_rect.x() + input_rect.width() - peek_size - 11.0;
    let peek_y = input_rect.y() + (input_rect.height() - peek_size) / 2.0;
    let peek_rect = Rect::from_xywh(peek_x, peek_y, peek_size, peek_size).unwrap();

    (input_rect, peek_rect, submit_rect)
}

fn draw_adwaita_eye_icon(target: &mut PixmapMut, rect: Rect, revealed: bool, color: Color) {
    let s = (rect.width() / 16.0).min(rect.height() / 16.0);
    let sx = rect.x() + (rect.width() - 16.0 * s) / 2.0;
    let sy = rect.y() + (rect.height() - 16.0 * s) / 2.0;

    let mut pb = PathBuilder::new();

    // 1. Outer eye contour
    pb.move_to(sx + 8.0 * s, sy + 2.0 * s);
    pb.cubic_to(
        sx + 4.3515 * s,
        sy + 2.0039 * s,
        sx + 1.1680 * s,
        sy + 4.4766 * s,
        sx + 0.2617 * s,
        sy + 8.0078 * s,
    );
    pb.cubic_to(
        sx + 1.1758 * s,
        sy + 11.5351 * s,
        sx + 4.3594 * s,
        sy + 13.9961 * s,
        sx + 8.0 * s,
        sy + 14.0 * s,
    );
    pb.cubic_to(
        sx + 11.6484 * s,
        sy + 13.9961 * s,
        sx + 14.8320 * s,
        sy + 11.5351 * s,
        sx + 15.7383 * s,
        sy + 8.0078 * s,
    );
    pb.cubic_to(
        sx + 14.8242 * s,
        sy + 4.4648 * s,
        sx + 11.6406 * s,
        sy + 2.0039 * s,
        sx + 8.0 * s,
        sy + 2.0 * s,
    );
    pb.close();

    // 2. Middle cutout circle (r=4 at 8,8)
    pb.move_to(sx + 8.0 * s, sy + 4.0 * s);
    pb.cubic_to(
        sx + 10.2109 * s,
        sy + 4.0 * s,
        sx + 12.0 * s,
        sy + 5.7891 * s,
        sx + 12.0 * s,
        sy + 8.0 * s,
    );
    pb.cubic_to(
        sx + 12.0 * s,
        sy + 10.2109 * s,
        sx + 10.2109 * s,
        sy + 12.0 * s,
        sx + 8.0 * s,
        sy + 12.0 * s,
    );
    pb.cubic_to(
        sx + 5.7891 * s,
        sy + 12.0 * s,
        sx + 4.0 * s,
        sy + 10.2109 * s,
        sx + 4.0 * s,
        sy + 8.0 * s,
    );
    pb.cubic_to(
        sx + 4.0 * s,
        sy + 5.7891 * s,
        sx + 5.7891 * s,
        sy + 4.0 * s,
        sx + 8.0 * s,
        sy + 4.0 * s,
    );
    pb.close();

    // 3. Inner pupil circle (r=2 at 8,8)
    pb.move_to(sx + 8.0 * s, sy + 6.0 * s);
    pb.cubic_to(
        sx + 6.8945 * s,
        sy + 6.0 * s,
        sx + 6.0 * s,
        sy + 6.8945 * s,
        sx + 6.0 * s,
        sy + 8.0 * s,
    );
    pb.cubic_to(
        sx + 6.0 * s,
        sy + 9.1055 * s,
        sx + 6.8945 * s,
        sy + 10.0 * s,
        sx + 8.0 * s,
        sy + 10.0 * s,
    );
    pb.cubic_to(
        sx + 9.1055 * s,
        sy + 10.0 * s,
        sx + 10.0 * s,
        sy + 9.1055 * s,
        sx + 10.0 * s,
        sy + 8.0 * s,
    );
    pb.cubic_to(
        sx + 10.0 * s,
        sy + 6.8945 * s,
        sx + 9.1055 * s,
        sy + 6.0 * s,
        sx + 8.0 * s,
        sy + 6.0 * s,
    );
    pb.close();

    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        target.fill_path(&path, &paint, FillRule::EvenOdd, Transform::identity(), None);
    }

    if revealed {
        let mut slash_pb = PathBuilder::new();
        slash_pb.move_to(sx + 1.5312 * s, sy + 0.4687 * s);
        slash_pb.line_to(sx + 15.5312 * s, sy + 14.4687 * s);
        if let Some(slash_path) = slash_pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(color);
            paint.anti_alias = true;
            let stroke = Stroke {
                width: 1.8 * s,
                line_cap: LineCap::Round,
                ..Default::default()
            };
            target.stroke_path(&slash_path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

fn rounded_rect_path(rect: Rect, radius: f32) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);

    let x = rect.x();
    let y = rect.y();
    let w = rect.width();
    let h = rect.height();

    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();

    pb.finish().unwrap()
}

pub fn format_local_time(fmt: &str) -> String {
    let mut t: libc::time_t = 0;
    unsafe {
        libc::time(&mut t);
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        if libc::localtime_r(&t, &mut tm).is_null() {
            return String::new();
        }
    }
    let Ok(c_fmt) = std::ffi::CString::new(fmt) else {
        return String::new();
    };
    let mut buf = [0u8; 128];
    let len = unsafe {
        libc::strftime(
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            c_fmt.as_ptr(),
            &tm,
        )
    };
    if len == 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..len]).into_owned()
}
