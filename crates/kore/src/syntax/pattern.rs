use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::fields::{need, set};
use super::{App, Assoc, AssocDir, Conn, Id, Ml, Patterns, Sort, Var};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Pattern {
    /// Element variable.
    EVar(Var),
    /// Set variable (name starts with `@`).
    SVar(Var),
    /// String literal. The value is the decoded string; quoting is done by the printers.
    String(String),
    /// Domain value: `\dv{SortInt{}}("42")`.
    Dv {
        sort: Sort,
        value: String,
    },
    App(App),
    Ml(Ml),
    Assoc(Assoc),
}

impl Pattern {
    /// The direct subpatterns.
    pub fn args(&self) -> &[Pattern] {
        match self {
            Pattern::App(a) => &a.args,
            Pattern::Ml(m) => &m.args,
            Pattern::Assoc(a) => &a.app.args,
            Pattern::EVar(_) | Pattern::SVar(_) | Pattern::String(_) | Pattern::Dv { .. } => &[],
        }
    }

    pub(super) fn args_mut(&mut self) -> Option<&mut Vec<Pattern>> {
        match self {
            Pattern::App(a) => Some(&mut a.args),
            Pattern::Ml(m) => Some(&mut m.args),
            Pattern::Assoc(a) => Some(&mut a.app.args),
            Pattern::EVar(_) | Pattern::SVar(_) | Pattern::String(_) | Pattern::Dv { .. } => None,
        }
    }

    pub fn app(symbol: impl Into<Id>, sorts: Vec<Sort>, args: Vec<Pattern>) -> Pattern {
        Pattern::App(App::new(symbol, sorts, args))
    }

    pub fn dv(sort: Sort, value: impl Into<String>) -> Pattern {
        Pattern::Dv {
            sort,
            value: value.into(),
        }
    }

    pub fn evar(name: impl Into<Id>, sort: Sort) -> Pattern {
        Pattern::EVar(Var::new(name, sort))
    }

    /// Calls `f` on every subpattern, parents before children, left to right. Does not recurse.
    pub fn visit(&self, mut f: impl FnMut(&Pattern)) {
        let mut stack = vec![self];
        while let Some(p) = stack.pop() {
            f(p);
            stack.extend(p.args().iter().rev());
        }
    }

    /// The nesting depth: 1 for a leaf. Does not recurse.
    pub fn depth(&self) -> usize {
        let mut max = 0;
        let mut stack = vec![(self, 1)];
        while let Some((p, d)) = stack.pop() {
            max = max.max(d);
            stack.extend(p.args().iter().map(|a| (a, d + 1)));
        }
        max
    }

    /// Rebuilds the pattern bottom-up: `f` gets each subpattern with its arguments already
    /// rebuilt, children before parents. Does not recurse.
    pub fn map_bottom_up(self, mut f: impl FnMut(Pattern) -> Pattern) -> Pattern {
        /// A pattern whose arguments are being rebuilt.
        struct Frame {
            node: Pattern,
            todo: std::vec::IntoIter<Pattern>,
            done: Vec<Pattern>,
        }
        let mut stack: Vec<Frame> = Vec::new();
        let mut current = self;
        loop {
            let args = current.args_mut().map(std::mem::take).unwrap_or_default();
            let mut todo = args.into_iter();
            if let Some(first) = todo.next() {
                let done = Vec::with_capacity(todo.len() + 1);
                stack.push(Frame {
                    node: current,
                    todo,
                    done,
                });
                current = first;
                continue;
            }
            // A leaf, or a pattern whose arguments are all rebuilt: go up until there is an
            // argument left to rebuild.
            let mut result = f(current);
            loop {
                let Some(frame) = stack.last_mut() else {
                    return result;
                };
                frame.done.push(result);
                if let Some(next) = frame.todo.next() {
                    current = next;
                    break;
                }
                let Frame { mut node, done, .. } = stack.pop().expect("not empty");
                *node.args_mut().expect("has arguments") = done;
                result = f(node);
            }
        }
    }

    /// Replaces `\left-assoc` and `\right-assoc` by nested binary applications, as pyk does when
    /// converting to KAST: `\left-assoc{}(f{}(a, b, c))` becomes `f{}(f{}(a, b), c)`, and an
    /// application to a single argument becomes the argument. The LLVM interpreter prints
    /// collections with `\left-assoc`.
    pub fn desugar_assoc(self) -> Pattern {
        self.map_bottom_up(|p| match p {
            Pattern::Assoc(Assoc { dir, app }) => {
                let App { symbol, sorts, args } = app;
                let args = args.into_vec();
                let join = |a: Pattern, b: Pattern| Pattern::app(symbol.clone(), sorts.clone(), vec![a, b]);
                match dir {
                    AssocDir::Left => {
                        let mut it = args.into_iter();
                        match it.next() {
                            Some(first) => it.fold(first, join),
                            None => Pattern::app(symbol.clone(), sorts.clone(), vec![]),
                        }
                    }
                    AssocDir::Right => {
                        let mut it = args.into_iter().rev();
                        match it.next() {
                            Some(last) => it.fold(last, |acc, a| join(a, acc)),
                            None => Pattern::app(symbol.clone(), sorts.clone(), vec![]),
                        }
                    }
                }
            }
            other => other,
        })
    }
}

/// Variables, strings and domain values here; the other variants in their own types.
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
            Pattern::App(app) => app.serialize(s),
            Pattern::Assoc(assoc) => assoc.serialize(s),
            Pattern::Ml(ml) => ml.serialize(s),
        }
    }
}

const TAGS: &[&str] = &[
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

const FIELDS: &[&str] = &[
    "tag", "name", "symbol", "var", "value", "sort", "argSort", "varSort", "arg", "first", "second", "source", "dest",
    "args", "patterns", "argss", "sorts",
];

/// The fields a pattern object may have. Each field name has one type in every kind of pattern,
/// so the fields can be read before the tag is known. The types build themselves from these
/// (`App::from_fields` and so on), taking the fields they use.
#[derive(Default)]
pub(super) struct PatternFields {
    pub tag: Option<String>,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub var: Option<String>,
    pub value: Option<String>,
    pub sort: Option<Sort>,
    pub arg_sort: Option<Sort>,
    pub var_sort: Option<Sort>,
    pub arg: Option<Pattern>,
    pub first: Option<Pattern>,
    pub second: Option<Pattern>,
    pub source: Option<Pattern>,
    pub dest: Option<Pattern>,
    pub args: Option<Patterns>,
    pub patterns: Option<Patterns>,
    pub argss: Option<Patterns>,
    pub sorts: Option<Vec<Sort>>,
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
                other => return Err(de::Error::unknown_field(other, FIELDS)),
            }
        }
        Ok(f)
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
            "App" => Pattern::App(App::from_fields(f)?),
            "LeftAssoc" => Pattern::Assoc(Assoc::from_fields(AssocDir::Left, f)?),
            "RightAssoc" => Pattern::Assoc(Assoc::from_fields(AssocDir::Right, f)?),
            _ => {
                let conn = Conn::from_json_tag(&tag).ok_or_else(|| E::unknown_variant(&tag, TAGS))?;
                Pattern::Ml(Ml::from_fields(conn, f)?)
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
