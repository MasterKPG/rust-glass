//! Window host: event loop, surface, clipboard, input method and the bridge
//! to screen readers. Draws only when something changes; an idle window uses
//! no CPU or GPU time.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WKey, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use crate::context::{Context, Cursor, HostCommand};
use crate::geometry::{Rect, vec2};
pub use crate::gpu::FrameStats;
use crate::gpu::{Gpu, Renderer};
use crate::input::{Event, Key, Modifiers};

/// Timings of recent frames, passed to [`App::update`].
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameInfo {
    /// Time spent in UI code and building the draw list, in milliseconds.
    pub ui_ms: f32,
    /// Time spent sending the frame to the GPU, in milliseconds.
    pub encode_ms: f32,
    /// What the renderer did.
    pub gpu: FrameStats,
    /// Time between frames while animating, in milliseconds.
    pub interval_ms: f32,
}

/// Your application. [`run`] calls it to draw each frame.
pub trait App {
    /// Called once, before the first frame. Load images here.
    fn setup(&mut self, _ctx: &mut Context) {}
    /// Describes the whole interface for this frame. Runs only when something
    /// changed or an animation is running.
    fn update(&mut self, ctx: &mut Context, info: &FrameInfo);
    /// Called when a file is dropped on the window.
    fn file_dropped(&mut self, _ctx: &mut Context, _path: &std::path::Path) {}
}

/// Settings of the window opened by [`run`].
#[derive(Clone, Debug)]
pub struct Options {
    /// Window title.
    pub title: String,
    /// Initial size in points. The window is made smaller if the screen is.
    pub size: (f32, f32),
    /// Smallest size in points.
    pub min_size: (f32, f32),
    /// Wait for the display refresh before showing a frame.
    pub vsync: bool,
    /// Prefer the integrated GPU on machines with two.
    pub low_power: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            title: "rust-glass".into(),
            size: (1180.0, 780.0),
            min_size: (480.0, 360.0),
            vsync: true,
            low_power: true,
        }
    }
}

/// Runs one frame of UI code, again when sizes were guessed.
pub(crate) fn run_ui(
    ctx: &mut Context,
    app: &mut dyn App,
    info: &FrameInfo,
    size: crate::geometry::Vec2,
    ppp: f32,
    time: f64,
    events: &[Event],
) {
    ctx.begin_frame(size, ppp, time, events);
    app.update(ctx, info);
    ctx.end_frame();
    let mut tries = 0;
    while ctx.wants_relayout() && tries < 3 {
        tries += 1;
        ctx.begin_frame(size, ppp, time, &[]);
        app.update(ctx, info);
        ctx.end_frame();
    }
    if ctx.wants_relayout() {
        ctx.request_repaint();
    }
}

struct Live {
    window: Arc<Window>,
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    cursor: Cursor,
    access: accesskit_winit::Adapter,
    /// Caret rectangle given to the input method, `None` while no field has focus.
    ime: Option<Rect>,
}

struct Host<A: App> {
    app: A,
    ctx: Context,
    options: Options,
    live: Option<Live>,
    proxy: EventLoopProxy<accesskit_winit::Event>,
    events: Vec<Event>,
    start: Instant,
    last: Instant,
    info: FrameInfo,
    clipboard: Option<arboard::Clipboard>,
    redraw: bool,
    was_animating: bool,
    error: Option<String>,
}

