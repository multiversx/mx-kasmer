//! `Serialize` and `Deserialize` for sorts and patterns, in the KORE JSON format.
//!
//! Fields are written in alphabetical order, as pyk writes them (`sort_keys=True`). When reading,
//! the type of each field follows from its name alone, so objects are read in one pass whatever
//! the order of their fields (pyk puts `tag` last).

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::syntax::*;

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

impl Serialize for Patterns {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (**self).serialize(s)
    }
}

impl Serialize for Pattern {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Pattern::EVar(v) | Pattern::SVar(v) => {
                let mut st = s.serialize_struct("Var", 3)?;
                st.serialize_field("name", &*v.name)?;
                st.serialize_field("sort", &v.sort)?;
                st.serialize_field(
                    "tag",
                    if matches!(self, Pattern::EVar(_)) {
                        "EVar"
                    } else {
                        "SVar"
                    },
                )?;
                st.end()
            }
            Pattern::String(value) => {
                let mut st = s.serialize_struct("String", 2)?;
                st.serialize_field("tag", "String")?;
                st.serialize_field("value", value)?;
                st.end()
            }
            Pattern::Dv { sort, value } => {
                let mut st = s.serialize_struct("DV", 3)?;
                st.serialize_field("sort", sort)?;
                st.serialize_field("tag", "DV")?;
                st.serialize_field("value", value)?;
                st.end()
            }
            Pattern::App(app) => {
                let mut st = s.serialize_struct("App", 4)?;
                st.serialize_field("args", &app.args)?;
                st.serialize_field("name", &*app.symbol)?;
                st.serialize_field("sorts", &app.sorts)?;
                st.serialize_field("tag", "App")?;
                st.end()
            }
            Pattern::Assoc(Assoc { dir, app }) => {
                let mut st = s.serialize_struct("Assoc", 4)?;
                st.serialize_field("argss", &app.args)?;
                st.serialize_field("sorts", &app.sorts)?;
                st.serialize_field("symbol", &*app.symbol)?;
                st.serialize_field(
                    "tag",
                    if *dir == AssocDir::Left {
                        "LeftAssoc"
                    } else {
                        "RightAssoc"
                    },
                )?;
                st.end()
            }
            Pattern::Ml(ml) => serialize_ml(ml, s),
        }
    }
}

fn serialize_ml<S: Serializer>(ml: &Ml, s: S) -> Result<S::Ok, S::Error> {
    let tag = ml.conn.json_tag();
    let a = &ml.args;
    let mut st = s.serialize_struct("Ml", 5)?;
    let var = |st: &mut S::SerializeStruct| -> Result<(), S::Error> {
        let v = ml.var.as_ref().expect("binder has a variable");
        st.serialize_field("var", &*v.name)?;
        st.serialize_field("varSort", &v.sort)
    };
    match ml.conn {
        Conn::Top | Conn::Bottom => {
            st.serialize_field("sort", &ml.sorts[0])?;
            st.serialize_field("tag", tag)?;
        }
        Conn::Not => {
            st.serialize_field("arg", &a[0])?;
            st.serialize_field("sort", &ml.sorts[0])?;
            st.serialize_field("tag", tag)?;
        }
        Conn::Implies | Conn::Iff => {
            st.serialize_field("first", &a[0])?;
            st.serialize_field("second", &a[1])?;
            st.serialize_field("sort", &ml.sorts[0])?;
            st.serialize_field("tag", tag)?;
        }
        Conn::And | Conn::Or => {
            st.serialize_field("patterns", a)?;
            st.serialize_field("sort", &ml.sorts[0])?;
            st.serialize_field("tag", tag)?;
        }
        Conn::Exists | Conn::Forall => {
            st.serialize_field("arg", &a[0])?;
            st.serialize_field("sort", &ml.sorts[0])?;
            st.serialize_field("tag", tag)?;
            var(&mut st)?;
        }
        Conn::Mu | Conn::Nu => {
            st.serialize_field("arg", &a[0])?;
            st.serialize_field("tag", tag)?;
            var(&mut st)?;
        }
        Conn::Ceil | Conn::Floor => {
            st.serialize_field("arg", &a[0])?;
            st.serialize_field("argSort", &ml.sorts[0])?;
            st.serialize_field("sort", &ml.sorts[1])?;
            st.serialize_field("tag", tag)?;
        }
        Conn::Equals | Conn::In => {
            st.serialize_field("argSort", &ml.sorts[0])?;
            st.serialize_field("first", &a[0])?;
            st.serialize_field("second", &a[1])?;
            st.serialize_field("sort", &ml.sorts[1])?;
            st.serialize_field("tag", tag)?;
        }
        Conn::Next => {
            st.serialize_field("dest", &a[0])?;
            st.serialize_field("sort", &ml.sorts[0])?;
            st.serialize_field("tag", tag)?;
        }
        Conn::Rewrites => {
            st.serialize_field("dest", &a[1])?;
            st.serialize_field("sort", &ml.sorts[0])?;
            st.serialize_field("source", &a[0])?;
            st.serialize_field("tag", tag)?;
        }
    }
    st.end()
}

