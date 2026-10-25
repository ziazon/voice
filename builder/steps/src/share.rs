//! Shares and percentages, rounded as the Python rounds them.

/// `round(n / total, 4)`, or 0 when there is nothing to divide by. Rust's fixed-precision formatting
/// and CPython's `round` both round the exact binary value, half to even.
pub fn share(n: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    format!("{:.4}", n as f64 / total as f64)
        .parse()
        .expect("a formatted float parses")
}

/// Python's `f"{x:.0%}"`: the value times 100, then rounded half to even.
pub fn percent(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_and_percentages_round_as_python_does() {
        // Expected values printed by CPython 3: round(n / total, 4) and f"{x:.0%}".
        assert_eq!(share(1, 32), 0.0312);
        assert_eq!(share(1, 8), 0.125);
        assert_eq!(share(2, 3), 0.6667);
        assert_eq!(share(0, 0), 0.0);
        assert_eq!(percent(0.125), "12%");
        assert_eq!(percent(0.375), "38%");
        assert_eq!(percent(share(1, 200)), "0%");
        assert_eq!(percent(0.0312), "3%");
        assert_eq!(percent(1.0), "100%");
        assert_eq!(serde_json::Value::from(share(1, 1)).to_string(), "1.0");
        assert_eq!(serde_json::Value::from(share(0, 3)).to_string(), "0.0");
        assert_eq!(
            serde_json::Value::from(share(1, 10000)).to_string(),
            "0.0001"
        );
    }
}
