use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputInfo, OutputState},
    reexports::calloop::{
        LoopHandle,
        timer::{TimeoutAction, Timer},
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
    },
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
    shell::{
        WaylandSurface,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use tiny_skia::PixmapMut;
use wayland_client::{
    Connection, Proxy, QueueHandle,
    globals::GlobalList,
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
};
use zeroize::{Zeroize, Zeroizing};

use crate::auth::{AuthResponse, AuthState, AuthWorker};
use crate::config::Config;
use crate::render::animations::{CursorGlideState, ShakeAnimation};
use crate::render::{
    CardUiState, UiRenderer, load_image, render_circular_avatar, render_wallpaper,
};

pub struct SurfaceView {
    pub surface: wl_surface::WlSurface,
    pub _lock_surface: Option<SessionLockSurface>,
    pub window: Option<Window>,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub pool: SlotPool,
}

pub struct LockApp {
    pub conn: Connection,
    pub loop_handle: LoopHandle<'static, Self>,
    pub registry_state: RegistryState,
    pub compositor_state: CompositorState,
    pub output_state: OutputState,
    pub seat_state: SeatState,
    pub shm: Shm,

    pub session_lock_state: Option<SessionLockState>,
    pub session_lock: Option<SessionLock>,
    pub xdg_shell: Option<XdgShell>,

    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    pub surfaces: HashMap<u32, SurfaceView>,

    pub cfg: Config,
    pub ui: UiRenderer,
    pub auth_worker: AuthWorker,
    pub auth_state: AuthState,

    pub password: Zeroizing<String>,
    pub glide: CursorGlideState,
    pub shake: ShakeAnimation,
    pub capslock: bool,
    pub layout_index: u32,

    pub is_windowed: bool,
    pub dev_escape: bool,
    pub target_monitor: Option<String>,
    pub exit: bool,
}

impl LockApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conn: Connection,
        loop_handle: LoopHandle<'static, Self>,
        cfg: Config,
        is_windowed: bool,
        mock_pam: bool,
        dev_escape: bool,
        target_monitor: Option<String>,
        qh: &QueueHandle<Self>,
        globals: &GlobalList,
    ) -> Result<Self> {
        let registry_state = RegistryState::new(globals);
        let compositor_state =
            CompositorState::bind(globals, qh).context("Failed to bind wl_compositor")?;
        let output_state = OutputState::new(globals, qh);
        let seat_state = SeatState::new(globals, qh);
        let shm = Shm::bind(globals, qh).context("Failed to bind wl_shm")?;

        let (session_lock_state, xdg_shell) = if is_windowed {
            let xdg = XdgShell::bind(globals, qh).context("Failed to bind xdg_shell")?;
            (None, Some(xdg))
        } else {
            let lock = SessionLockState::new(globals, qh);
            (Some(lock), None)
        };

        let mut ui = UiRenderer::new().context("Failed to initialize UI renderer")?;

        if let Some(wall_path) = cfg.resolve_wallpaper()
            && let Ok(wall_img) = load_image(&wall_path)
            && let Ok(wall_pix) = render_wallpaper(
                &wall_img,
                1920,
                1080,
                cfg.wallpaper.blur_sigma,
                cfg.appearance.backdrop_dim,
            )
        {
            ui.set_wallpaper(wall_pix);
        }

        if cfg.card.show_avatar
            && let Some(avatar_path) = cfg.resolve_avatar()
            && let Ok(av_img) = load_image(&avatar_path)
        {
            let av_size = cfg.card.avatar_size as u32;
            let border_col = cfg
                .card
                .avatar_border_color
                .as_deref()
                .map(|c| crate::config::parse_color(c, 1.0));
            if let Ok(av_pix) =
                render_circular_avatar(&av_img, av_size, cfg.card.avatar_border_width, border_col)
            {
                ui.set_avatar(av_pix);
            }
        }

        let auth_worker = AuthWorker::spawn(mock_pam);
        let glide = CursorGlideState::new(cfg.input.cursor_glide.duration_ms);

        Ok(Self {
            conn,
            loop_handle,
            registry_state,
            compositor_state,
            output_state,
            seat_state,
            shm,
            session_lock_state,
            session_lock: None,
            xdg_shell,
            keyboard: None,
            surfaces: HashMap::new(),
            cfg,
            ui,
            auth_worker,
            auth_state: AuthState::Idle,
            password: Zeroizing::new(String::new()),
            glide,
            shake: ShakeAnimation::new(),
            capslock: false,
            layout_index: 0,
            is_windowed,
            dev_escape,
            target_monitor,
            exit: false,
        })
    }

    pub fn start(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        if self.is_windowed {
            let xdg = self.xdg_shell.as_ref().unwrap();
            let surface = self.compositor_state.create_surface(qh);
            let window = xdg.create_window(surface.clone(), WindowDecorations::RequestServer, qh);
            window.set_title("klock preview");
            window.set_app_id("klock");
            window.set_min_size(Some((960, 540)));
            window.commit();

            let pool = SlotPool::new(1920 * 1080 * 4, &self.shm)?;
            let id = surface.id().protocol_id();
            self.surfaces.insert(
                id,
                SurfaceView {
                    surface,
                    _lock_surface: None,
                    window: Some(window),
                    width: 960,
                    height: 540,
                    is_primary: true,
                    pool,
                },
            );
        } else {
            let lock_state = self.session_lock_state.as_ref().unwrap();
            let lock = lock_state
                .lock(qh)
                .context("Compositor does not support ext-session-lock-v1")?;
            self.session_lock = Some(lock);

            let outputs: Vec<wl_output::WlOutput> = self.output_state.outputs().collect();
            for output in outputs {
                self.create_lock_surface_for_output(&output, qh)?;
            }
        }

        self.schedule_animation_timer();
        Ok(())
    }

    fn create_lock_surface_for_output(
        &mut self,
        output: &wl_output::WlOutput,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let lock = match self.session_lock.as_ref() {
            Some(l) => l,
            None => return Ok(()),
        };

        let surface = self.compositor_state.create_surface(qh);
        let lock_surface = lock.create_lock_surface(surface.clone(), output, qh);

        let info = self.output_state.info(output);
        let is_primary = self.is_target_output(info.as_ref());

        let pool = SlotPool::new(1920 * 1080 * 4, &self.shm)?;
        let id = surface.id().protocol_id();

        self.surfaces.insert(
            id,
            SurfaceView {
                surface,
                _lock_surface: Some(lock_surface),
                window: None,
                width: 1920,
                height: 1080,
                is_primary,
                pool,
            },
        );

        Ok(())
    }

    fn is_target_output(&self, info: Option<&OutputInfo>) -> bool {
        let name = info.and_then(|i| i.name.as_deref()).unwrap_or("");
        if let Some(ref target) = self.target_monitor {
            if target == "primary" {
                return name == "DP-2" || name.starts_with("eDP") || self.surfaces.is_empty();
            }
            return name == target;
        }
        name == "DP-2" || self.surfaces.is_empty()
    }

    fn schedule_animation_timer(&self) {
        self.loop_handle
            .insert_source(
                Timer::from_duration(Duration::from_millis(8)),
                |_deadline, _, app: &mut Self| {
                    app.tick();
                    TimeoutAction::ToDuration(Duration::from_millis(8))
                },
            )
            .expect("Failed to insert animation timer");
    }

    pub fn tick(&mut self) {
        if let Some(res) = self.auth_worker.try_recv() {
            match res {
                AuthResponse::Success => {
                    self.auth_state = AuthState::Success;
                    self.unlock_and_exit();
                    return;
                }
                AuthResponse::Failure(_err) => {
                    self.auth_state = AuthState::Failure;
                    self.password.zeroize();
                    self.password.clear();
                    self.glide.set_target(0.0);
                    self.shake.trigger();
                }
            }
        }

        let glide_moving = self.glide.update();
        let shake_moving = self.shake.is_active();

        if glide_moving || shake_moving {
            self.redraw_all();
        }
    }

    pub fn redraw_all(&mut self) {
        let surface_ids: Vec<u32> = self.surfaces.keys().copied().collect();
        for id in surface_ids {
            self.redraw_surface(id);
        }
    }

    pub fn redraw_surface(&mut self, surface_id: u32) {
        let view = match self.surfaces.get_mut(&surface_id) {
            Some(v) => v,
            None => return,
        };

        if view.width == 0 || view.height == 0 {
            return;
        }

        let w = view.width;
        let h = view.height;
        let stride = (w * 4) as i32;

        let (buffer, canvas) =
            match view
                .pool
                .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Abgr8888)
            {
                Ok(b) => b,
                Err(_) => return,
            };

        let mut pixmap = match PixmapMut::from_bytes(canvas, w, h) {
            Some(p) => p,
            None => return,
        };

        let layout_name = if self.layout_index == 1 { "RU" } else { "US" };
        let state = CardUiState {
            username: &self.cfg.user.default,
            display_name: self.cfg.user.display_name.as_deref(),
            password_len: self.password.len(),
            capslock: self.capslock,
            layout: layout_name,
            error: self.auth_state == AuthState::Failure,
            glide: &self.glide,
            shake: &self.shake,
        };

        self.ui
            .render_frame(&mut pixmap, &self.cfg, &state, view.is_primary);

        view.surface.attach(Some(buffer.wl_buffer()), 0, 0);
        view.surface.damage_buffer(0, 0, w as i32, h as i32);
        view.surface.commit();
    }

    pub fn unlock_and_exit(&mut self) {
        if let Some(lock) = self.session_lock.take() {
            lock.unlock();
            let _ = self.conn.roundtrip();
        }
        self.exit = true;
    }

    fn submit_auth(&mut self) {
        if self.auth_state == AuthState::Verifying {
            return;
        }
        self.auth_state = AuthState::Verifying;
        let secret = Zeroizing::new(self.password.to_string());
        self.auth_worker
            .submit(self.cfg.user.default.clone(), secret);
    }
}

