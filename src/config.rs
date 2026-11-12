use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tiny_skia::Color;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub user: UserConfig,
    #[serde(default)]
    pub card: CardConfig,
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
    #[serde(default)]
    pub wallpaper: WallpaperConfig,
    #[serde(default)]
    pub keyboard: KeyboardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default = "default_user")]
    pub default: String,
    #[serde(default = "default_true")]
    pub show_display_name: bool,
    #[serde(default)]
    pub display_name: Option<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            default: default_user(),
            show_display_name: true,
            display_name: None,
        }
    }
}

fn default_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "user".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardConfig {
    #[serde(default = "default_card_bg")]
    pub background_color: String,
    #[serde(default = "default_card_opacity")]
    pub opacity: f32,
    #[serde(default = "default_border_color")]
    pub border_color: String,
    #[serde(default = "default_border_opacity")]
    pub border_opacity: f32,
    #[serde(default = "default_border_radius")]
    pub border_radius: f32,
    #[serde(default = "default_card_width")]
    pub width: f32,
    #[serde(default = "default_padding_x")]
    pub padding_x: f32,
    #[serde(default = "default_padding_y")]
    pub padding_y: f32,
    #[serde(default = "default_true")]
    pub show_avatar: bool,
    #[serde(default = "default_avatar_size")]
    pub avatar_size: f32,
    #[serde(default = "default_avatar_border_width")]
    pub avatar_border_width: f32,
    #[serde(default)]
    pub avatar_border_color: Option<String>,
}

impl Default for CardConfig {
    fn default() -> Self {
        Self {
            background_color: default_card_bg(),
            opacity: default_card_opacity(),
            border_color: default_border_color(),
            border_opacity: default_border_opacity(),
            border_radius: default_border_radius(),
            width: default_card_width(),
            padding_x: default_padding_x(),
            padding_y: default_padding_y(),
            show_avatar: true,
            avatar_size: default_avatar_size(),
            avatar_border_width: default_avatar_border_width(),
            avatar_border_color: None,
        }
    }
}

fn default_card_bg() -> String {
    "#282828".to_string()
}
fn default_card_opacity() -> f32 {
    0.84
}
fn default_border_color() -> String {
    "#928374".to_string()
}
fn default_border_opacity() -> f32 {
    0.45
}
fn default_border_radius() -> f32 {
    18.0
}
fn default_card_width() -> f32 {
    410.0
}
fn default_padding_x() -> f32 {
    26.0
}
fn default_padding_y() -> f32 {
    24.0
}
fn default_avatar_size() -> f32 {
    96.0
}
fn default_avatar_border_width() -> f32 {
    2.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    #[serde(default = "default_input_bg")]
    pub background_color: String,
    #[serde(default = "default_input_opacity")]
    pub opacity: f32,
    #[serde(default = "default_border_color")]
    pub border_color: String,
    #[serde(default = "default_border_opacity")]
    pub border_opacity: f32,
    #[serde(default)]
    pub focus_border_color: Option<String>,
    #[serde(default = "default_input_radius")]
    pub border_radius: f32,
    #[serde(default = "default_input_height")]
    pub height: f32,
    #[serde(default = "default_dot_char")]
    pub dot_char: String,
    #[serde(default = "default_dot_spacing")]
    pub dot_spacing: f32,
    #[serde(default = "default_placeholder")]
    pub placeholder: String,
    #[serde(default = "default_true")]
    pub show_peek_icon: bool,
    #[serde(default)]
    pub cursor_glide: CursorGlideConfig,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            background_color: default_input_bg(),
            opacity: default_input_opacity(),
            border_color: default_border_color(),
            border_opacity: default_border_opacity(),
            focus_border_color: None,
            border_radius: default_input_radius(),
            height: default_input_height(),
            dot_char: default_dot_char(),
            dot_spacing: default_dot_spacing(),
            placeholder: default_placeholder(),
            show_peek_icon: true,
            cursor_glide: CursorGlideConfig::default(),
        }
    }
}

fn default_input_bg() -> String {
    "#1d2021".to_string()
}
fn default_input_opacity() -> f32 {
    0.45
}
fn default_input_radius() -> f32 {
    999.0
}
fn default_input_height() -> f32 {
    42.0
}
fn default_dot_char() -> String {
    "•".to_string()
}
fn default_dot_spacing() -> f32 {
    2.0
}
fn default_placeholder() -> String {
    "Enter password".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorGlideConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_glide_duration")]
    pub duration_ms: u64,
    #[serde(default = "default_glide_width")]
    pub width: f32,
    #[serde(default = "default_glide_radius")]
    pub corner_radius: f32,
    #[serde(default = "default_glide_trail")]
    pub trail_smear: f32,
    #[serde(default = "default_glide_color")]
    pub color: String,
}

impl Default for CursorGlideConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            duration_ms: default_glide_duration(),
            width: default_glide_width(),
            corner_radius: default_glide_radius(),
            trail_smear: default_glide_trail(),
            color: default_glide_color(),
        }
    }
}

