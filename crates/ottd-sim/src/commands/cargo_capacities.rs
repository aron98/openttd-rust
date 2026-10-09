use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

/// Exact native `CargoArray`: one unsigned capacity for each of 64 cargo slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoCapacities(pub [u32; 64]);
impl Serialize for CargoCapacities {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_slice().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for CargoCapacities {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = Vec::<u32>::deserialize(deserializer)?;
        Ok(Self(values.try_into().map_err(|_| {
            D::Error::custom("native CargoArray requires 64 entries")
        })?))
    }
}
