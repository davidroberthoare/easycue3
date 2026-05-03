//! Serde helpers for custom serialization
//!
//! Provides functions for serializing data with specific formatting requirements,
//! such as limiting decimal precision for f32 values.

use serde::Serializer;
use std::collections::HashMap;

/// Round f32 to 2 decimal places for serialization
/// 
/// This ensures all f32 values in saved show files have at most 2 decimal places,
/// keeping the JSON files clean and human-readable.
///
/// # Usage
/// ```ignore
/// #[serde(serialize_with = "crate::serde_helpers::round_f32_2")]
/// pub number: f32,
/// ```
pub fn round_f32_2<S>(value: &f32, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let rounded = (*value * 100.0).round() / 100.0;
    serializer.serialize_f32(rounded)
}

/// Round Option<f32> to 2 decimal places for serialization
/// 
/// Similar to `round_f32_2` but handles Option<f32> fields.
///
/// # Usage
/// ```ignore
/// #[serde(serialize_with = "crate::serde_helpers::round_option_f32_2")]
/// pub triggers_audio_cue: Option<f32>,
/// ```
pub fn round_option_f32_2<S>(value: &Option<f32>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(v) => {
            let rounded = (*v * 100.0).round() / 100.0;
            serializer.serialize_some(&rounded)
        }
        None => serializer.serialize_none(),
    }
}

/// Deserialize `HashMap<u16, u8>` from JSON where keys are strings (e.g. `"10": 59`).
/// JSON object keys are always strings; this parses them back to u16.
pub fn deserialize_channel_map<'de, D>(deserializer: D) -> Result<HashMap<u16, u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{MapAccess, Visitor};
    use std::fmt;

    struct ChannelMapVisitor;

    impl<'de> Visitor<'de> for ChannelMapVisitor {
        type Value = HashMap<u16, u8>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("map of channel numbers to values")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
            let mut map = HashMap::new();
            while let Some((k, v)) = access.next_entry::<String, u8>()? {
                let channel: u16 = k.parse().map_err(serde::de::Error::custom)?;
                map.insert(channel, v);
            }
            Ok(map)
        }
    }

    deserializer.deserialize_map(ChannelMapVisitor)
}
