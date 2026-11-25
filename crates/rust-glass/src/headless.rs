//! Run an app without a window: scripted input, fixed time steps, pixels back.
//! Used for tests and for comparing renders with reference footage.

use crate::app::{App, FrameInfo, run_ui};
use crate::context::Context;
use crate::geom::{Vec2, vec2};
use crate::gpu::{Gpu, Renderer};
use crate::input::Event;

pub struct Headless {
    pub ctx: Context,
    gpu: Gpu,
    renderer: Renderer,
    target: wgpu::Texture,
    size: (u32, u32),
    ppp: f32,
    time: f64,
    pub info: FrameInfo,
}

impl Headless {
    pub fn new(width: u32, height: u32, ppp: f32) -> Result<Self, String> {
        let gpu = Gpu::new(Gpu::instance(), None, true)?;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let renderer = Renderer::new(&gpu, format);
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
            ctx: Context::new(),
            gpu,
            renderer,
            target,
            size: (width, height),
            ppp,
            time: 0.0,
            info: FrameInfo::default(),
        })
    }

    pub fn describe(&self) -> String {
        self.gpu.describe()
    }

    fn size_points(&self) -> Vec2 {
        vec2(self.size.0 as f32, self.size.1 as f32) / self.ppp
    }

    /// Advance by `dt` seconds and draw.
    pub fn step(&mut self, app: &mut dyn App, dt: f64, events: &[Event]) {
        self.time += dt;
        let size = self.size_points();
        let t0 = std::time::Instant::now();
        run_ui(&mut self.ctx, app, &self.info, size, self.ppp, self.time, events);
        self.info.ui_ms = t0.elapsed().as_secs_f32() * 1000.0;
        for u in self.ctx.uploads.drain(..) {
            self.renderer.add_image(u.w, u.h, &u.rgba);
        }
        self.renderer.upload_atlas(&mut self.ctx.text.uploads);
        let view = self.target.create_view(&Default::default());
        let t1 = std::time::Instant::now();
        self.renderer.render(&self.ctx.paint.data, &view, self.size.0, self.size.1);
        self.info.encode_ms = t1.elapsed().as_secs_f32() * 1000.0;
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let stats = self.renderer.poll_stats();
        self.ctx.apply_stats(&stats);
        self.info.gpu = self.renderer.frame_stats;
    }

    /// Step until animations settle, at most `max` seconds.
    pub fn settle(&mut self, app: &mut dyn App, max: f64) {
        let end = self.time + max;
        let mut quiet = 0;
        while self.time < end && quiet < 6 {
            self.step(app, 1.0 / 60.0, &[]);
            quiet = if self.ctx.wants_repaint() { 0 } else { quiet + 1 };
        }
    }

    /// RGBA8 pixels of the last frame.
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
