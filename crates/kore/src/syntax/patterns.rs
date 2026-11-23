use std::fmt;
use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Pattern;

/// The arguments of an application or connective.
///
/// A `Vec<Pattern>` whose drop does not recurse: dropping a term nested a million levels deep
/// would otherwise overflow the stack. Dereferences to the vector. In KORE JSON, an array.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Patterns(Vec<Pattern>);

impl Patterns {
    pub fn into_vec(mut self) -> Vec<Pattern> {
        std::mem::take(&mut self.0)
    }
}

impl Deref for Patterns {
    type Target = Vec<Pattern>;
    fn deref(&self) -> &Vec<Pattern> {
        &self.0
    }
}

impl DerefMut for Patterns {
    fn deref_mut(&mut self) -> &mut Vec<Pattern> {
        &mut self.0
    }
}

impl From<Vec<Pattern>> for Patterns {
    fn from(v: Vec<Pattern>) -> Patterns {
        Patterns(v)
    }
}

impl fmt::Debug for Patterns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Drop for Patterns {
    fn drop(&mut self) {
        let mut stack = std::mem::take(&mut self.0);
        while let Some(mut p) = stack.pop() {
            if let Some(args) = p.args_mut() {
                stack.append(args);
            }
            // `p` is dropped here with no arguments left.
        }
    }
}

impl Serialize for Patterns {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Patterns {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Patterns, D::Error> {
        Vec::deserialize(d).map(Patterns)
    }
}