impl SessionLockHandler for LockApp {
    fn locked(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _session_lock: SessionLock) {
        self.redraw_all();
    }

    fn finished(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _session_lock: SessionLock,
    ) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        session_lock_surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _serial: u32,
    ) {
        let (w, h) = configure.new_size;
        let id = session_lock_surface.wl_surface().id().protocol_id();
        if let Some(view) = self.surfaces.get_mut(&id) {
            view.width = w;
            view.height = h;
        }
        self.redraw_surface(id);
    }
}

impl WindowHandler for LockApp {
    fn request_close(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _window: &Window) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let new_w = configure.new_size.0.map(|v| v.get()).unwrap_or(960);
        let new_h = configure.new_size.1.map(|v| v.get()).unwrap_or(540);

        for view in self.surfaces.values_mut() {
            if view.window.is_some() {
                view.width = new_w;
                view.height = new_h;
            }
        }
        self.redraw_all();
    }
}

impl SeatHandler for LockApp {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard
            && self.keyboard.is_none()
            && let Ok(kbd) = self.seat_state.get_keyboard(qh, &seat, None)
        {
            self.keyboard = Some(kbd);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard
            && let Some(kbd) = self.keyboard.take()
        {
            kbd.release();
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

impl KeyboardHandler for LockApp {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        match event.keysym {
            Keysym::Return | Keysym::KP_Enter => {
                self.submit_auth();
            }
            Keysym::Escape => {
                if self.dev_escape || self.is_windowed {
                    self.unlock_and_exit();
                    return;
                }
                self.password.zeroize();
                self.password.clear();
                let target_x = self.ui.compute_dot_x_offset(&self.cfg, 0);
                self.glide.set_target(target_x);
                self.auth_state = AuthState::Idle;
                self.redraw_all();
            }
            Keysym::BackSpace => {
                if self.auth_state != AuthState::Verifying && !self.password.is_empty() {
                    self.password.pop();
                    let target_x = self.ui.compute_dot_x_offset(&self.cfg, self.password.len());
                    self.glide.set_target(target_x);
                    self.auth_state = AuthState::Idle;
                    self.redraw_all();
                }
            }
            _ => {
                if self.auth_state != AuthState::Verifying
                    && let Some(utf8) = event.utf8
                {
                    let filtered: String = utf8.chars().filter(|c| !c.is_control()).collect();
                    if !filtered.is_empty() {
                        self.password.push_str(&filtered);
                        let target_x = self.ui.compute_dot_x_offset(&self.cfg, self.password.len());
                        self.glide.set_target(target_x);
                        self.auth_state = AuthState::Idle;
                        self.redraw_all();
                    }
                }
            }
        }
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        keyboard: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.press_key(conn, qh, keyboard, serial, event);
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _raw: smithay_client_toolkit::seat::keyboard::RawModifiers,
        layout: u32,
    ) {
        let changed = self.capslock != modifiers.caps_lock || self.layout_index != layout;
        self.capslock = modifiers.caps_lock;
        self.layout_index = layout;
        if changed {
            self.redraw_all();
        }
    }
}

impl CompositorHandler for LockApp {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for LockApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if !self.is_windowed && self.session_lock.is_some() {
            let _ = self.create_lock_surface_for_output(&output, qh);
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl ShmHandler for LockApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for LockApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(LockApp);
wayland_client::delegate_noop!(LockApp: ignore wl_buffer::WlBuffer);
smithay_client_toolkit::delegate_dispatch2!(LockApp);