/// Opens a window and runs `app` until the window closes.
///
/// # Errors
///
/// Returns an error when the window or the GPU cannot be set up.
pub fn run(options: Options, app: impl App + 'static) -> Result<(), String> {
    let start = Instant::now();
    let el = EventLoop::with_user_event().build().map_err(|e| e.to_string())?;
    let mut host = Host {
        app,
        ctx: Context::new(),
        options,
        live: None,
        proxy: el.create_proxy(),
        events: Vec::new(),
        start,
        last: Instant::now(),
        info: FrameInfo::default(),
        clipboard: None,
        redraw: true,
        was_animating: false,
        error: None,
    };
    el.run_app(&mut host).map_err(|e| e.to_string())?;
    match host.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

impl<A: App> Host<A> {
    fn create(&mut self, el: &ActiveEventLoop) -> Result<Live, String> {
        let o = &self.options;
        let attrs = Window::default_attributes()
            .with_title(&o.title)
            .with_inner_size(LogicalSize::new(o.size.0, o.size.1))
            .with_min_inner_size(LogicalSize::new(o.min_size.0, o.min_size.1))
            // Shown after the first frame, so there is no flash.
            .with_visible(false);
        let window = Arc::new(el.create_window(attrs).map_err(|e| e.to_string())?);
        // Must exist before the window is shown.
        let access = accesskit_winit::Adapter::with_event_loop_proxy(el, &window, self.proxy.clone());
        fit_on_screen(&window);
        let instance = Gpu::instance();
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let gpu = Gpu::new(instance, Some(&surface), o.low_power)?;
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| matches!(f, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm))
            .unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or("the surface is not supported by the adapter")?;
        config.format = format;
        config.present_mode =
            if o.vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
        config.desired_maximum_frame_latency = 1;
        surface.configure(&gpu.device, &config);
        let renderer = Renderer::new(&gpu, format);
        log::info!("rendering on {}", gpu.describe());
        Ok(Live { window, gpu, surface, config, renderer, cursor: Cursor::Default, access, ime: None })
    }

    fn frame(&mut self) {
        let Some(live) = &mut self.live else { return };
        let PhysicalSize { width, height } = live.window.inner_size();
        if width == 0 || height == 0 {
            return;
        }
        if (width, height) != (live.config.width, live.config.height) {
            live.config.width = width;
            live.config.height = height;
            live.surface.configure(&live.gpu.device, &live.config);
        }
        let now = Instant::now();
        if self.was_animating {
            let iv = (now - self.last).as_secs_f32() * 1000.0;
            self.info.interval_ms =
                if self.info.interval_ms == 0.0 { iv } else { self.info.interval_ms * 0.9 + iv * 0.1 };
        }
        self.last = now;

        let stats = live.renderer.poll_stats();
        self.ctx.apply_stats(&stats);
        let ppp = live.window.scale_factor() as f32;
        let size = vec2(width as f32, height as f32) / ppp;
        let events = std::mem::take(&mut self.events);
        let time = (now - self.start).as_secs_f64();
        run_ui(&mut self.ctx, &mut self.app, &self.info, size, ppp, time, &events);
        if self.ctx.accessibility() {
            let (ctx, title) = (&self.ctx, &self.options.title);
            live.access.update_if_active(|| ctx.access_update(title));
        }
        if live.ime != self.ctx.ime_area {
            match self.ctx.ime_area {
                Some(r) => {
                    if live.ime.is_none() {
                        live.window.set_ime_allowed(true);
                    }
                    live.window.set_ime_cursor_area(
                        LogicalPosition::new(r.min.x, r.min.y),
                        LogicalSize::new(r.width().max(1.0), r.height().max(1.0)),
                    );
                }
                None => live.window.set_ime_allowed(false),
            }
            live.ime = self.ctx.ime_area;
        }
        let ui_done = Instant::now();
        let smooth = |old: f32, new: f32| if old == 0.0 { new } else { old * 0.9 + new * 0.1 };
        self.info.ui_ms = smooth(self.info.ui_ms, (ui_done - now).as_secs_f32() * 1000.0);

        for u in self.ctx.uploads.drain(..) {
            let id = live.renderer.add_image(u.w, u.h, &u.rgba);
            debug_assert_eq!(id, u.id);
        }
        for id in self.ctx.removed_images.drain(..) {
            live.renderer.remove_image(id);
        }
        live.renderer.upload_atlas(&mut self.ctx.text.uploads);

        use wgpu::CurrentSurfaceTexture as C;
        let frame = match live.surface.get_current_texture() {
            C::Success(t) => t,
            C::Suboptimal(t) => {
                // Use it now, reconfigure on the next frame.
                live.config.width = 0;
                t
            }
            C::Outdated | C::Lost => {
                live.surface.configure(&live.gpu.device, &live.config);
                self.redraw = true;
                return;
            }
            _ => {
                self.redraw = !live.window.is_minimized().unwrap_or(false);
                return;
            }
        };
        let enc = Instant::now();
        let view = frame.texture.create_view(&Default::default());
        live.renderer.render(&self.ctx.paint.data, &view, width, height);
        live.window.pre_present_notify();
        live.gpu.queue.present(frame);
        self.info.encode_ms = smooth(self.info.encode_ms, enc.elapsed().as_secs_f32() * 1000.0);
        self.info.gpu = live.renderer.frame_stats;

        if live.cursor != self.ctx.cursor {
            live.cursor = self.ctx.cursor;
            live.window.set_cursor(match live.cursor {
                Cursor::Default => CursorIcon::Default,
                Cursor::Pointer => CursorIcon::Pointer,
                Cursor::Text => CursorIcon::Text,
                Cursor::Grab => CursorIcon::Default,
                Cursor::ResizeHorizontal => CursorIcon::EwResize,
                Cursor::NotAllowed => CursorIcon::NotAllowed,
            });
        }
        self.was_animating = self.ctx.wants_repaint();
        self.redraw |= self.was_animating;
    }

    fn key(&mut self, k: &WKey) -> Option<Key> {
        Some(match k {
            WKey::Named(n) => match n {
                NamedKey::Enter => Key::Enter,
                NamedKey::Escape => Key::Escape,
                NamedKey::Tab => Key::Tab,
                NamedKey::Space => Key::Space,
                NamedKey::Backspace => Key::Backspace,
                NamedKey::Delete => Key::Delete,
                NamedKey::ArrowLeft => Key::Left,
                NamedKey::ArrowRight => Key::Right,
                NamedKey::ArrowUp => Key::Up,
                NamedKey::ArrowDown => Key::Down,
                NamedKey::Home => Key::Home,
                NamedKey::End => Key::End,
                NamedKey::PageUp => Key::PageUp,
                NamedKey::PageDown => Key::PageDown,
                _ => return None,
            },
            WKey::Character(s) => Key::Char(s.chars().next()?.to_ascii_lowercase()),
            _ => return None,
        })
    }
}

