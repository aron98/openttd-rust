//! Pinned Squirrel raw-value equality and non-IEEE three-way ordering.
use crate::{Value, VmError};
use std::cmp::Ordering;
impl Value {
    pub(crate) fn equal(self, other: Self) -> Result<bool, VmError> {
        match (self, other) {
            (Self::Null, Self::Null) => Ok(true),
            (Self::Bool(left), Self::Bool(right)) => Ok(left == right),
            (Self::Integer(left), Self::Integer(right)) => Ok(left == right),
            (Self::Float(left), Self::Float(right)) => Ok(left == right),
            (Self::Integer(_), Self::Float(_)) | (Self::Float(_), Self::Integer(_)) => {
                Ok(self.order(other)? == Ordering::Equal)
            }
            (Self::Null | Self::Bool(_), _) | (_, Self::Null | Self::Bool(_)) => Ok(false),
        }
    }
    #[expect(
        clippy::float_cmp,
        reason = "Native mixed numeric ObjCmp uses exact float equality; same-type floats compare raw bits first"
    )]
    fn order(self, other: Self) -> Result<Ordering, VmError> {
        match (self, other) {
            (Self::Null, Self::Null) => Ok(Ordering::Equal),
            (Self::Integer(left), Self::Integer(right)) => Ok(left.cmp(&right)),
            (Self::Bool(left), Self::Bool(right)) => Ok(left.cmp(&right)),
            (Self::Float(left), Self::Float(right)) => Ok(if left == right {
                Ordering::Equal
            } else if f32::from_bits(left) < f32::from_bits(right) {
                Ordering::Less
            } else {
                Ordering::Greater
            }),
            (Self::Integer(_), Self::Float(_)) | (Self::Float(_), Self::Integer(_)) => {
                let left = self.numeric()?;
                let right = other.numeric()?;
                Ok(if left == right {
                    Ordering::Equal
                } else if left < right {
                    Ordering::Less
                } else {
                    Ordering::Greater
                })
            }
            (Self::Null, _) => Ok(Ordering::Less),
            (_, Self::Null) => Ok(Ordering::Greater),
            (Self::Bool(_), Self::Integer(_) | Self::Float(_))
            | (Self::Integer(_) | Self::Float(_), Self::Bool(_)) => Err(VmError::OperandType),
        }
    }
    pub(crate) fn compare(self, other: Self, selector: u8) -> Result<Self, VmError> {
        let order = self.order(other)?;
        let result = match selector {
            0 => order.is_gt(),
            2 => order.is_ge(),
            3 => order.is_lt(),
            4 => order.is_le(),
            _ => return Err(VmError::InvalidBytecode),
        };
        Ok(Self::Bool(result))
    }
}
