#[allow(dead_code)]
pub fn round_f32(value: f32, decimals: u8) -> f32 {
    let multiplier = 10.0f32.powi(decimals as i32);

    (value * multiplier).round() / multiplier
}

pub fn round_f64(value: f64, decimals: u8) -> f64 {
    let multiplier = 10.0f64.powi(decimals as i32);

    (value * multiplier).round() / multiplier
}
