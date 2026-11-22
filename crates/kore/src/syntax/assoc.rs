use serde::de;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::App;
use super::fields::need;
use super::pattern::PatternFields;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AssocDir {
    Left,
    Right,
}

impl AssocDir {
    /// The `tag` of the KORE JSON format.
    pub fn json_tag(self) -> &'static str {
        match self {
            AssocDir::Left => "LeftAssoc",
            AssocDir::Right => "RightAssoc",
        }
    }
}

/// `\left-assoc{}(f{}(a, b, c))` or `\right-assoc{}(...)`: `f` applied to its arguments,
/// associating to the left or to the right.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Assoc {
    pub dir: AssocDir,
    pub app: App,
}

impl Assoc {
    /// From the fields of a JSON object tagged `LeftAssoc` or `RightAssoc`.
    pub(super) fn from_fields<E: de::Error>(dir: AssocDir, f: &mut PatternFields) -> Result<Assoc, E> {
        let app = App {
            symbol: need(f.symbol.take(), "symbol")?.into(),
            sorts: need(f.sorts.take(), "sorts")?,
            args: need(f.argss.take(), "argss")?,
        };
        Ok(Assoc { dir, app })
    }
}

/// `{"argss": [...], "sorts": [...], "symbol": ..., "tag": "LeftAssoc"}`
impl Serialize for Assoc {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Assoc", 4)?;
        st.serialize_field("argss", &self.app.args)?;
        st.serialize_field("sorts", &self.app.sorts)?;
        st.serialize_field("symbol", &*self.app.symbol)?;
        st.serialize_field("tag", self.dir.json_tag())?;
        st.end()
    }
}
