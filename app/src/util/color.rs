pub use warp_core::ui::color::contrast::*;
pub use warp_core::ui::color::*;

use pathfinder_color::ColorU;

pub fn lighten_toward_white(c: ColorU, amount: f32) -> ColorU {
    let amount = amount.clamp(0.0, 1.0);
    let blend = |channel: u8| -> u8 {
        let v = channel as f32;
        (v + (255.0 - v) * amount).round().clamp(0.0, 255.0) as u8
    };
    ColorU::new(blend(c.r), blend(c.g), blend(c.b), c.a)
}