impl<A: App> ApplicationHandler<accesskit_winit::Event> for Host<A> {
    fn user_event(&mut self, _: &ActiveEventLoop, ev: accesskit_winit::Event) {
        use accesskit_winit::WindowEvent as W;
        match ev.window_event {
            W::InitialTreeRequested => self.ctx.set_accessibility(true),
            W::ActionRequested(req) => {
                if let Some((id, a)) = crate::accessibility::from_accesskit(&req) {
                    self.events.push(Event::Access(id, a));
                }
            }
            W::AccessibilityDeactivated => self.ctx.set_accessibility(false),
        }
        self.redraw = true;
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.live.is_some() {
            return;
        }
        match self.create(el) {
            Ok(l) => {
                self.live = Some(l);
                self.app.setup(&mut self.ctx);
                self.frame();
                if let Some(l) = &self.live {
                    l.window.set_visible(true);
                }
                self.redraw = true;
            }
            Err(e) => {
                self.error = Some(e);
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        if let Some(l) = &mut self.live {
            l.access.process_event(&l.window, &ev);
        }
        let ppp = self.live.as_ref().map_or(1.0, |l| l.window.scale_factor());
        let mut dirty = true;
        match ev {
            WindowEvent::CloseRequested => {
                el.exit();
                return;
            }
            WindowEvent::RedrawRequested => {
                self.redraw = false;
                self.frame();
                dirty = false;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(ppp);
                self.events.push(Event::PointerMoved(vec2(p.x, p.y)));
            }
            WindowEvent::CursorLeft { .. } => self.events.push(Event::PointerLeft),
            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, MouseButton::Left) => self.events.push(Event::PointerDown),
                (ElementState::Released, MouseButton::Left) => self.events.push(Event::PointerUp),
                (ElementState::Pressed, MouseButton::Right) => self.events.push(Event::SecondaryDown),
                _ => dirty = false,
            },
            WindowEvent::Touch(t) => {
                let p = t.location.to_logical::<f32>(ppp);
                self.events.push(Event::PointerMoved(vec2(p.x, p.y)));
                match t.phase {
                    winit::event::TouchPhase::Started => self.events.push(Event::PointerDown),
                    winit::event::TouchPhase::Moved => {}
                    _ => {
                        self.events.push(Event::PointerUp);
                        self.events.push(Event::PointerLeft);
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(x, y) => vec2(x, y) * 56.0,
                    MouseScrollDelta::PixelDelta(p) => {
                        let p = p.to_logical::<f32>(ppp);
                        vec2(p.x, p.y)
                    }
                };
                self.events.push(Event::Scroll(d));
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.events.push(Event::Modifiers(Modifiers {
                    shift: s.shift_key(),
                    ctrl: s.control_key(),
                    alt: s.alt_key(),
                    command: if cfg!(target_os = "macos") { s.super_key() } else { s.control_key() },
                }));
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let m = self.ctx.input.modifiers;
                if let Some(k) = self.key(&event.logical_key) {
                    if k == Key::Char('v') && m.command {
                        if self.clipboard.is_none() {
                            self.clipboard = arboard::Clipboard::new().ok();
                        }
                        if let Some(t) = self.clipboard.as_mut().and_then(|c| c.get_text().ok()) {
                            self.events.push(Event::Paste(t));
                        }
                    } else {
                        self.events.push(Event::Key(k));
                    }
                }
                if !m.command
                    && let Some(t) = &event.text
                    && !t.chars().all(char::is_control)
                {
                    self.events.push(Event::Text(t.to_string()));
                }
            }
            WindowEvent::Ime(Ime::Commit(t)) => {
                self.events.push(Event::Preedit(String::new(), None));
                self.events.push(Event::Text(t));
            }
            WindowEvent::Ime(Ime::Preedit(t, c)) => self.events.push(Event::Preedit(t, c)),
            WindowEvent::Ime(Ime::Disabled) => self.events.push(Event::Preedit(String::new(), None)),
            WindowEvent::Focused(f) => self.events.push(Event::Focus(f)),
            WindowEvent::DroppedFile(p) => self.app.file_dropped(&mut self.ctx, &p),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {}
            WindowEvent::Occluded(false) | WindowEvent::ThemeChanged(_) => {}
            _ => dirty = false,
        }
        self.redraw |= dirty;
        for c in std::mem::take(&mut self.ctx.host_commands) {
            match c {
                HostCommand::Copy(s) => {
                    if self.clipboard.is_none() {
                        self.clipboard = arboard::Clipboard::new().ok();
                    }
                    if let Some(cb) = &mut self.clipboard {
                        let _ = cb.set_text(s);
                    }
                }
                HostCommand::OpenUrl(u) => open_url(&u),
                HostCommand::Close => el.exit(),
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let Some(live) = &mut self.live else { return };
        if self.redraw {
            live.window.request_redraw();
            el.set_control_flow(ControlFlow::Wait);
            return;
        }
        // Nothing to draw, but measurements may still be on their way.
        if live.renderer.stats_pending() {
            let stats = live.renderer.poll_stats();
            if self.ctx.apply_stats(&stats) {
                live.window.request_redraw();
            }
            el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(8)));
        } else {
            el.set_control_flow(ControlFlow::Wait);
        }
    }
}

/// Center the window on its monitor, smaller if it would not fit.
fn fit_on_screen(window: &Window) {
    let Some(m) = window.current_monitor().or_else(|| window.primary_monitor()) else { return };
    let (area, at) = (m.size(), m.position());
    // Room for a taskbar or dock and the title bar.
    let reserve = (96.0 * m.scale_factor()) as u32;
    let size = window.inner_size();
    let w = size.width.min(area.width.saturating_sub(reserve / 4));
    let h = size.height.min(area.height.saturating_sub(reserve));
    if (w, h) != (size.width, size.height) {
        let _ = window.request_inner_size(PhysicalSize::new(w, h));
    }
    let x = at.x + (area.width.saturating_sub(w) / 2) as i32;
    let y = at.y + (area.height.saturating_sub(h + reserve / 2) / 3) as i32;
    window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
}

fn open_url(u: &str) {
    if !(u.starts_with("https://") || u.starts_with("http://")) {
        return;
    }
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(u).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(u).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(u).spawn();
    if let Err(e) = r {
        log::warn!("could not open {u}: {e}");
    }
}
