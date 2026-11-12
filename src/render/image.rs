use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageReader};
use std::path::Path;
use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

pub fn load_image(path: &Path) -> Result<DynamicImage> {
    ImageReader::open(path)
        .with_context(|| format!("Failed to open image at {:?}", path))?
        .with_guessed_format()?
        .decode()
        .with_context(|| format!("Failed to decode image at {:?}", path))
}

pub fn render_wallpaper(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    blur_sigma: f32,
    dim: f32,
) -> Result<Pixmap> {
    let (src_w, src_h) = img.dimensions();
    if src_w == 0 || src_h == 0 || target_width == 0 || target_height == 0 {
        anyhow::bail!("Invalid dimensions for wallpaper rendering");
    }

    let scale_x = target_width as f32 / src_w as f32;
    let scale_y = target_height as f32 / src_h as f32;
    let scale = scale_x.max(scale_y);

    let scaled_w = (src_w as f32 * scale).round() as u32;
    let scaled_h = (src_h as f32 * scale).round() as u32;

    let resized = img.resize_exact(scaled_w, scaled_h, image::imageops::FilterType::Triangle);

    let crop_x = (scaled_w.saturating_sub(target_width)) / 2;
    let crop_y = (scaled_h.saturating_sub(target_height)) / 2;
    let cropped = resized.crop_imm(crop_x, crop_y, target_width, target_height);

    let mut rgba_img = cropped.to_rgba8();

    if blur_sigma > 0.5 {
        let (w, h) = (rgba_img.width() as usize, rgba_img.height() as usize);
        let mut rgb: Vec<[u8; 3]> = rgba_img
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        fastblur::gaussian_blur(&mut rgb, w, h, blur_sigma);
        for (dst, src) in rgba_img.as_chunks_mut::<4>().0.iter_mut().zip(rgb.iter()) {
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
        }
    }

    let mut pixmap = Pixmap::new(target_width, target_height)
        .context("Failed to allocate pixmap for wallpaper")?;

    pixmap.data_mut().copy_from_slice(rgba_img.as_raw());

    if dim > 0.001 {
        let dim_alpha = dim.clamp(0.0, 1.0);
        let black = Color::from_rgba(0.0, 0.0, 0.0, dim_alpha).unwrap();
        let mut dim_pix = Pixmap::new(target_width, target_height).unwrap();
        dim_pix.fill(black);
        pixmap.draw_pixmap(
            0,
            0,
            dim_pix.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    Ok(pixmap)
}

pub fn render_circular_avatar(
    img: &DynamicImage,
    size: u32,
    border_width: f32,
    border_color: Option<Color>,
) -> Result<Pixmap> {
    let scaled = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
    let rgba = scaled.to_rgba8();

    let mut pixmap = Pixmap::new(size, size).context("Failed to allocate avatar pixmap")?;
    let radius = size as f32 / 2.0;
    let radius_sq = radius * radius;
    let inner_radius = radius - border_width;
    let inner_radius_sq = inner_radius * inner_radius;

    let src_raw = rgba.as_raw();
    let dst_raw = pixmap.data_mut();

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - radius;
            let dy = y as f32 + 0.5 - radius;
            let dist_sq = dx * dx + dy * dy;

            let idx = ((y * size + x) * 4) as usize;

            if dist_sq > radius_sq {
                dst_raw[idx..idx + 4].fill(0);
            } else if dist_sq > inner_radius_sq && border_width > 0.0 {
                if let Some(color) = border_color {
                    dst_raw[idx] = (color.red() * 255.0) as u8;
                    dst_raw[idx + 1] = (color.green() * 255.0) as u8;
                    dst_raw[idx + 2] = (color.blue() * 255.0) as u8;
                    dst_raw[idx + 3] = (color.alpha() * 255.0) as u8;
                } else {
                    dst_raw[idx..idx + 4].copy_from_slice(&src_raw[idx..idx + 4]);
                }
            } else {
                dst_raw[idx..idx + 4].copy_from_slice(&src_raw[idx..idx + 4]);
            }
        }
    }

    Ok(pixmap)
}
