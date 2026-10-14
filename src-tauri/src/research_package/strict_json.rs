//! Bounded JSON visitor rejects duplicate fields at every nesting level.
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;
struct Seed(usize);
impl<'de> DeserializeSeed<'de> for Seed {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        if self.0 > 32 {
            return Err(de::Error::custom("package_depth_budget"));
        }
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Seed {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON metadata")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("package_non_finite"))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(Seed(self.0 + 1))? {
            if values.len() >= 10000 {
                return Err(de::Error::custom("package_array_budget"));
            }
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Value, A::Error> {
        let mut fields = Map::new();
        while let Some(key) = input.next_key::<String>()? {
            if fields.contains_key(&key) {
                return Err(de::Error::custom("duplicate_package_json_field"));
            }
            if fields.len() >= 10000 {
                return Err(de::Error::custom("package_object_budget"));
            }
            fields.insert(key, input.next_value_seed(Seed(self.0 + 1))?);
        }
        Ok(Value::Object(fields))
    }
}
pub fn decode(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > crate::research_package::MAX_FILE {
        return Err("package_file_budget".into());
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let value = Seed(0)
        .deserialize(&mut d)
        .map_err(|_| "package_json_invalid")?;
    d.end().map_err(|_| "package_json_invalid")?;
    Ok(value)
}
