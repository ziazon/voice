//! The exact git blob version of the decisions being refreshed.
pub const DECISIONS_MOST: u64 = 256 * 1024;
pub fn version(decisions: &[u8]) -> String {
    let mut hash = sha1_smol::Sha1::new();
    hash.update(format!("blob {}\0", decisions.len()).as_bytes());
    hash.update(decisions);
    hash.digest().to_string()
}