/// A pattern with the wrapper of the format: `{"format": "KORE", "term": ..., "version": 1}`.
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

/// Stores a field, rejecting duplicates.
fn set<T, E: de::Error>(slot: &mut Option<T>, value: T, name: &'static str) -> Result<(), E> {
    if slot.replace(value).is_some() {
        return Err(E::duplicate_field(name));
    }
    Ok(())
}

fn need<T, E: de::Error>(slot: Option<T>, name: &'static str) -> Result<T, E> {
    slot.ok_or_else(|| E::missing_field(name))
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
                let (mut tag, mut name, mut args) = (None::<String>, None::<String>, None::<Vec<Sort>>);
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "tag" => set(&mut tag, map.next_value()?, "tag")?,
                        "name" => set(&mut name, map.next_value()?, "name")?,
                        "args" => set(&mut args, map.next_value()?, "args")?,
                        other => return Err(de::Error::unknown_field(other, &["tag", "name", "args"])),
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

const PATTERN_TAGS: &[&str] = &[
    "EVar",
    "SVar",
    "String",
    "DV",
    "App",
    "LeftAssoc",
    "RightAssoc",
    "Top",
    "Bottom",
    "Not",
    "And",
    "Or",
    "Implies",
    "Iff",
    "Exists",
    "Forall",
    "Mu",
    "Nu",
    "Ceil",
    "Floor",
    "Equals",
    "In",
    "Next",
    "Rewrites",
];

const PATTERN_FIELDS: &[&str] = &[
    "tag", "name", "symbol", "var", "value", "sort", "argSort", "varSort", "arg", "first", "second", "source", "dest",
    "args", "patterns", "argss", "sorts",
];

/// The fields a pattern object may have, by type.
#[derive(Default)]
struct PatternFields {
    tag: Option<String>,
    name: Option<String>,
    symbol: Option<String>,
    var: Option<String>,
    value: Option<String>,
    sort: Option<Sort>,
    arg_sort: Option<Sort>,
    var_sort: Option<Sort>,
    arg: Option<Pattern>,
    first: Option<Pattern>,
    second: Option<Pattern>,
    source: Option<Pattern>,
    dest: Option<Pattern>,
    args: Option<Vec<Pattern>>,
    patterns: Option<Vec<Pattern>>,
    argss: Option<Vec<Pattern>>,
    sorts: Option<Vec<Sort>>,
}

impl PatternFields {
    fn read<'de, A: MapAccess<'de>>(map: &mut A) -> Result<PatternFields, A::Error> {
        let mut f = PatternFields::default();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "tag" => set(&mut f.tag, map.next_value()?, "tag")?,
                "name" => set(&mut f.name, map.next_value()?, "name")?,
                "symbol" => set(&mut f.symbol, map.next_value()?, "symbol")?,
                "var" => set(&mut f.var, map.next_value()?, "var")?,
                "value" => set(&mut f.value, map.next_value()?, "value")?,
                "sort" => set(&mut f.sort, map.next_value()?, "sort")?,
                "argSort" => set(&mut f.arg_sort, map.next_value()?, "argSort")?,
                "varSort" => set(&mut f.var_sort, map.next_value()?, "varSort")?,
                "arg" => set(&mut f.arg, map.next_value()?, "arg")?,
                "first" => set(&mut f.first, map.next_value()?, "first")?,
                "second" => set(&mut f.second, map.next_value()?, "second")?,
                "source" => set(&mut f.source, map.next_value()?, "source")?,
                "dest" => set(&mut f.dest, map.next_value()?, "dest")?,
                "args" => set(&mut f.args, map.next_value()?, "args")?,
                "patterns" => set(&mut f.patterns, map.next_value()?, "patterns")?,
                "argss" => set(&mut f.argss, map.next_value()?, "argss")?,
                "sorts" => set(&mut f.sorts, map.next_value()?, "sorts")?,
                other => return Err(de::Error::unknown_field(other, PATTERN_FIELDS)),
            }
        }
        Ok(f)
    }

    fn var<E: de::Error>(&mut self) -> Result<Var, E> {
        Ok(Var::new(
            need(self.var.take(), "var")?,
            need(self.var_sort.take(), "varSort")?,
        ))
    }

    fn into_pattern<E: de::Error>(mut self) -> Result<Pattern, E> {
        let tag = need(self.tag.take(), "tag")?;
        let f = &mut self;
        Ok(match tag.as_str() {
            "EVar" | "SVar" => {
                let v = Var::new(need(f.name.take(), "name")?, need(f.sort.take(), "sort")?);
                if (tag == "SVar") != v.is_set_var() {
                    return Err(E::custom(format!(
                        "{tag}: set variable names start with '@', others do not"
                    )));
                }
                if tag == "EVar" {
                    Pattern::EVar(v)
                } else {
                    Pattern::SVar(v)
                }
            }
            "String" => Pattern::String(need(f.value.take(), "value")?),
            "DV" => Pattern::Dv {
                sort: need(f.sort.take(), "sort")?,
                value: need(f.value.take(), "value")?,
            },
            "App" => Pattern::App(App {
                symbol: need(f.name.take(), "name")?.into(),
                sorts: need(f.sorts.take(), "sorts")?,
                args: need(f.args.take(), "args")?.into(),
            }),
            "LeftAssoc" | "RightAssoc" => Pattern::Assoc(Assoc {
                dir: if tag == "LeftAssoc" {
                    AssocDir::Left
                } else {
                    AssocDir::Right
                },
                app: App {
                    symbol: need(f.symbol.take(), "symbol")?.into(),
                    sorts: need(f.sorts.take(), "sorts")?,
                    args: need(f.argss.take(), "argss")?.into(),
                },
            }),
            _ => {
                let conn = Conn::from_json_tag(&tag).ok_or_else(|| E::unknown_variant(&tag, PATTERN_TAGS))?;
                let sort = |f: &mut PatternFields| need(f.sort.take(), "sort");
                let (sorts, var, args) = match conn {
                    Conn::Top | Conn::Bottom => (vec![sort(f)?], None, vec![]),
                    Conn::Not => (vec![sort(f)?], None, vec![need(f.arg.take(), "arg")?]),
                    Conn::Implies | Conn::Iff => (
                        vec![sort(f)?],
                        None,
                        vec![need(f.first.take(), "first")?, need(f.second.take(), "second")?],
                    ),
                    Conn::And | Conn::Or => (vec![sort(f)?], None, need(f.patterns.take(), "patterns")?),
                    Conn::Exists | Conn::Forall => (vec![sort(f)?], Some(f.var()?), vec![need(f.arg.take(), "arg")?]),
                    Conn::Mu | Conn::Nu => (vec![], Some(f.var()?), vec![need(f.arg.take(), "arg")?]),
                    Conn::Ceil | Conn::Floor => (
                        vec![need(f.arg_sort.take(), "argSort")?, sort(f)?],
                        None,
                        vec![need(f.arg.take(), "arg")?],
                    ),
                    Conn::Equals | Conn::In => (
                        vec![need(f.arg_sort.take(), "argSort")?, sort(f)?],
                        None,
                        vec![need(f.first.take(), "first")?, need(f.second.take(), "second")?],
                    ),
                    Conn::Next => (vec![sort(f)?], None, vec![need(f.dest.take(), "dest")?]),
                    Conn::Rewrites => (
                        vec![sort(f)?],
                        None,
                        vec![need(f.source.take(), "source")?, need(f.dest.take(), "dest")?],
                    ),
                };
                Pattern::Ml(Ml::new(conn, sorts, var, args).map_err(E::custom)?)
            }
        })
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Pattern, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Pattern;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a KORE pattern")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pattern, A::Error> {
                PatternFields::read(&mut map)?.into_pattern()
            }
        }
        d.deserialize_map(V)
    }
}

/// Reads the wrapper of the format and returns the pattern inside.
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
