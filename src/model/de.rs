//! Lenient (de)serializers for the loosely-typed data real Nightscout servers hold.
//!
//! Uploaders disagree on types: numbers arrive as strings (`"sgv": "171"`, profile
//! `"value": "1.0"`), booleans as strings, and fields are frequently `null` or `""`.
//! These `#[serde(with = …)]` modules accept all of those so one odd document cannot fail a
//! whole response, and serialize whole floats as integers so values round-trip unchanged.

use serde::{Deserialize, Deserializer, Serializer};
use serde_json::Value;

fn number_from(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|f| f.is_finite()),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Serializes a float as an integer when it has no fractional part (`120.0` → `120`).
pub(crate) fn serialize_f64<S: Serializer>(v: f64, s: S) -> Result<S::Ok, S::Error> {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        s.serialize_i64(v as i64)
    } else {
        s.serialize_f64(v)
    }
}

/// `Option<f64>` from a number, numeric string, bool, `null` or `""`.
pub(crate) mod num {
    use super::{Deserialize, Deserializer, Serializer, Value, number_from, serialize_f64};

    pub(crate) fn serialize<S: Serializer>(v: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => serialize_f64(*v, s),
            None => s.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
        Ok(Option::<Value>::deserialize(d)?
            .as_ref()
            .and_then(number_from))
    }
}

/// `f64` with the leniency of [`num`]; missing or unparsable values become `0.0`.
pub(crate) mod num_or_zero {
    use super::{Deserializer, Serializer, serialize_f64};

    pub(crate) fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        serialize_f64(*v, s)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(super::num::deserialize(d)?.unwrap_or_default())
    }
}

/// `Option<i64>` from integers, integral floats and numeric strings.
pub(crate) mod int {
    use super::{Deserialize, Deserializer, Serializer, Value, number_from};

    pub(crate) fn serialize<S: Serializer>(v: &Option<i64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => s.serialize_i64(*v),
            None => s.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
        Ok(Option::<Value>::deserialize(d)?.and_then(|v| match &v {
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
            other => number_from(other).map(|f| f.round() as i64),
        }))
    }
}

/// `Option<i32>`, range-checked, with the leniency of [`int`].
pub(crate) mod int32 {
    use super::{Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(v: &Option<i32>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => s.serialize_i32(*v),
            None => s.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
        Ok(super::int::deserialize(d)?.and_then(|v| i32::try_from(v).ok()))
    }
}

/// `Option<bool>` from bools, `"true"`/`"false"`, and `0`/`1`.
pub(crate) mod flag {
    use super::{Deserialize, Deserializer, Serializer, Value};

    pub(crate) fn serialize<S: Serializer>(v: &Option<bool>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => s.serialize_bool(*v),
            None => s.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
        Ok(Option::<Value>::deserialize(d)?.and_then(|v| match v {
            Value::Bool(b) => Some(b),
            Value::Number(n) => n.as_f64().map(|f| f != 0.0),
            Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        }))
    }
}

/// `Option<String>` from strings and scalars (numbers are stringified); `null` → `None`.
pub(crate) mod text {
    use super::{Deserialize, Deserializer, Serializer, Value};

    pub(crate) fn serialize<S: Serializer>(v: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => s.serialize_str(v),
            None => s.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        Ok(Option::<Value>::deserialize(d)?.and_then(|v| match v {
            Value::String(s) => Some(s),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }))
    }
}

/// `Vec<f64>` of lenient numbers (`null` → empty; unparsable items dropped), serialized with
/// whole values as integers.
pub(crate) mod nums {
    use super::{Deserialize, Deserializer, Serializer, Value, number_from, serialize_f64};
    use serde::ser::SerializeSeq;

    pub(crate) fn serialize<S: Serializer>(v: &[f64], s: S) -> Result<S::Ok, S::Error> {
        struct Num(f64);
        impl serde::Serialize for Num {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                serialize_f64(self.0, s)
            }
        }
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for x in v {
            seq.serialize_element(&Num(*x))?;
        }
        seq.end()
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
        Ok(Option::<Vec<Value>>::deserialize(d)?
            .unwrap_or_default()
            .iter()
            .filter_map(number_from)
            .collect())
    }
}

/// `Vec<T>` that treats `null` (and a missing field, with `#[serde(default)]`) as empty.
pub(crate) fn null_as_empty<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

/// `Option<T>` that turns values of the wrong shape into `None` instead of an error.
pub(crate) fn tolerant<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    Ok(Option::<Value>::deserialize(d)?.and_then(|v| serde_json::from_value(v).ok()))
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    struct Probe {
        #[serde(default, skip_serializing_if = "Option::is_none", with = "super::num")]
        f: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "super::int")]
        i: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "super::flag")]
        b: Option<bool>,
    }

    fn probe(json: &str) -> Probe {
        serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
    }

    #[test]
    fn accepts_what_uploaders_send() {
        assert_eq!(probe(r#"{"f":"1.5"}"#).f, Some(1.5));
        assert_eq!(probe(r#"{"f":""}"#).f, None);
        assert_eq!(probe(r#"{"f":null}"#).f, None);
        assert_eq!(probe(r"{}").f, None);
        assert_eq!(probe(r#"{"i":"171"}"#).i, Some(171));
        assert_eq!(probe(r#"{"i":170.6}"#).i, Some(171));
        assert_eq!(probe(r#"{"b":"false"}"#).b, Some(false));
        assert_eq!(probe(r#"{"b":1}"#).b, Some(true));
        assert_eq!(probe(r#"{"f":"NaN"}"#).f, None);
    }

    #[test]
    fn whole_floats_serialize_as_integers() {
        let json = serde_json::to_string(&probe(r#"{"f":131000}"#)).unwrap_or_default();
        assert_eq!(json, r#"{"f":131000}"#);
        let json = serde_json::to_string(&probe(r#"{"f":2.5}"#)).unwrap_or_default();
        assert_eq!(json, r#"{"f":2.5}"#);
    }
}
