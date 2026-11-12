use anyhow::Result;
use fontdue::{Font, FontSettings};
use std::path::Path;
use tiny_skia::{Color, PixmapMut};

pub struct FontRenderer {
    font: Font,
}

impl FontRenderer {
    pub fn load_default() -> Result<Self> {
        let candidates = [
            "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf",
            "/usr/share/fonts/adwaita-sans/AdwaitaSans-Regular.otf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ];

        for path in candidates {
            if Path::new(path).exists()
                && let Ok(bytes) = std::fs::read(path)
                && let Ok(font) = Font::from_bytes(bytes, FontSettings::default())
            {
                return Ok(Self { font });
            }
        }

        anyhow::bail!("No system fonts found for font renderer");
    }

    pub fn measure_text(&self, text: &str, px_size: f32) -> (f32, f32) {
        let mut width = 0.0;
        let mut max_height: f32 = 0.0;

        for ch in text.chars() {
            let metrics = self.font.metrics(ch, px_size);
            width += metrics.advance_width;
            max_height = max_height.max(metrics.height as f32);
        }

        (width, max_height)
    }

    pub fn measure_text_bold(&self, text: &str, px_size: f32) -> (f32, f32) {
        let (width, max_height) = self.measure_text(text, px_size);
        let char_count = text.chars().count();
        (width + (char_count as f32 * 0.65), max_height)
    }

    pub fn draw_text_bold(
        &self,
        pixmap: &mut PixmapMut,
        text: &str,
        mut x: f32,
        y: f32,
        px_size: f32,
        color: Color,
    ) {
        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, px_size);
            let gx = x + metrics.xmin as f32;
            let gy = y + px_size - metrics.ymin as f32 - metrics.height as f32;

            self.blend_glyph(
                pixmap,
                &bitmap,
                (metrics.width, metrics.height),
                (gx, gy),
                color,
            );
            self.blend_glyph(
                pixmap,
                &bitmap,
                (metrics.width, metrics.height),
                (gx + 0.65, gy),
                color,
            );
            self.blend_glyph(
                pixmap,
                &bitmap,
                (metrics.width, metrics.height),
                (gx + 0.35, gy + 0.35),
                color,
            );
            x += metrics.advance_width + 0.65;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_clipped(
        &self,
        pixmap: &mut PixmapMut,
        text: &str,
        mut x: f32,
        y: f32,
        px_size: f32,
        color: Color,
        clip_x: (f32, f32),
    ) {
        let clip_min = clip_x.0.floor() as i32;
        let clip_max = clip_x.1.ceil() as i32;
        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, px_size);
            let gx = x + metrics.xmin as f32;
            let gy = y + px_size - metrics.ymin as f32 - metrics.height as f32;

            self.blend_glyph_clipped(
                pixmap,
                &bitmap,
                (metrics.width, metrics.height),
                (gx, gy),
                color,
                Some((clip_min, clip_max)),
            );
            x += metrics.advance_width;
        }
    }

    pub fn draw_text(
        &self,
        pixmap: &mut PixmapMut,
        text: &str,
        mut x: f32,
        y: f32,
        px_size: f32,
        color: Color,
    ) {
        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, px_size);
            let gx = x + metrics.xmin as f32;
            let gy = y + px_size - metrics.ymin as f32 - metrics.height as f32;

            self.blend_glyph(
                pixmap,
                &bitmap,
                (metrics.width, metrics.height),
                (gx, gy),
                color,
            );
            x += metrics.advance_width;
        }
    }

    fn blend_glyph(
        &self,
        pixmap: &mut PixmapMut,
        bitmap: &[u8],
        size: (usize, usize),
        pos: (f32, f32),
        color: Color,
    ) {
        self.blend_glyph_clipped(pixmap, bitmap, size, pos, color, None);
    }

    fn blend_glyph_clipped(
        &self,
        pixmap: &mut PixmapMut,
        bitmap: &[u8],
        size: (usize, usize),
        pos: (f32, f32),
        color: Color,
        clip_x: Option<(i32, i32)>,
    ) {
        let (w, h) = size;
        let (gx, gy) = pos;
        let cr = color.red();
        let cg = color.green();
        let cb = color.blue();
        let ca = color.alpha();

        let pix_w = pixmap.width() as i32;
        let pix_h = pixmap.height() as i32;
        let (min_x, max_x) = match clip_x {
            Some((cmin, cmax)) => (cmin.max(0), cmax.min(pix_w)),
            None => (0, pix_w),
        };
        let stride = pixmap.width() as usize;
        let buf = pixmap.data_mut();

        for row in 0..h {
            let py = (gy as i32) + row as i32;
            if py < 0 || py >= pix_h {
                continue;
            }

            for col in 0..w {
                let px = (gx as i32) + col as i32;
                if px < min_x || px >= max_x {
                    continue;
                }

                let alpha_byte = bitmap[row * w + col];
                if alpha_byte == 0 {
                    continue;
                }

                let alpha_f = (alpha_byte as f32 / 255.0) * ca;
                let pixel_idx = (py as usize * stride + px as usize) * 4;

                let dst_r = buf[pixel_idx] as f32 / 255.0;
                let dst_g = buf[pixel_idx + 1] as f32 / 255.0;
                let dst_b = buf[pixel_idx + 2] as f32 / 255.0;
                let dst_a = buf[pixel_idx + 3] as f32 / 255.0;

                let out_a = alpha_f + dst_a * (1.0 - alpha_f);
                if out_a > 0.001 {
                    let out_r = (cr * alpha_f + dst_r * dst_a * (1.0 - alpha_f)) / out_a;
                    let out_g = (cg * alpha_f + dst_g * dst_a * (1.0 - alpha_f)) / out_a;
                    let out_b = (cb * alpha_f + dst_b * dst_a * (1.0 - alpha_f)) / out_a;

                    buf[pixel_idx] = (out_r.clamp(0.0, 1.0) * 255.0) as u8;
                    buf[pixel_idx + 1] = (out_g.clamp(0.0, 1.0) * 255.0) as u8;
                    buf[pixel_idx + 2] = (out_b.clamp(0.0, 1.0) * 255.0) as u8;
                    buf[pixel_idx + 3] = (out_a.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }
    }
}
