use persona_builder_grow::diff::{apply, unified};

fn roundtrip(old: &str, new: &str) -> String {
    let change = unified("decisions.md", old, new).unwrap();
    assert_eq!(apply(old, &change, "decisions.md").unwrap(), new);
    change
}
#[test]
fn added_and_replaced_lines() {
    assert_eq!(
        roundtrip("a\n", "a\nb\n"),
        "--- a/decisions.md\n+++ b/decisions.md\n@@ -1,1 +1,2 @@\n a\n+b\n"
    );
    assert_eq!(
        roundtrip("a\n", "b\n"),
        "--- a/decisions.md\n+++ b/decisions.md\n@@ -1,1 +1,1 @@\n-a\n+b\n"
    );
    assert!(roundtrip("", "a\n").contains("@@ -0,0 +1,1 @@"));
    assert!(roundtrip("a\n", "").contains("@@ -1,1 +0,0 @@"));
}
#[test]
fn clipped_context_and_merged_or_separate_hunks() {
    let old = (0..20).map(|n| format!("{n}\n")).collect::<String>();
    let both = roundtrip(
        &old,
        &old.replacen("0\n", "first\n", 1).replace("19\n", "last\n"),
    );
    assert_eq!(both.matches("@@ -").count(), 2);
    assert!(both.contains("@@ -1,4 +1,4 @@"));
    assert!(both.contains("@@ -17,4 +17,4 @@"));
    let near = roundtrip(
        &old,
        &old.replace("3\n", "three\n").replace("7\n", "seven\n"),
    );
    assert_eq!(near.matches("@@ -").count(), 1);
}
#[test]
fn final_newlines_and_carriage_returns_are_exact() {
    for (old, new) in [
        ("a", "b"),
        ("a\n", "a"),
        ("a", "a\n"),
        ("a\r\n", "b\r\n"),
        ("", ""),
        ("a\n", "a\n"),
    ] {
        let change = roundtrip(old, new);
        if !old.is_empty() && !old.ends_with('\n') || !new.is_empty() && !new.ends_with('\n') {
            assert!(change.contains("\\ No newline at end of file"));
        }
    }
}
#[test]
fn strict_refusals() {
    let good = unified("decisions.md", "a\nb\nc\n", "a\nB\nc\n").unwrap();
    let bad = [
        good.replace(" a\n", " x\n"),
        good.replace("-1,3", "-1,4"),
        good.replace("+1,3", "+1,4"),
        good.trim_end().to_owned(),
        good.replace("@@ -1,3", "@@ -x,3"),
        good.replace(" a\n", "?a\n"),
        good.replace("a/decisions.md", "a/wrong.md"),
        good.replace("b/decisions.md", "b/wrong.md"),
        format!("{good}@@ -2,1 +2,1 @@\n-b\n+B\n"),
        good.replace("+1,3", "+2,3"),
        good.replace("-1,3", "-0,3"),
    ];
    for bad in bad {
        assert!(apply("a\nb\nc\n", &bad, "decisions.md").is_err(), "{bad}");
    }
    assert!(apply("a\nb\n", &good, "decisions.md").is_err());
    assert!(apply("a\nb\nc", &good, "decisions.md").is_err());
}
#[test]
fn bounded_changed_region_not_whole_file() {
    let old = "same\n".repeat(3000);
    roundtrip(&old, &(old.clone() + "new\n"));
    assert!(unified("decisions.md", &"old\n".repeat(2001), &"new\n".repeat(2001)).is_err());
}
