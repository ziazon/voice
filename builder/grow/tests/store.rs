use persona_builder::{error::Result, member::Member};
use persona_builder_grow::{record::*, store::Store};
use persona_builder_ingest::private::{lock_state, write_private_at};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "growth-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn growth(&self) -> PathBuf {
        self.0.join(".claude/state/robin/growth")
    }
    fn store(&self) -> Store {
        Store::open(&self.0, &member()).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn member() -> Member {
    Member::from_value(json!({"name":"Robin","slug":"robin","principal":"Robin","principal_slug":"robin","state_dir":"~/.claude/state/robin","mark_word":"mark","filters":{"excluded_folders":"","personal":"","secret":"","pasted_prefixes":[]}}),"fixture").unwrap()
}
fn example<T: Record>(name: &str) -> T {
    parse(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../docs/spec/growth-loop/examples/{name}.json")),
        )
        .unwrap(),
    )
    .unwrap()
}
fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}
#[test]
fn writes_private_records_and_reads_them_back() {
    let s = Scratch::new();
    let store = s.store();
    let signal: Signal = example("signal");
    let case: Case = example("case");
    assert!(store.append_signal(&signal).is_ok());
    assert_eq!(store.signals().unwrap(), vec![signal]);
    store.append_case(&case).unwrap();
    assert_eq!(store.case_lines().unwrap(), vec![case.clone()]);
    assert_eq!(store.cases().unwrap().get(&case.id), Some(&case));
    assert_eq!(mode(&s.growth()), 0o700);
    for file in ["signals.jsonl", "cases.jsonl"] {
        assert_eq!(mode(&s.growth().join(file)), 0o600);
    }
}
#[test]
fn slug_must_match_the_persona_pattern() {
    let s = Scratch::new();
    let mut m = member();
    for slug in ["Robin", "../robin", "robin_1", ""] {
        m.slug = slug.into();
        assert!(Store::open(&s.0, &m).is_err(), "{slug}");
    }
}
#[test]
fn linked_growth_is_refused() {
    let s = Scratch::new();
    write_private_at(&s.0, "robin", None, "empty.json", b"{}").unwrap();
    let outside = s.0.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, s.growth()).unwrap();
    assert!(s.store().append_signal(&example("signal")).is_err());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    assert!(s.store().signals().is_err());
}
#[test]
fn append_keeps_every_old_byte() {
    let s = Scratch::new();
    let signal: Signal = example("signal");
    let old = format!(" {}  \n", serde_json::to_string(&signal).unwrap());
    write_private_at(
        &s.0,
        "robin",
        Some("growth"),
        "signals.jsonl",
        old.as_bytes(),
    )
    .unwrap();
    let mut next = signal;
    next.id = "sig-next".into();
    s.store().append_signal(&next).unwrap();
    let bytes = fs::read(s.growth().join("signals.jsonl")).unwrap();
    assert!(bytes.len() > old.len());
    assert!(bytes.starts_with(old.as_bytes()));
    assert_eq!(s.store().signals().unwrap().len(), 2);
}
#[test]
fn damaged_lines_refuse_without_changing_bytes() {
    let s = Scratch::new();
    for file in ["signals.jsonl", "cases.jsonl"] {
        let old = b"{\"situation\":\"sentinel\"}\n";
        write_private_at(&s.0, "robin", Some("growth"), file, old).unwrap();
        let result: Result<()> = if file == "signals.jsonl" {
            s.store().append_signal(&example("signal"))
        } else {
            s.store().append_case(&example("case"))
        };
        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains(file) && message.contains('1'), "{message}");
        assert!(!message.contains("sentinel"));
        assert_eq!(fs::read(s.growth().join(file)).unwrap(), old);
    }
}
#[test]
fn ids_count_existing_records_by_prefix_and_day() {
    let s = Scratch::new();
    let store = s.store();
    assert_eq!(
        store.next_id("sig", "2026-03-04").unwrap(),
        "sig-2026-03-04-001"
    );
    let mut signal: Signal = example("signal");
    store.append_signal(&signal).unwrap();
    assert_eq!(
        store.next_id("sig", "2026-03-04").unwrap(),
        "sig-2026-03-04-002"
    );
    assert_eq!(
        store.next_id("sig", "2026-03-05").unwrap(),
        "sig-2026-03-05-001"
    );
    signal.id = "sig-2026-03-04-999".into();
    store.append_signal(&signal).unwrap();
    assert_eq!(
        store.next_id("sig", "2026-03-04").unwrap(),
        "sig-2026-03-04-1000"
    );
    store.append_case(&example("case")).unwrap();
    assert_eq!(
        store.next_id("case", "2026-03-04").unwrap(),
        "case-2026-03-04-002"
    );
    store.save_proposal(&example("proposal")).unwrap();
    assert_eq!(
        store.next_id("prop", "2026-03-05").unwrap(),
        "prop-2026-03-05-002"
    );
}
#[test]
fn json_records_cannot_be_rewritten_and_readers_ignore_other_names() {
    let s = Scratch::new();
    let store = s.store();
    let p: Proposal = example("proposal");
    let a: Approval = example("approval");
    let m: Measurement = example("measurement");
    assert!(store.save_proposal(&p).is_ok());
    assert_eq!(store.proposal(&p.id).unwrap(), Some(p.clone()));
    store.save_approval(&a).unwrap();
    store.save_measurement(&m).unwrap();
    for result in [
        store.save_proposal(&p),
        store.save_approval(&a),
        store.save_measurement(&m),
    ] {
        assert!(result.is_err());
    }
    for file in ["unrelated.json", "prop-wrong.txt", ".prop-hidden.json"] {
        fs::write(s.growth().join(file), b"sentinel").unwrap();
    }
    assert_eq!(store.proposals().unwrap(), vec![p]);
    assert_eq!(store.approvals().unwrap(), vec![a.clone()]);
    assert_eq!(store.measurements().unwrap(), vec![m.clone()]);
    assert_eq!(store.approval(&a.id).unwrap(), Some(a));
    assert_eq!(store.measurement(&m.id).unwrap(), Some(m));
    assert!(store.proposal("prop-missing").unwrap().is_none());
    for path in fs::read_dir(s.growth())
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(['a', 'm']))
    {
        assert_eq!(mode(&path.path()), 0o600);
    }
}
#[test]
fn writes_refuse_an_already_held_lock() {
    let s = Scratch::new();
    let store = s.store();
    let _lock = lock_state(&s.0, "robin").unwrap();
    for result in [
        store.append_signal(&example("signal")),
        store.append_case(&example("case")),
        store.save_proposal(&example("proposal")),
        store.save_approval(&example("approval")),
        store.save_measurement(&example("measurement")),
    ] {
        assert!(result.is_err());
    }
    assert!(!s.growth().exists());
}
#[test]
fn check_all_checks_references_and_append_history() {
    let s = Scratch::new();
    let store = s.store();
    assert!(store.check_all().is_ok());
    store.append_case(&example("case")).unwrap();
    assert!(store.check_all().is_err());
    store.append_signal(&example("signal")).unwrap();
    assert!(store.check_all().is_ok());
    let mut changed: Case = example("case");
    changed.situation = "A different invented situation".into();
    assert!(store.append_case(&changed).is_err());
}

