# klock

Wayland session lockscreen written in Rust using `ext-session-lock-v1` and `tiny-skia`, matching `kgreet` styling.

![klock preview](assets/preview.webp)

## Features

- Detailed YAML configuration (`config.yaml` / `config.yml`, see [config.example.yaml](config.example.yaml)).
- Configurable translucent or solid surfaces for the lock card and password input.
- Display name header resolved from `user.display_name` or `/etc/passwd` GECOS.
- Configurable clock, date (`strftime`), and optional third subtitle line (`{os}`, `{session}`, `{host}`, `{user}`).
- Glide-animated vertical bar cursor on the password input (`input.cursor_glide`) with horizontal overflow scrolling.
- Peek password toggle (`view-reveal-symbolic` / `view-conceal-symbolic`), inline submit button, and shake animation on failed authentication.
- Non-blocking PAM authentication with `zeroize`-guarded credential memory.
- Static images with `cover` scaling, Gaussian blur, and dimmed backdrop (`wallpaper.blur_sigma`, `appearance.backdrop_dim`).
- Primary-monitor login card with secondary monitor lock coverage.

## Build and local test

```bash
cargo build --release
cargo run -- --windowed --mock-pam --config config.example.yaml
```

In `--mock-pam` mode, `"test"`, `"mock"`, or an empty password unlocks and exits, while any other input triggers the error shake animation. Use `--dev-escape` to allow unlocking via `Escape` during fullscreen testing.

## Configuration

`klock` searches for its YAML config in this order (unless `-c` / `--config` is passed):

1. `~/.config/klock/config.yaml`
2. `~/.config/kgreet/config.yaml` (automatic theme fallback)
3. `/etc/klock/config.yaml`

## Integration

`klock` requires a Wayland compositor implementing `ext-session-lock-v1` (such as `umbriel`, `sway`, or `hyprland`) and a PAM service file at `/etc/pam.d/klock`:

```text
#%PAM-1.0
auth      include   system-auth
account   include   system-auth
password  include   system-auth
session   include   system-auth
```

Example `hypridle` configuration:

```ini
general {
    lock_cmd = pidof klock || klock
}
```
