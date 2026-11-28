//! Runs an app without a window: scripted input, fixed time steps, pixels
//! back. Use it for tests and to render screenshots.

use crate::app::{App, FrameInfo, FrameTimes, Peaks, run_ui};
use crate::context::Context;
use crate::geometry::{Vec2, vec2};
use crate::gpu::{Gpu, Renderer};
use crate::input::Event;

/// An app host without a window, for tests and screenshots.
///
/// It still needs a GPU. Time only moves when you call [`Headless::step`].
pub struct Headless {
    /// The window state, for reading what widgets did.
    pub ctx: Context,
    gpu: Gpu,
    renderer: Renderer,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
    ppp: f32,
    time: f64,
    /// Timings of the last frame.
    pub info: FrameInfo,
    peaks: Peaks,
}

impl Headless {
    /// Creates a host that renders `width` by `height` pixels at
    /// `pixels_per_point`.
    ///
    /// # Errors
    ///
    /// Returns an error when no graphics adapter is available.
    pub fn new(width: u32, height: u32, pixels_per_point: f32) -> Result<Self, String> {
        let ppp = pixels_per_point;
        let gpu = Gpu::new(Gpu::instance(), None, true)?;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let renderer = Renderer::new(&gpu, format);
        let mut ctx = Context::new();
        ctx.quality = crate::Quality::Full;
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Ok(Self {
            ctx,
            gpu,
            renderer,
            view: target.create_view(&Default::default()),
            target,
            size: (width, height),
            ppp,
            time: 0.0,
            info: FrameInfo::default(),
            peaks: Peaks::default(),
        })
    }

    /// Returns the name and backend of the GPU in use.
    pub fn describe(&self) -> String {
        self.gpu.describe()
    }

    fn size_points(&self) -> Vec2 {
        vec2(self.size.0 as f32, self.size.1 as f32) / self.ppp
    }

    /// Advances time by `dt` seconds, runs one frame of `app` with `events` and
    /// renders it.
    pub fn step(&mut self, app: &mut dyn App, dt: f64, events: &[Event]) {
        self.time += dt;
        let size = self.size_points();
        let t0 = std::time::Instant::now();
        run_ui(&mut self.ctx, app, &self.info, size, self.ppp, self.time, events);
        self.info.ui_ms = t0.elapsed().as_secs_f32() * 1000.0;
        for u in self.ctx.uploads.drain(..) {
            self.renderer.add_image(u.w, u.h, &u.rgba);
        }
        self.renderer.upload_atlas(&mut self.ctx.text);
        for id in self.ctx.removed_images.drain(..) {
            self.renderer.remove_image(id);
        }
        let view = &self.view;
        let t1 = std::time::Instant::now();
        self.renderer.render(&self.ctx.paint.data, view, self.size.0, self.size.1);
        self.info.encode_ms = t1.elapsed().as_secs_f32() * 1000.0;
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let stats = self.renderer.poll_stats();
        self.ctx.apply_stats(stats);
        self.info.gpu = self.renderer.frame_stats;
        let last = FrameTimes {
            ui_ms: self.info.ui_ms,
            encode_ms: self.info.encode_ms,
            gpu_ms: self.info.gpu.gpu_ms_last,
            interval_ms: (dt * 1000.0) as f32,
        };
        self.info.last = last;
        self.info.peak = self.peaks.add(self.time, last);
    }

    /// Runs frames of 1/60 second until nothing animates any more, for at most
    /// `max` seconds. Frames asked for later with
    /// [`Context::request_repaint_after`](crate::Context::request_repaint_after)
    /// count too, as long as they fall before the end.
    pub fn settle(&mut self, app: &mut dyn App, max: f64) {
        let end = self.time + max;
        let mut quiet = 0;
        while self.time < end && quiet < 6 {
            self.step(app, 1.0 / 60.0, &[]);
            let waiting = self.ctx.repaint_deadline().is_some_and(|at| at <= end);
            quiet = if self.ctx.wants_repaint() || waiting { 0 } else { quiet + 1 };
        }
    }

    /// Draws every frame in full when `on`, instead of only the part that
    /// changed. Use it to time how long a whole frame takes.
    pub fn set_full_redraw(&mut self, on: bool) {
        self.renderer.set_full_redraw(on);
    }

    /// Draws the inside of large blurred glass at half resolution when `on`,
    /// the default. Turn it off to compare with a frame drawn at full
    /// resolution everywhere.
    pub fn set_half_resolution(&mut self, on: bool) {
        self.renderer.set_half_resolution(on);
    }

    /// Returns `true` if the last frame was drawn only in part, because the
    /// rest did not change.
    pub fn last_frame_partial(&self) -> bool {
        let (w, h) = (self.size.0.next_multiple_of(32) as u64, self.size.1.next_multiple_of(32) as u64);
        self.info.gpu.redrawn_px < w * h
    }

    /// Returns the pixels of the last frame, row by row, in RGBA order with 8
    /// bits per channel.
    pub fn pixels(&self) -> Vec<u8> {
        let (w, h) = self.size;
        let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixels"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        if let Ok(view) = buf.slice(..).get_mapped_range() {
            for y in 0..h {
                let a = (y * row) as usize;
                out.extend_from_slice(&view[a..a + (w * 4) as usize]);
            }
        }
        out
    }
}
