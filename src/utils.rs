pub type FloatType = f64;

pub const EPSILON: FloatType = 0.000001;

const BASE_10: FloatType = 10.0;

pub fn round(value: FloatType, decimals: u8) -> FloatType {
    let multiplier = BASE_10.powi(decimals as i32);

    (value * multiplier).round() / multiplier
}

pub fn clamp_to_zero(value: FloatType) -> FloatType {
    if value.abs() < EPSILON {
        0.0
    } else {
        value
    }
}

pub fn is_zero(value: FloatType) -> bool {
    value.abs() < EPSILON
}
