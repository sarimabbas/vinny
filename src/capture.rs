use objc2_core_graphics::{CGDisplayCopyDisplayMode, CGDisplayMode};
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub origin_x: i32,
    pub origin_y: i32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub capture_width: u16,
    pub capture_height: u16,
}

pub struct Capture {
    stream: SCStream,
    pub geometry: Geometry,
}

impl Capture {
    pub fn stop(&mut self) {
        let _ = self.stream.stop_capture();
    }
}

struct Handler {
    frames: mpsc::Sender<Vec<u8>>,
    width: usize,
    height: usize,
}

impl SCStreamOutputTrait for Handler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, output_type: SCStreamOutputType) {
        if !matches!(output_type, SCStreamOutputType::Screen) {
            return;
        }
        let Some(buffer) = sample.image_buffer() else {
            return;
        };
        if buffer.width() != self.width || buffer.height() != self.height {
            return;
        }
        let Ok(guard) = buffer.lock(CVPixelBufferLockFlags::READ_ONLY) else {
            return;
        };

        let source = guard.as_slice();
        let stride = guard.bytes_per_row();
        let row_bytes = self.width * 4;
        if stride < row_bytes || source.len() < stride * self.height {
            return;
        }

        let mut rgba = vec![0_u8; row_bytes * self.height];
        for y in 0..self.height {
            let src = &source[y * stride..y * stride + row_bytes];
            let dst = &mut rgba[y * row_bytes..(y + 1) * row_bytes];
            for (bgra, rgba) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
                rgba[0] = bgra[2];
                rgba[1] = bgra[1];
                rgba[2] = bgra[0];
                rgba[3] = 255;
            }
        }

        // ponytail: keep only the freshest frame; remote control values latency over completeness.
        let _ = self.frames.try_send(rgba);
    }
}

fn capture_dimensions(native_width: u32, native_height: u32, max_width: u32) -> (u16, u16) {
    let scale = (max_width as f64 / native_width as f64).min(1.0);
    let width = ((native_width as f64 * scale).round() as u32).clamp(1, u16::MAX as u32);
    let height = ((native_height as f64 * scale).round() as u32).clamp(1, u16::MAX as u32);
    (width as u16, height as u16)
}

fn display_geometry(display: &SCDisplay, max_width: u32) -> Geometry {
    let frame = display.frame();
    let logical_width = frame.size.width.round().max(1.0) as u32;
    let logical_height = frame.size.height.round().max(1.0) as u32;
    let (native_width, native_height) = CGDisplayCopyDisplayMode(display.display_id())
        .map(|mode| {
            (
                CGDisplayMode::pixel_width(Some(&mode)) as u32,
                CGDisplayMode::pixel_height(Some(&mode)) as u32,
            )
        })
        .unwrap_or_else(|| (display.width(), display.height()));
    let (capture_width, capture_height) =
        capture_dimensions(native_width, native_height, max_width);
    Geometry {
        origin_x: frame.origin.x.round() as i32,
        origin_y: frame.origin.y.round() as i32,
        logical_width,
        logical_height,
        capture_width,
        capture_height,
    }
}

pub fn geometry(
    display_index: usize,
    max_width: u32,
) -> Result<Geometry, Box<dyn std::error::Error>> {
    let content = SCShareableContent::get()?;
    let displays = content.displays();
    let display = displays.get(display_index).ok_or_else(|| {
        format!(
            "display {display_index} does not exist (found {})",
            displays.len()
        )
    })?;
    Ok(display_geometry(display, max_width))
}

pub fn start(
    display_index: usize,
    max_width: u32,
    fps: u32,
    frames: mpsc::Sender<Vec<u8>>,
) -> Result<Capture, Box<dyn std::error::Error>> {
    let content = SCShareableContent::get()?;
    let displays = content.displays();
    let display = displays.get(display_index).ok_or_else(|| {
        format!(
            "display {display_index} does not exist (found {})",
            displays.len()
        )
    })?;
    let geometry = display_geometry(display, max_width);
    let filter = SCContentFilter::create()
        .with_display(display)
        .with_excluding_windows(&[])
        .build();
    let config = SCStreamConfiguration::new()
        .with_width(u32::from(geometry.capture_width))
        .with_height(u32::from(geometry.capture_height))
        .with_pixel_format(PixelFormat::BGRA)
        .with_scales_to_fit(true)
        .with_shows_cursor(false)
        .with_queue_depth(2)
        .with_fps(fps);
    let mut stream = SCStream::new(&filter, &config);
    stream.add_output_handler(
        Handler {
            frames,
            width: usize::from(geometry.capture_width),
            height: usize::from(geometry.capture_height),
        },
        SCStreamOutputType::Screen,
    );
    stream.start_capture()?;

    Ok(Capture { stream, geometry })
}

#[cfg(test)]
mod tests {
    use super::capture_dimensions;

    #[test]
    fn max_width_scales_native_retina_pixels() {
        assert_eq!(capture_dimensions(5120, 2880, 3840), (3840, 2160));
    }

    #[test]
    fn max_width_does_not_upscale_past_native_resolution() {
        assert_eq!(capture_dimensions(5120, 2880, 7680), (5120, 2880));
    }
}
