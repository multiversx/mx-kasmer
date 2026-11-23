use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Id;
use super::fields::{need, set};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Sort {
    /// A sort variable, such as `S` in `\top{S}()` inside a parametric axiom.
    Var(Id),
    /// A sort constructor applied to sorts: `SortInt{}`, `SortMap{}`, `SortList{SortInt{}}`.
    App(Id, Vec<Sort>),
}

impl Sort {
    /// A sort without parameters, such as `SortInt{}`.
    pub fn app0(name: &str) -> Sort {
        Sort::App(name.into(), Vec::new())
    }
}

impl Serialize for Sort {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Sort::Var(name) => {
                let mut st = s.serialize_struct("SortVar", 2)?;
                st.serialize_field("name", &**name)?;
                st.serialize_field("tag", "SortVar")?;
                st.end()
            }
            Sort::App(name, args) => {
                let mut st = s.serialize_struct("SortApp", 3)?;
                st.serialize_field("args", args)?;
                st.serialize_field("name", &**name)?;
                st.serialize_field("tag", "SortApp")?;
                st.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Sort {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Sort, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Sort;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a KORE sort")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Sort, A::Error> {
                let (mut tag, mut name, mut args) =
                    (None::<String>, None::<String>, None::<Vec<Sort>>);
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "tag" => set(&mut tag, map.next_value()?, "tag")?,
                        "name" => set(&mut name, map.next_value()?, "name")?,
                        "args" => set(&mut args, map.next_value()?, "args")?,
                        other => {
                            return Err(de::Error::unknown_field(other, &["tag", "name", "args"]));
                        }
                    }
                }
                let name: Id = need(name, "name")?.into();
                match need(tag, "tag")?.as_str() {
                    "SortVar" => Ok(Sort::Var(name)),
                    "SortApp" => Ok(Sort::App(name, need(args, "args")?)),
                    other => Err(de::Error::unknown_variant(other, &["SortVar", "SortApp"])),
                }
            }
        }
        d.deserialize_map(V)
    }
}
