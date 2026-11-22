use serde::de;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::fields::need;
use super::pattern::PatternFields;
use super::{Id, Pattern, Patterns, Sort};

/// Symbol application: `Lbl'Unds'Plus'Int'Unds'{}(X : SortInt{}, \dv{SortInt{}}("1"))`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct App {
    pub symbol: Id,
    pub sorts: Vec<Sort>,
    pub args: Patterns,
}

impl App {
    pub fn new(symbol: impl Into<Id>, sorts: Vec<Sort>, args: Vec<Pattern>) -> App {
        App {
            symbol: symbol.into(),
            sorts,
            args: args.into(),
        }
    }

    /// From the fields of a JSON object tagged `App`.
    pub(super) fn from_fields<E: de::Error>(f: &mut PatternFields) -> Result<App, E> {
        Ok(App {
            symbol: need(f.name.take(), "name")?.into(),
            sorts: need(f.sorts.take(), "sorts")?,
            args: need(f.args.take(), "args")?,
        })
    }
}

/// `{"args": [...], "name": ..., "sorts": [...], "tag": "App"}`
impl Serialize for App {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("App", 4)?;
        st.serialize_field("args", &self.args)?;
        st.serialize_field("name", &*self.symbol)?;
        st.serialize_field("sorts", &self.sorts)?;
        st.serialize_field("tag", "App")?;
        st.end()
    }
}
