//! The messages of the K JSON-RPC servers, as `pyk.kore.rpc` sends and reads them.
//!
//! Every pattern is a KORE JSON term with its wrapper (`{"format": "KORE", "version": 1, "term":
//! ...}`), see [`Kore`]. Optional parameters left at `None` are not sent; the server then uses its
//! default. Optional fields of results are `None` when the server leaves them out, so that a
//! result written back gives the same JSON.
//!
//! Log entries (`logs`, `haskell-log-entries`) are kept as JSON values; [`rewrite_rule_ids`]
//! reads the rules applied out of them.

use kore::Pattern;
use kore::json::{Term, TermOwned};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// A pattern as a KORE JSON term, with the format wrapper.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Kore(pub Pattern);

impl Serialize for Kore {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Term(&self.0).serialize(s)
    }
}

impl<'de> Deserialize<'de> for Kore {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Kore, D::Error> {
        TermOwned::deserialize(d).map(|t| Kore(t.0))
    }
}

impl From<Pattern> for Kore {
    fn from(p: Pattern) -> Kore {
        Kore(p)
    }
}

// ---------------------------------------------------------------------------------------------
// execute

/// Parameters of `execute`: rewrite `state` until a stop condition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExecuteParams {
    pub state: Kore,
    /// Stop after this many rewrite steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u64>,
    /// Stop before applying one of these rules (by label); the result has the state before and,
    /// in `next_states`, the state after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut_point_rules: Option<Vec<String>>,
    /// Stop after applying one of these rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_rules: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moving_average_step_timeout: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assume_state_defined: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_successful_rewrites: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_failed_rewrites: Option<bool>,
    /// Simplify with the booster only, without falling back to the Haskell backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub booster_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haskell_logging: Option<Vec<String>>,
}

impl ExecuteParams {
    pub fn new(state: Pattern) -> ExecuteParams {
        ExecuteParams {
            state: Kore(state),
            max_depth: None,
            cut_point_rules: None,
            terminal_rules: None,
            step_timeout: None,
            moving_average_step_timeout: None,
            assume_state_defined: None,
            module: None,
            log_successful_rewrites: None,
            log_failed_rewrites: None,
            booster_only: None,
            haskell_logging: None,
        }
    }
}

/// Why `execute` stopped.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StopReason {
    /// No rule applies.
    Stuck,
    DepthBound,
    Timeout,
    /// Several rules apply; `next_states` has the results.
    Branching,
    /// A cut-point rule applies; `next_states` has the result of applying it.
    CutPointRule,
    /// A terminal rule was applied.
    TerminalRule,
    /// The path condition is unsatisfiable.
    Vacuous,
    /// The server could not decide something it needed, see `unknown_predicate`.
    Aborted,
}

/// A configuration with its condition: `term` and `predicate` and `substitution` (both optional),
/// and, for a state that results from applying a rule, the rule and what it required.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct State {
    pub term: Kore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substitution: Option<Kore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<Kore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_substitution: Option<Kore>,
    /// The condition under which the rule applies: what distinguishes the branches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_predicate: Option<Kore>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExecuteResult {
    pub reason: StopReason,
    /// Rewrite steps taken.
    pub depth: u64,
    pub state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_states: Option<Vec<State>>,
    /// The cut-point or terminal rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown_predicate: Option<Kore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haskell_log_entries: Option<Vec<Value>>,
}

impl ExecuteResult {
    pub fn next_states(&self) -> &[State] {
        self.next_states.as_deref().unwrap_or_default()
    }

    /// The unique ids of the rules applied, in order, from the logs (present when the request
    /// asked for `log_successful_rewrites`).
    pub fn rule_ids(&self) -> Vec<&str> {
        rewrite_rule_ids(self.logs.as_deref().unwrap_or_default())
    }
}

/// The `rule-id` of every successful rewrite in a list of log entries.
pub fn rewrite_rule_ids(logs: &[Value]) -> Vec<&str> {
    logs.iter()
        .filter(|l| l.get("tag").and_then(Value::as_str) == Some("rewrite"))
        .filter_map(|l| l.get("result"))
        .filter(|r| r.get("tag").and_then(Value::as_str) == Some("success"))
        .filter_map(|r| r.get("rule-id").and_then(Value::as_str))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// implies

/// Parameters of `implies`: does `antecedent` imply `consequent`? Variables of the consequent
/// that do not occur in the antecedent must be bound by `\exists` (pyk does this).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ImpliesParams {
    pub antecedent: Kore,
    pub consequent: Kore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assume_defined: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub booster_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haskell_logging: Option<Vec<String>>,
}

impl ImpliesParams {
    pub fn new(antecedent: Pattern, consequent: Pattern) -> ImpliesParams {
        ImpliesParams {
            antecedent: Kore(antecedent),
            consequent: Kore(consequent),
            module: None,
            assume_defined: None,
            booster_only: None,
            haskell_logging: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImpliesStatus {
    Valid,
    Invalid,
    /// The booster could not decide. Reaches the client only in booster-only mode.
    Indeterminate,
}

/// The condition under which an implication holds: the substitution of the consequent's
/// variables, and a predicate.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Condition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substitution: Option<Kore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<Kore>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ImpliesResult {
    pub status: ImpliesStatus,
    pub implication: Kore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haskell_log_entries: Option<Vec<Value>>,
}

impl ImpliesResult {
    pub fn is_valid(&self) -> bool {
        self.status == ImpliesStatus::Valid
    }
}

// ---------------------------------------------------------------------------------------------
// simplify, get-model, add-module

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SimplifyParams {
    pub state: Kore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub booster_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haskell_logging: Option<Vec<String>>,
}

impl SimplifyParams {
    pub fn new(state: Pattern) -> SimplifyParams {
        SimplifyParams {
            state: Kore(state),
            module: None,
            booster_only: None,
            haskell_logging: None,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SimplifyResult {
    pub state: Kore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haskell_log_entries: Option<Vec<Value>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GetModelParams {
    pub state: Kore,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haskell_logging: Option<Vec<String>>,
}

impl GetModelParams {
    pub fn new(state: Pattern) -> GetModelParams {
        GetModelParams {
            state: Kore(state),
            module: None,
            haskell_logging: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Satisfiable {
    Sat,
    Unsat,
    Unknown,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GetModelResult {
    pub satisfiable: Satisfiable,
    /// For `Sat`: values of the variables, as a conjunction of `\equals`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substitution: Option<Kore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub haskell_log_entries: Option<Vec<Value>>,
}

/// Parameters of `add-module`: a KORE module, as text, whose axioms later requests can use by
/// naming the module.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AddModuleParams {
    pub module: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_as_id: Option<bool>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AddModuleResult {
    /// The id to name the module by.
    pub module: String,
}
