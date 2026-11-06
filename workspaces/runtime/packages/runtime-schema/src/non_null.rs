pub mod option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
        d: D,
    ) -> Result<Option<T>, D::Error> {
        T::deserialize(d).map(Some)
    }

    pub fn serialize<S: Serializer, T: Serialize>(
        value: &Option<T>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        value.serialize(s)
    }
}
