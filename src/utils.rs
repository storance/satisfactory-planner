pub type FloatType = f64;

pub const EPSILON: FloatType = 0.000001;

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
