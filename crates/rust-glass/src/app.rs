//! Window host: event loop, surface, clipboard. Draws only when something
//! changes; an idle window uses no CPU or GPU time.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use crate::context::{Context, Cursor, HostCmd};
use crate::geom::vec2;
use crate::gpu::{FrameStats, Gpu, Renderer};
use crate::input::{Event, Key, Modifiers};

/// Timings of recent frames, readable from the UI.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameInfo {
    /// UI code and draw list building, milliseconds.
    pub ui_ms: f32,
    /// Encoding and submission, milliseconds.
    pub encode_ms: f32,
    pub gpu: FrameStats,
    /// Interval between frames while animating, milliseconds.
    pub interval_ms: f32,
}

pub trait App {
    /// Called once, before the first frame.
    fn setup(&mut self, _ctx: &mut Context) {}
    fn update(&mut self, ctx: &mut Context, info: &FrameInfo);
    /// A file was dropped on the window.
    fn file_dropped(&mut self, _ctx: &mut Context, _path: &std::path::Path) {}
}

#[derive(Clone, Debug)]
pub struct Options {
    pub title: String,
    pub size: (f32, f32),
    pub min_size: (f32, f32),
    pub vsync: bool,
    /// Prefer the integrated adapter on machines with two.
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

/// Run one frame of UI code, repeating it when sizes were guessed.
pub fn run_ui(
    ctx: &mut Context,
    app: &mut dyn App,
    info: &FrameInfo,
    size: crate::geom::Vec2,
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
}

struct Host<A: App> {
    app: A,
    ctx: Context,
    options: Options,
    live: Option<Live>,
    events: Vec<Event>,
    start: Instant,
    last: Instant,
    info: FrameInfo,
    clipboard: Option<arboard::Clipboard>,
    redraw: bool,
    was_animating: bool,
    error: Option<String>,
}

/// Open a window and run `app` until it closes.
pub fn run(options: Options, app: impl App + 'static) -> Result<(), String> {
    let el = EventLoop::new().map_err(|e| e.to_string())?;
    let mut host = Host {
        app,
        ctx: Context::new(),
        options,
        live: None,
        events: Vec::new(),
        start: Instant::now(),
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
        window.set_ime_allowed(true);
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
        Ok(Live { window, gpu, surface, config, renderer, cursor: Cursor::Default })
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
        let ui_done = Instant::now();
        let smooth = |old: f32, new: f32| if old == 0.0 { new } else { old * 0.9 + new * 0.1 };
        self.info.ui_ms = smooth(self.info.ui_ms, (ui_done - now).as_secs_f32() * 1000.0);

        for u in self.ctx.uploads.drain(..) {
            let id = live.renderer.add_image(u.w, u.h, &u.rgba);
            debug_assert_eq!(id, u.id);
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
                Cursor::ResizeH => CursorIcon::EwResize,
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

impl<A: App> ApplicationHandler for Host<A> {
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
            WindowEvent::Ime(winit::event::Ime::Commit(t)) => self.events.push(Event::Text(t)),
            WindowEvent::Focused(f) => self.events.push(Event::Focus(f)),
            WindowEvent::DroppedFile(p) => self.app.file_dropped(&mut self.ctx, &p),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {}
            WindowEvent::Occluded(false) | WindowEvent::ThemeChanged(_) => {}
            _ => dirty = false,
        }
        self.redraw |= dirty;
        for c in std::mem::take(&mut self.ctx.host) {
            match c {
                HostCmd::Copy(s) => {
                    if self.clipboard.is_none() {
                        self.clipboard = arboard::Clipboard::new().ok();
                    }
                    if let Some(cb) = &mut self.clipboard {
                        let _ = cb.set_text(s);
                    }
                }
                HostCmd::OpenUrl(u) => open_url(&u),
                HostCmd::Close => el.exit(),
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
