use persona_builder::member::Member;
use persona_builder_build::{
    build::{Built, Counts},
    draft,
};
use persona_builder_grow::edit::{Edit, EditKind, apply_edit};
use persona_builder_steps::book::Book;
use serde_json::json;

fn decisions() -> String {
    let member = Member::from_value(json!({"name":"Robin", "slug":"robin", "principal":"Morgan", "principal_slug":"morgan", "state_dir":"~/.claude/state/robin", "mark_word":"Robin", "filters":{"excluded_folders":"^private$", "personal":"private", "secret":"secret", "pasted_prefixes":["You are "]}}), "member.json").unwrap();
    let book = Book {
        categories: vec!["scope".into()],
        domains: vec![],
        escalates: vec![],
        signals: vec![],
        groups: vec![],
    };
    let built = Built { sections: "### scope\n\n**The lean:**\n- Keep the goal small.\n\n**What flips it:**\n- A deadline changes the order.\n\n**When they write their own answer:**\n- Ask for a clearer aim.\n\n**Standing rules:**\n- None yet.\n\n**Precedents:**\n- Once we waited.\n".into(), tally: vec![], backtest: None, held: 0, counts: Counts::default(), calls: 0, reused: 0 };
    draft::decisions(&member, &book, &built)
}
fn add(area: &str, text: &str) -> Edit {
    Edit {
        area: area.into(),
        kind: EditKind::Add,
        old: None,
        text: text.into(),
        why: "Learned from an answer.".into(),
        cases: vec!["case-a".into()],
    }
}
fn replace(old: &str) -> Edit {
    Edit {
        kind: EditKind::Replace,
        old: Some(old.into()),
        ..add("scope", "Choose a smaller goal.")
    }
}
#[test]
fn add_to_empty_and_nonempty_rules() {
    let old = decisions();
    let first = apply_edit(&old, &add("scope", "Start with a small trial.")).unwrap();
    assert!(first.contains("**Standing rules:**\n- Start with a small trial.\n"));
    assert!(!first.contains("- None yet."));
    let second = apply_edit(&first, &add("scope", "Check the aim first.")).unwrap();
    assert!(second.contains("- Start with a small trial.\n- Check the aim first.\n"));
    assert!(apply_edit(&second, &add("scope", "Check the aim first.")).is_err());
    assert!(apply_edit(&old, &add("scope", "Keep the goal small.")).is_err());
}
#[test]
fn adds_missing_list_or_area_before_next_heading() {
    let old = decisions().replace("**Standing rules:**\n- None yet.\n\n", "");
    let new = apply_edit(&old, &add("scope", "Start with a small trial.")).unwrap();
    assert!(new.contains("**Standing rules:**\n- Start with a small trial.\n"));
    let old = old + "\n## Later notes\n\nLeave these here.\n";
    let new = apply_edit(&old, &add("methods", "Start with a small trial.")).unwrap();
    assert!(new.contains(
        "### methods\n\n**Standing rules:**\n- Start with a small trial.\n\n## Later notes"
    ));
    assert!(new.ends_with("Leave these here.\n"));
}
#[test]
fn replaces_any_point_except_precedents_or_placeholder() {
    let old = decisions();
    for point in [
        "Keep the goal small.",
        "A deadline changes the order.",
        "Ask for a clearer aim.",
    ] {
        let new = apply_edit(&old, &replace(&format!(" {point} "))).unwrap();
        assert!(!new.contains(point));
        assert!(new.contains("- Choose a smaller goal.\n"));
    }
    for point in ["Once we waited.", "None yet.", "Missing point."] {
        assert!(apply_edit(&old, &replace(point)).is_err());
    }
    let doubled = old.replace("A deadline changes the order.", "Keep the goal small.");
    assert!(apply_edit(&doubled, &replace("Keep the goal small.")).is_err());
    assert!(
        apply_edit(
            &old.replace("## By kind of question", "## Other"),
            &add("scope", "A rule.")
        )
        .is_err()
    );
}
