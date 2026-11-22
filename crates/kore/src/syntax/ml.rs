use serde::de;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::fields::need;
use super::pattern::PatternFields;
use super::{Arity, Binder, Conn, Pattern, Patterns, Sort, Var};

/// A matching-logic connective: `\and`, `\equals`, `\exists`, ... The number of sorts, the bound
/// variable and the number of arguments depend on the connective, see [`Conn::shape`].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Ml {
    pub conn: Conn,
    pub sorts: Vec<Sort>,
    pub var: Option<Var>,
    pub args: Patterns,
}

impl Ml {
    /// Checks the counts of sorts and arguments and the bound variable against the connective.
    pub fn new(conn: Conn, sorts: Vec<Sort>, var: Option<Var>, args: Vec<Pattern>) -> Result<Ml, String> {
        let shape = conn.shape();
        let name = conn.symbol();
        if sorts.len() != shape.sorts {
            return Err(format!("{name} takes {} sorts, got {}", shape.sorts, sorts.len()));
        }
        match (shape.binder, &var) {
            (Binder::None, None) => {}
            (Binder::Element, Some(v)) if !v.is_set_var() => {}
            (Binder::Set, Some(v)) if v.is_set_var() => {}
            (Binder::None, Some(_)) => return Err(format!("{name} binds no variable")),
            (Binder::Element, _) => return Err(format!("{name} binds an element variable")),
            (Binder::Set, _) => return Err(format!("{name} binds a set variable")),
        }
        if let Arity::Exactly(n) = shape.args
            && args.len() != n
        {
            return Err(format!("{name} takes {n} arguments, got {}", args.len()));
        }
        Ok(Ml {
            conn,
            sorts,
            var,
            args: args.into(),
        })
    }

    /// From the fields of a JSON object with the tag of `conn`.
    pub(super) fn from_fields<E: de::Error>(conn: Conn, f: &mut PatternFields) -> Result<Ml, E> {
        let sort = |f: &mut PatternFields| need(f.sort.take(), "sort");
        let arg_sort = |f: &mut PatternFields| need(f.arg_sort.take(), "argSort");
        let var = |f: &mut PatternFields| -> Result<Var, E> {
            Ok(Var::new(
                need(f.var.take(), "var")?,
                need(f.var_sort.take(), "varSort")?,
            ))
        };
        let one = |f: &mut PatternFields, name| -> Result<Vec<Pattern>, E> {
            let slot = match name {
                "arg" => &mut f.arg,
                "dest" => &mut f.dest,
                _ => unreachable!(),
            };
            Ok(vec![need(slot.take(), name)?])
        };
        let two = |f: &mut PatternFields, first, second| -> Result<Vec<Pattern>, E> {
            let (a, b) = match (first, second) {
                ("first", "second") => (f.first.take(), f.second.take()),
                ("source", "dest") => (f.source.take(), f.dest.take()),
                _ => unreachable!(),
            };
            Ok(vec![need(a, first)?, need(b, second)?])
        };
        let (sorts, var, args) = match conn {
            Conn::Top | Conn::Bottom => (vec![sort(f)?], None, vec![]),
            Conn::Not => (vec![sort(f)?], None, one(f, "arg")?),
            Conn::Implies | Conn::Iff => (vec![sort(f)?], None, two(f, "first", "second")?),
            Conn::And | Conn::Or => (vec![sort(f)?], None, need(f.patterns.take(), "patterns")?.into_vec()),
            Conn::Exists | Conn::Forall => (vec![sort(f)?], Some(var(f)?), one(f, "arg")?),
            Conn::Mu | Conn::Nu => (vec![], Some(var(f)?), one(f, "arg")?),
            Conn::Ceil | Conn::Floor => (vec![arg_sort(f)?, sort(f)?], None, one(f, "arg")?),
            Conn::Equals | Conn::In => (vec![arg_sort(f)?, sort(f)?], None, two(f, "first", "second")?),
            Conn::Next => (vec![sort(f)?], None, one(f, "dest")?),
            Conn::Rewrites => (vec![sort(f)?], None, two(f, "source", "dest")?),
        };
        Ml::new(conn, sorts, var, args).map_err(E::custom)
    }
}

/// The fields, in alphabetical order, by connective:
///
/// | Connective | Fields |
/// |---|---|
/// | `Top`, `Bottom` | `sort` |
/// | `Not` | `arg`, `sort` |
/// | `Implies`, `Iff` | `first`, `second`, `sort` |
/// | `And`, `Or` | `patterns`, `sort` |
/// | `Exists`, `Forall` | `arg`, `sort`, `var`, `varSort` |
/// | `Mu`, `Nu` | `arg`, `var`, `varSort` |
/// | `Ceil`, `Floor` | `arg`, `argSort`, `sort` |
/// | `Equals`, `In` | `argSort`, `first`, `second`, `sort` |
/// | `Next` | `dest`, `sort` |
/// | `Rewrites` | `dest`, `sort`, `source` |
///
/// plus `tag`, in its place in the order.
impl Serialize for Ml {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let tag = self.conn.json_tag();
        let a = &self.args;
        let mut st = s.serialize_struct("Ml", 5)?;
        let var = |st: &mut S::SerializeStruct| -> Result<(), S::Error> {
            let v = self.var.as_ref().expect("binder has a variable");
            st.serialize_field("var", &*v.name)?;
            st.serialize_field("varSort", &v.sort)
        };
        match self.conn {
            Conn::Top | Conn::Bottom => {
                st.serialize_field("sort", &self.sorts[0])?;
                st.serialize_field("tag", tag)?;
            }
            Conn::Not => {
                st.serialize_field("arg", &a[0])?;
                st.serialize_field("sort", &self.sorts[0])?;
                st.serialize_field("tag", tag)?;
            }
            Conn::Implies | Conn::Iff => {
                st.serialize_field("first", &a[0])?;
                st.serialize_field("second", &a[1])?;
                st.serialize_field("sort", &self.sorts[0])?;
                st.serialize_field("tag", tag)?;
            }
            Conn::And | Conn::Or => {
                st.serialize_field("patterns", a)?;
                st.serialize_field("sort", &self.sorts[0])?;
                st.serialize_field("tag", tag)?;
            }
            Conn::Exists | Conn::Forall => {
                st.serialize_field("arg", &a[0])?;
                st.serialize_field("sort", &self.sorts[0])?;
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
                st.serialize_field("argSort", &self.sorts[0])?;
                st.serialize_field("sort", &self.sorts[1])?;
                st.serialize_field("tag", tag)?;
            }
            Conn::Equals | Conn::In => {
                st.serialize_field("argSort", &self.sorts[0])?;
                st.serialize_field("first", &a[0])?;
                st.serialize_field("second", &a[1])?;
                st.serialize_field("sort", &self.sorts[1])?;
                st.serialize_field("tag", tag)?;
            }
            Conn::Next => {
                st.serialize_field("dest", &a[0])?;
                st.serialize_field("sort", &self.sorts[0])?;
                st.serialize_field("tag", tag)?;
            }
            Conn::Rewrites => {
                st.serialize_field("dest", &a[1])?;
                st.serialize_field("sort", &self.sorts[0])?;
                st.serialize_field("source", &a[0])?;
                st.serialize_field("tag", tag)?;
            }
        }
        st.end()
    }
}
