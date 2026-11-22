//! Helpers for reading JSON objects field by field.

use serde::de;

/// Stores a field, rejecting duplicates.
pub(crate) fn set<T, E: de::Error>(slot: &mut Option<T>, value: T, name: &'static str) -> Result<(), E> {
    if slot.replace(value).is_some() {
        return Err(E::duplicate_field(name));
    }
    Ok(())
}

/// A field that must be present.
pub(crate) fn need<T, E: de::Error>(slot: Option<T>, name: &'static str) -> Result<T, E> {
    slot.ok_or_else(|| E::missing_field(name))
}
