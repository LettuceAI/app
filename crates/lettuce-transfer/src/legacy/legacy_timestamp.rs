use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(untagged)]
enum LegacyTimestamp {
    Signed(i64),
    Unsigned(u64),
}

impl LegacyTimestamp {
    fn signed(self) -> i64 {
        match self {
            Self::Signed(value) => value,
            Self::Unsigned(value) => i64::from_le_bytes(value.to_le_bytes()),
        }
    }
}

pub fn deserialize_legacy_timestamp<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<i64, D::Error> {
    LegacyTimestamp::deserialize(deserializer).map(LegacyTimestamp::signed)
}

pub fn deserialize_optional_legacy_timestamp<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    Option::<LegacyTimestamp>::deserialize(deserializer).map(|value| value.map(LegacyTimestamp::signed))
}

pub fn legacy_timestamp_value(value: &serde_json::Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value.as_u64().map(|value| i64::from_le_bytes(value.to_le_bytes()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Times {
        #[serde(deserialize_with = "deserialize_legacy_timestamp")]
        required: i64,
        #[serde(default, deserialize_with = "deserialize_optional_legacy_timestamp")]
        optional: Option<i64>,
    }

    #[test]
    fn signed_and_legacy_sqlite_unsigned_dates_keep_the_same_bits() {
        for timestamp in [i64::MIN, -315_619_200_000, -1, 0, i64::MAX] {
            for encoded in [json!(timestamp), json!(u64::from_le_bytes(timestamp.to_le_bytes()))] {
                let times: Times = serde_json::from_value(json!({ "required": encoded, "optional": encoded })).expect("legacy times");
                assert_eq!(times, Times { required: timestamp, optional: Some(timestamp) });
            }
        }
    }

    #[test]
    fn optional_dates_can_be_missing_or_null_but_other_types_stay_invalid() {
        for value in [json!({ "required": 0 }), json!({ "required": 0, "optional": null })] {
            assert_eq!(serde_json::from_value::<Times>(value).expect("optional date").optional, None);
        }
        for value in [json!(1.5), json!("20"), json!(true), json!({}), json!([])] {
            assert!(serde_json::from_value::<Times>(json!({ "required": value })).is_err());
            assert!(serde_json::from_value::<Times>(json!({ "required": 0, "optional": value })).is_err());
        }
    }
}
