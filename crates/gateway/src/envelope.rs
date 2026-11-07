//! Reading the outer frame without decoding the body, `ARCHITECTURE.md` 10.1.

use dr_model::Envelope;
use std::ops::Range;

/// Reads `op`, `s`, `t` and the raw `d` of a text frame. Unparsable text is dropped and counted by
/// the caller, the shard continues.
pub fn parse(text: &str) -> Result<Envelope<'_>, serde_json::Error> {
    serde_json::from_str(text)
}

/// Where a raw `d` sits inside the text it was parsed from, so that the buffer can be handed over
/// with no copy. `d` must borrow from `text`, which [`parse`] guarantees for a body with no escaping
/// of the value itself.
pub fn d_range(text: &str, d: &str) -> Option<Range<usize>> {
    // Addresses are only compared here, and the result is checked against the text itself.
    let start = (d.as_ptr() as usize).checked_sub(text.as_ptr() as usize)?;
    let range = start..start.checked_add(d.len())?;
    (text.get(range.clone()) == Some(d)).then_some(range)
}
