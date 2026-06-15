#[derive(Clone, Copy, Eq, Hash, Debug, PartialEq)]
pub struct ParameterId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedValue(f64);

impl NormalizedValue {
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn new_unchecked(value: f64) -> Self {
        Self(value)
    }

    pub fn get(&self) -> f64 {
        self.0
    }
}

pub struct ParameterInfo {
    pub id: ParameterId,
    pub name: &'static str,
    pub default_value: NormalizedValue,
    pub min_value: NormalizedValue,
    pub max_value: NormalizedValue,
    pub step_count: Option<u32>,
}

pub trait ParameterGateway {
    fn begin_gesture(&self, id: ParameterId);
    fn end_gesture(&self, id: ParameterId);
    fn set_normalized(&self, id: ParameterId, value: NormalizedValue);
    fn get_normalized(&self, id: ParameterId) -> Option<NormalizedValue>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_value_clamps_below_zero() {
        assert_eq!(NormalizedValue::new(-0.5).get(), 0.0);
    }

    #[test]
    fn normalized_value_clamps_above_one() {
        assert_eq!(NormalizedValue::new(1.5).get(), 1.0);
    }

    #[test]
    fn normalized_value_preserves_in_range() {
        assert_eq!(NormalizedValue::new(0.3).get(), 0.3);
    }

    #[test]
    fn normalized_value_unchecked_does_not_clamp() {
        assert_eq!(NormalizedValue::new_unchecked(2.0).get(), 2.0);
    }
}
