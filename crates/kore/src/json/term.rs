//! The wrapper of the KORE JSON format: `{"format": "KORE", "term": ..., "version": 1}`.
//! The pattern itself is (de)serialized by its own implementation in [`crate::syntax`].

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::syntax::Pattern;
use crate::syntax::fields::{need, set};

/// A pattern with the wrapper of the format, for writing.
pub struct Term<'a>(pub &'a Pattern);

impl Serialize for Term<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Term", 3)?;
        st.serialize_field("format", "KORE")?;
        st.serialize_field("term", self.0)?;
        st.serialize_field("version", &1)?;
        st.end()
    }
}

/// A pattern with the wrapper of the format, for reading. Checks format and version.
pub struct TermOwned(pub Pattern);

impl<'de> Deserialize<'de> for TermOwned {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<TermOwned, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = TermOwned;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(r#"a KORE JSON term: {"format": "KORE", "version": 1, "term": ...}"#)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<TermOwned, A::Error> {
                let (mut format, mut version, mut term) = (None::<String>, None::<u64>, None::<Pattern>);
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "format" => set(&mut format, map.next_value()?, "format")?,
                        "version" => set(&mut version, map.next_value()?, "version")?,
                        "term" => set(&mut term, map.next_value()?, "term")?,
                        other => return Err(de::Error::unknown_field(other, &["format", "version", "term"])),
                    }
                }
                if need::<_, A::Error>(format, "format")? != "KORE" {
                    return Err(de::Error::custom("format must be \"KORE\""));
                }
                if need::<_, A::Error>(version, "version")? != 1 {
                    return Err(de::Error::custom("version must be 1"));
                }
                Ok(TermOwned(need(term, "term")?))
            }
        }
        d.deserialize_map(V)
    }
}