#[test]
fn json_record_id_must_match_its_file_name() {
    let s = Scratch::new();
    let store = s.store();
    for (kind, prefix) in [
        ("proposal", "prop"),
        ("approval", "appr"),
        ("measurement", "meas"),
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(
            &fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../docs/spec/growth-loop/examples/{kind}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let id = value["id"].as_str().unwrap().to_owned();
        value["id"] = json!(format!("{prefix}-other"));
        write_private_at(
            &s.0,
            "robin",
            Some("growth"),
            &format!("{id}.json"),
            &serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        let (one, many) = match kind {
            "proposal" => (
                store.proposal(&id).map(|_| ()),
                store.proposals().map(|_| ()),
            ),
            "approval" => (
                store.approval(&id).map(|_| ()),
                store.approvals().map(|_| ()),
            ),
            _ => (
                store.measurement(&id).map(|_| ()),
                store.measurements().map(|_| ()),
            ),
        };
        assert!(
            one.is_err(),
            "{kind}: singular reader must refuse a different id"
        );
        assert!(
            many.is_err(),
            "{kind}: plural reader must refuse a different id"
        );
        fs::remove_file(s.growth().join(format!("{id}.json"))).unwrap();
    }
}

#[test]
fn check_all_refuses_duplicate_signal_ids_already_on_disk() {
    let s = Scratch::new();
    let store = s.store();
    let signal: Signal = example("signal");
    let line = to_line(&signal);
    write_private_at(
        &s.0,
        "robin",
        Some("growth"),
        "signals.jsonl",
        format!("{line}{line}").as_bytes(),
    )
    .unwrap();
    let result = store.check_all();
    assert!(
        result.is_err(),
        "signals must have unique ids even when read from disk"
    );
    assert!(result.unwrap_err().to_string().contains(&signal.id));
}

fn seed_proposal(store: &Store) {
    store.append_signal(&example("signal")).unwrap();
    store.append_case(&example("case")).unwrap();
    store.save_proposal(&example("proposal")).unwrap();
}

#[test]
fn check_all_checks_approval_role_without_requiring_approval() {
    let s = Scratch::new();
    let store = s.store();
    seed_proposal(&store);
    let mut approval: Approval = example("approval");
    approval.decision = Decision::Rejected;
    approval.role = GivenBy::Staff;
    store.save_approval(&approval).unwrap();
    let result = store.check_all();
    assert!(
        result.is_err(),
        "a rejected answer still needs the proposal's approver role"
    );
    assert!(result.unwrap_err().to_string().contains(&approval.id));
    fs::remove_file(s.growth().join(format!("{}.json", approval.id))).unwrap();
    approval.role = GivenBy::Subject;
    store.save_approval(&approval).unwrap();
    assert!(
        store.check_all().is_ok(),
        "the right approver can reject a proposal"
    );
}

#[test]
fn check_all_checks_measurement_base_against_its_proposal() {
    let s = Scratch::new();
    let store = s.store();
    seed_proposal(&store);
    store.save_approval(&example("approval")).unwrap();
    let mut measurement: Measurement = example("measurement");
    measurement.results.truncate(1);
    measurement.base = "sentinel-private-version".into();
    store.save_measurement(&measurement).unwrap();
    let result = store.check_all();
    assert!(
        result.is_err(),
        "measurement and proposal must name the same base"
    );
    let message = result.unwrap_err().to_string();
    assert!(message.contains(&measurement.id));
    assert!(!message.contains("sentinel-private-version"));
}

fn measurement_fixture() -> (Scratch, Store, Measurement) {
    let scratch = Scratch::new();
    let store = scratch.store();
    seed_proposal(&store);
    store.save_approval(&example("approval")).unwrap();
    let mut measurement: Measurement = example("measurement");
    measurement.results.truncate(1);
    (scratch, store, measurement)
}

#[test]
fn check_all_keep_requires_exactly_the_proposals_new_cases() {
    for mutation in 0..3 {
        let (_scratch, store, mut measurement) = measurement_fixture();
        let mut other: Case = example("case");
        other.id = "case-other".into();
        store.append_case(&other).unwrap();
        match mutation {
            0 => measurement.results[0].new = false,
            1 => measurement.results[0].case = other.id.clone(),
            _ => measurement.results.push(CaseResult {
                case: other.id,
                ..measurement.results[0].clone()
            }),
        }
        store.save_measurement(&measurement).unwrap();
        assert!(
            store.check_all().is_err(),
            "Keep must name exactly the proposal's new cases: mutation {mutation}"
        );
    }
}

#[test]
fn check_all_keep_refuses_a_failing_new_case_but_accepts_revert() {
    let (scratch, store, mut measurement) = measurement_fixture();
    measurement.results[0].after = Grade::Fail;
    store.save_measurement(&measurement).unwrap();
    assert!(
        store.check_all().is_err(),
        "a failing new case cannot produce Keep"
    );
    fs::remove_file(scratch.growth().join(format!("{}.json", measurement.id))).unwrap();
    measurement.outcome = KeepOrRevert::Revert;
    store.save_measurement(&measurement).unwrap();
    assert!(
        store.check_all().is_ok(),
        "failed new cases can produce Revert"
    );
}

#[test]
fn check_all_keep_refuses_an_existing_case_regression() {
    let (_scratch, store, mut measurement) = measurement_fixture();
    let mut old: Case = example("case");
    old.id = "case-old".into();
    old.status = Status::Active;
    store.append_case(&old).unwrap();
    measurement.results.push(CaseResult {
        case: old.id,
        new: false,
        before: Grade::Pass,
        after: Grade::Fail,
    });
    store.save_measurement(&measurement).unwrap();
    assert!(
        store.check_all().is_err(),
        "an existing case regression cannot produce Keep"
    );
}

#[test]
fn check_all_measurement_results_must_name_existing_cases() {
    for outcome in [KeepOrRevert::Keep, KeepOrRevert::Revert] {
        let (_scratch, store, mut measurement) = measurement_fixture();
        measurement.outcome = outcome;
        measurement.results.push(CaseResult {
            case: "case-missing".into(),
            new: false,
            before: Grade::Pass,
            after: Grade::Pass,
        });
        store.save_measurement(&measurement).unwrap();
        assert!(
            store.check_all().is_err(),
            "every measured case must exist, including for {outcome:?}"
        );
    }
}
