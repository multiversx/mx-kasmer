//! Names of rules for the edges of the graph. The server identifies a rule by its unique id (a
//! hash); pyk writes `<label>:<source file>:(<location>)`, with the unique id when the rule has
//! no label. The attributes come from the axioms of the definition.

use std::collections::HashMap;

use kore::{Definition, Pattern};

#[derive(Clone, Debug, Default)]
pub struct RuleLabels {
    by_id: HashMap<String, String>,
}

impl RuleLabels {
    /// No names: every rule is `<unique id>:UNKNOWN`, as pyk writes rules it does not know.
    pub fn none() -> RuleLabels {
        RuleLabels::default()
    }

    pub fn from_definition(definition: &Definition) -> RuleLabels {
        let mut by_id = HashMap::new();
        for axiom in definition.axioms() {
            let attr = |name: &str| {
                axiom
                    .attrs
                    .iter()
                    .find(|a| &*a.symbol == name)
                    .and_then(|a| match a.args.first() {
                        Some(Pattern::String(s)) => Some(s.as_str()),
                        _ => None,
                    })
            };
            let Some(id) = attr("UNIQUE'Unds'ID") else {
                continue;
            };
            let label = attr("label").unwrap_or(id);
            let source = attr("org'Stop'kframework'Stop'attributes'Stop'Source")
                .and_then(|s| s.strip_prefix("Source(")?.strip_suffix(')'));
            let location = attr("org'Stop'kframework'Stop'attributes'Stop'Location")
                .and_then(|s| s.strip_prefix("Location(")?.strip_suffix(')'))
                .map(|l| l.split(',').collect::<Vec<_>>().join(", "));
            let name = match (source, location) {
                (Some(s), Some(l)) => format!("{label}:{s}:({l})"),
                _ => label.to_owned(),
            };
            by_id.insert(id.to_owned(), name);
        }
        RuleLabels { by_id }
    }

    pub fn of(&self, id: &str) -> String {
        match self.by_id.get(id) {
            Some(name) => name.clone(),
            None => format!("{id}:UNKNOWN"),
        }
    }

    pub fn of_all(&self, ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| self.of(id)).collect()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}
