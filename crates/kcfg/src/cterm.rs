//! Constrained terms: a configuration and the conditions on its variables, in KORE.

use kore::{Conn, Ml, Pattern, Sort};
use kore_rpc::protocol::State;
use serde::{Deserialize, Serialize};

/// The sort of configurations, and of the predicates conjoined with them.
pub fn top_sort() -> Sort {
    Sort::app0("SortGeneratedTopCell")
}

/// A configuration and its path condition: predicates (`\equals`, `\not`, ...) of sort
/// `SortGeneratedTopCell`, a conjunction.
///
/// The constraints are kept flat (no `\and`), without `\top` and without duplicates, in the order
/// they were added. pyk sorts them by the length of their KAST text; the order does not change
/// the meaning.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CTerm {
    pub config: Pattern,
    pub constraints: Vec<Pattern>,
}

impl CTerm {
    pub fn new(config: Pattern, constraints: impl IntoIterator<Item = Pattern>) -> CTerm {
        let mut c = CTerm {
            config,
            constraints: Vec::new(),
        };
        c.add_constraints(constraints);
        c
    }

    /// Adds constraints, flattening conjunctions, dropping `\top` and those already there.
    pub fn add_constraints(&mut self, constraints: impl IntoIterator<Item = Pattern>) {
        for c in constraints {
            for c in conjuncts(c) {
                if !is_top(&c) && !self.constraints.contains(&c) {
                    self.constraints.push(c);
                }
            }
        }
    }

    pub fn with_constraints(&self, constraints: impl IntoIterator<Item = Pattern>) -> CTerm {
        let mut c = self.clone();
        c.add_constraints(constraints);
        c
    }

    /// The configuration of a state, with its substitution and predicate as constraints. The term
    /// itself can be a conjunction of the configuration and constraints too; as pyk's
    /// `CTerm.from_kast`, every conjunct that is not a matching-logic connective is the
    /// configuration.
    pub fn from_state(state: &State) -> CTerm {
        let mut parts = conjuncts(state.term.0.clone());
        let at = parts
            .iter()
            .position(|p| !matches!(p, Pattern::Ml(_)))
            .unwrap_or(0);
        let config = parts.remove(at);
        if let Some(s) = &state.substitution {
            parts.push(s.0.clone());
        }
        if let Some(p) = &state.predicate {
            parts.push(p.0.clone());
        }
        CTerm::new(config, parts)
    }

    /// As one pattern: `\and{SortGeneratedTopCell{}}(config, \and{...}(c1, \and{...}(c2, ...)))`,
    /// nested to the right like pyk's `mlAnd`; the configuration alone without constraints.
    pub fn to_kore(&self) -> Pattern {
        let mut all = Vec::with_capacity(1 + self.constraints.len());
        all.push(self.config.clone());
        all.extend(self.constraints.iter().cloned());
        and(top_sort(), all)
    }

    /// The constraints as one predicate (`\top` if none).
    pub fn constraint(&self) -> Pattern {
        and(top_sort(), self.constraints.clone())
    }
}

/// A substitution and constraints under which one constrained term is an instance of another
/// (the condition of a cover), in the form the server gives it.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct CSubst {
    /// A conjunction of `\equals(variable, value)`.
    pub substitution: Option<Pattern>,
    pub constraints: Vec<Pattern>,
}

/// The conjuncts of a pattern: the arguments of nested `\and`s, left to right.
pub fn conjuncts(p: Pattern) -> Vec<Pattern> {
    let mut out = Vec::new();
    let mut todo = vec![p];
    while let Some(p) = todo.pop() {
        match p {
            Pattern::Ml(Ml {
                conn: Conn::And,
                args,
                ..
            }) => todo.extend(args.into_vec().into_iter().rev()),
            p => out.push(p),
        }
    }
    out
}

/// The conjunction of patterns, nested to the right: `\top` if none, the pattern itself if one.
pub fn and(sort: Sort, mut ps: Vec<Pattern>) -> Pattern {
    let Some(mut acc) = ps.pop() else {
        return ml(Conn::Top, vec![sort], vec![]);
    };
    while let Some(p) = ps.pop() {
        acc = ml(Conn::And, vec![sort.clone()], vec![p, acc]);
    }
    acc
}

pub fn is_top(p: &Pattern) -> bool {
    matches!(
        p,
        Pattern::Ml(Ml {
            conn: Conn::Top,
            ..
        })
    )
}

fn ml(conn: Conn, sorts: Vec<Sort>, args: Vec<Pattern>) -> Pattern {
    Pattern::Ml(Ml::new(conn, sorts, None, args).expect("well-formed connective"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::text::parse_pattern;

    fn p(s: &str) -> Pattern {
        parse_pattern(s).unwrap()
    }

    #[test]
    fn flat_and_unique() {
        let a = p(
            r#"\equals{SortBool{}, SortGeneratedTopCell{}}(X : SortBool{}, \dv{SortBool{}}("true"))"#,
        );
        let b = p(r"\not{SortGeneratedTopCell{}}(\bottom{SortGeneratedTopCell{}}())");
        let both = and(top_sort(), vec![a.clone(), b.clone()]);
        let c = CTerm::new(
            p("c{}()"),
            [both, a.clone(), p(r"\top{SortGeneratedTopCell{}}()")],
        );
        assert_eq!(c.constraints, vec![a.clone(), b.clone()]);
        assert_eq!(
            c.to_kore().to_string(),
            format!(
                r"\and{{SortGeneratedTopCell{{}}}}(c{{}}(), \and{{SortGeneratedTopCell{{}}}}({a}, {b}))"
            )
        );
        assert_eq!(CTerm::new(p("c{}()"), []).to_kore(), p("c{}()"));
        assert_eq!(
            CTerm::new(p("c{}()"), []).constraint(),
            p(r"\top{SortGeneratedTopCell{}}()")
        );
    }
}
