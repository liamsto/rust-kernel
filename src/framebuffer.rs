use core::{fmt, ptr};

use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use font_constants::INVALID_CHAR;
use noto_sans_mono_bitmap::{get_raster, RasterizedChar};

const LINE_SPACING: usize = 2;
const LETTER_SPACING: usize = 0;
const BORDER_PADDING: usize = 1;

mod font_constants {
    use noto_sans_mono_bitmap::{get_raster_width, FontWeight, RasterHeight};

    pub const CHAR_RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;
    pub const CHAR_RASTER_WIDTH: usize = get_raster_width(
        noto_sans_mono_bitmap::FontWeight::Regular,
        CHAR_RASTER_HEIGHT,
    );
    pub const INVALID_CHAR: char = '�';
    pub const FONT_WEIGHT: FontWeight = FontWeight::Regular;
}

fn get_char_raster(c: char) -> RasterizedChar {
    fn get(c: char) -> Option<RasterizedChar> {
        get_raster(
            c,
            font_constants::FONT_WEIGHT,
            font_constants::CHAR_RASTER_HEIGHT,
        )
    }
    get(c).unwrap_or_else(|| get(INVALID_CHAR).expect("ERROR: Failed to get invalid char."))
}

pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
}

impl FrameBufferWriter {
    /// Create a new logger using a given FrameBufferInfo
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut logger = Self {
            framebuffer,
            info,
            x_pos: 0,
            y_pos: 0,
        };
        logger.clear();
        logger
    }

    /// Prints a newline based on character raster height and line spacing
    fn newline(&mut self) {
        self.y_pos += font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        self.carriage_return();
    }

    /// Increments the x position by `BORDER_PADDING` to simulate a carriage return
    fn carriage_return(&mut self) {
        self.x_pos = BORDER_PADDING;
    }

    /// Clears the framebuffer and resets `x_pos` and `y_pos`
    pub fn clear(&mut self) {
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING;
        self.framebuffer.fill(0);
    }

    /// Returns the width of the framebuffer
    pub fn width(&self) -> usize {
        self.info.width
    }

    /// Returns the height of the framebuffer
    pub fn height(&self) -> usize {
        self.info.height
    }

    /// Writes a character to the framebuffer
    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                let new_xpos = self.x_pos + font_constants::CHAR_RASTER_WIDTH;
                if new_xpos > self.width() {
                    self.newline();
                }
                let new_ypos =
                    self.y_pos + font_constants::CHAR_RASTER_HEIGHT.val() + BORDER_PADDING;
                if new_ypos > self.height() {
                    self.clear();
                }
                self.write_rendered_char(get_char_raster(c));
            }
        }
    }

    fn write_rendered_char(&mut self, rendered_char: RasterizedChar) {
        for (y, row) in rendered_char.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                self.write_pixel(self.x_pos + x, self.y_pos + y, *byte);
            }
        }
        self.x_pos += rendered_char.width() + LETTER_SPACING;
    }

    /// Write a given pixel to the framebuffer. Supports RGB, BGR, and U8 pixel formats.
    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = y * self.info.stride + x;
        let color = match self.info.pixel_format {
            PixelFormat::Rgb => [intensity, intensity, intensity / 2, 0],
            PixelFormat::Bgr => [intensity / 2, intensity, intensity, 0],
            PixelFormat::U8 => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
            other => {
                self.info.pixel_format = PixelFormat::Rgb;
                panic!("Unsupported pixel format: {:?}", other);
            }
        };
        let bpp = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bpp;
        self.framebuffer[byte_offset..(byte_offset + bpp)].copy_from_slice(&color[..bpp]);
        let _ = unsafe { ptr::read_volatile(&self.framebuffer[byte_offset]) };
    }
}

unsafe impl Send for FrameBufferWriter {}
unsafe impl Sync for FrameBufferWriter {}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}
