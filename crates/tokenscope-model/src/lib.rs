//! Typed data model shared by every TokenScope component.
//!
//! Nothing in this crate executes programs or touches the network; it only
//! defines serializable, validated data.

/// Version of the persisted JSON formats produced by this crate (see `FORMAT.md`).
pub const SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