fn default_glide_duration() -> u64 {
    150
}
fn default_glide_width() -> f32 {
    3.0
}
fn default_glide_radius() -> f32 {
    1.5
}
fn default_glide_trail() -> f32 {
    0.85
}
fn default_glide_color() -> String {
    "#d4be98".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_text_color")]
    pub text_color: String,
    #[serde(default = "default_muted_color")]
    pub muted_color: String,
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    #[serde(default = "default_accent_text_color")]
    pub accent_text_color: String,
    #[serde(default = "default_error_color")]
    pub error_color: String,
    #[serde(default = "default_backdrop_dim")]
    pub backdrop_dim: f32,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            font_family: default_font_family(),
            text_color: default_text_color(),
            muted_color: default_muted_color(),
            accent_color: default_accent_color(),
            accent_text_color: default_accent_text_color(),
            error_color: default_error_color(),
            backdrop_dim: default_backdrop_dim(),
        }
    }
}

fn default_font_family() -> String {
    "Adwaita Sans".to_string()
}
fn default_text_color() -> String {
    "#ebdbb2".to_string()
}
fn default_muted_color() -> String {
    "#bdae93".to_string()
}
fn default_accent_color() -> String {
    "#b8bb26".to_string()
}
fn default_accent_text_color() -> String {
    "#1d2021".to_string()
}
fn default_error_color() -> String {
    "#fb4934".to_string()
}
fn default_backdrop_dim() -> f32 {
    0.12
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperConfig {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default = "default_fill_mode")]
    pub fill_mode: String,
    #[serde(default)]
    pub blur_sigma: f32,
    #[serde(default = "default_solid_color")]
    pub solid_color: String,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            path: None,
            fill_mode: default_fill_mode(),
            blur_sigma: 0.0,
            solid_color: default_solid_color(),
        }
    }
}

fn default_fill_mode() -> String {
    "cover".to_string()
}
fn default_solid_color() -> String {
    "#1d2021".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardConfig {
    #[serde(default = "default_true")]
    pub show_capslock_warning: bool,
    #[serde(default = "default_true")]
    pub show_layout: bool,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            show_capslock_warning: true,
            show_layout: true,
        }
    }
}

fn default_true() -> bool {
    true
}

impl Config {
    pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
        let path = Self::resolve_path(explicit_path);
        if let Some(path) = path.filter(|p| p.exists()) {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file at {:?}", path))?;
            let config: Self = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse YAML config from {:?}", path))?;
            return Ok(config);
        }
        Ok(Self::default())
    }

    fn resolve_path(explicit_path: Option<&Path>) -> Option<PathBuf> {
        if let Some(p) = explicit_path {
            return Some(p.to_path_buf());
        }
        if let Ok(home) = std::env::var("HOME") {
            let klock_cfg = PathBuf::from(&home).join(".config/klock/config.yaml");
            if klock_cfg.exists() {
                return Some(klock_cfg);
            }
            let kgreet_cfg = PathBuf::from(&home).join(".config/kgreet/config.yaml");
            if kgreet_cfg.exists() {
                return Some(kgreet_cfg);
            }
        }
        let sys_cfg = PathBuf::from("/etc/klock/config.yaml");
        if sys_cfg.exists() {
            return Some(sys_cfg);
        }
        None
    }

    pub fn resolve_wallpaper(&self) -> Option<PathBuf> {
        if let Some(ref p) = self.wallpaper.path {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let noctalia_wall = PathBuf::from(&home).join("Pictures/lockscreens/current.jpg");
            if noctalia_wall.exists() {
                return Some(noctalia_wall);
            }
        }
        let greeter_wall = PathBuf::from("/usr/share/backgrounds/greeter-wallpaper.jpg");
        if greeter_wall.exists() {
            return Some(greeter_wall);
        }
        None
    }

    pub fn resolve_avatar(&self) -> Option<PathBuf> {
        if let Ok(home) = std::env::var("HOME") {
            let local_avatar = PathBuf::from(&home).join(".local/share/icons/user-avatar.png");
            if local_avatar.exists() {
                return Some(local_avatar);
            }
            let face_png = PathBuf::from(&home).join(".face");
            if face_png.exists() {
                return Some(face_png);
            }
        }
        let acct_avatar = PathBuf::from(format!(
            "/var/lib/AccountsService/icons/{}",
            self.user.default
        ));
        if acct_avatar.exists() {
            return Some(acct_avatar);
        }
        None
    }
}

pub fn parse_color(hex: &str, alpha: f32) -> Color {
    let clean = hex.trim_start_matches('#');
    let (r, g, b) = match clean.len() {
        6 => {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0);
            (r, g, b)
        }
        3 => {
            let r = u8::from_str_radix(&clean[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&clean[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&clean[2..3], 16).unwrap_or(0) * 17;
            (r, g, b)
        }
        _ => (40, 40, 40),
    };
    Color::from_rgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        alpha.clamp(0.0, 1.0),
    )
    .unwrap_or(Color::BLACK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_parses() {
        let yaml = "user:\n  default: 'testuser'\ncard:\n  width: 420.0\n";
        let cfg: Config = serde_yaml::from_str(yaml).expect("YAML parse failed");
        assert_eq!(cfg.user.default, "testuser");
        assert_eq!(cfg.card.width, 420.0);
        assert_eq!(cfg.appearance.accent_color, "#b8bb26");
    }

    #[test]
    fn test_parse_color() {
        let c = parse_color("#b8bb26", 1.0);
        assert!((c.red() - 0.72).abs() < 0.05);
        assert!((c.green() - 0.73).abs() < 0.05);
        assert_eq!(c.alpha(), 1.0);
    }
}
