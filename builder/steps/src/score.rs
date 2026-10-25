//! `score`: the blind backtest's predictions against the held-out answers, per confidence tier and
//! per category, beside the baseline of always taking the recommendation.
use crate::{
    input::{Record, Records},
    labels::category,
    share::{percent, share},
};
use persona_builder::{
    error::{Error, Result},
    json::to_python_json,
    member::Member,
};
use persona_builder_ingest::{mine::Mark, python::truthy};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashSet};

pub const TIERS: [&str; 3] = ["high", "medium", "low"];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub n: usize,
    pub hits: usize,
}
impl Cell {
    fn add(&mut self, hit: bool) {
        self.n += 1;
        self.hits += usize::from(hit);
    }
    pub fn accuracy(&self) -> f64 {
        share(self.hits, self.n)
    }
    fn json(&self) -> Map<String, Value> {
        Map::from_iter([
            ("n".to_owned(), Value::from(self.n)),
            ("hits".to_owned(), Value::from(self.hits)),
            ("accuracy".to_owned(), Value::from(self.accuracy())),
        ])
    }
}

pub struct Score {
    pub overall: Cell,
    pub baseline: usize,
    /// The questions the member marked a pick on, and how often the principal took it.
    pub led: Cell,
    pub took_member: usize,
    pub tiers: [Option<Cell>; 3],
    pub categories: BTreeMap<String, [Option<Cell>; 3]>,
}

fn tally(tiers: &mut [Option<Cell>; 3], tier: usize, hit: bool) {
    tiers[tier].get_or_insert_default().add(hit);
}

fn bare_set(mark: &Mark, r: &Record, key: &str) -> Result<HashSet<String>> {
    Ok(r.strings(key)?.into_iter().map(|p| mark.bare(p)).collect())
}

/// The Python's `is_hit`, reading the answer's fields only where it reads them.
fn is_hit(mark: &Mark, a: &Record, prediction: &Map<String, Value>) -> Result<bool> {
    let pick = prediction.get("pick");
    if a.get("outcome")? == "own_words" {
        return Ok(pick.is_some_and(|p| p == "own"));
    }
    if truthy(a.get("multiSelect")?) {
        let picks: HashSet<String> = match pick {
            Some(Value::Array(list)) => list
                .iter()
                .filter_map(Value::as_str)
                .map(|p| mark.bare(p))
                .collect(),
            Some(Value::String(p)) => HashSet::from([mark.bare(p)]),
            _ => HashSet::new(),
        };
        return Ok(picks == bare_set(mark, a, "picks")?);
    }
    let picks = a.strings("picks")?;
    Ok(match (picks.first(), pick) {
        (Some(first), Some(Value::String(p))) => mark.bare(p) == mark.bare(first),
        _ => false,
    })
}

pub fn score(
    member: &Member,
    answers_name: &str,
    answers: &Records,
    labels_name: &str,
    labels: &Map<String, Value>,
    predictions: &Map<String, Value>,
) -> Result<Score> {
    let mark = Mark::new(&member.mark_word)?;
    let pick_field = member.pick_field();
    let mut missing = Vec::new();
    for a in Record::all(answers_name, answers) {
        let id = a.id()?;
        if !predictions.contains_key(id) {
            missing.push(id);
        }
    }
    if !missing.is_empty() {
        missing.sort_unstable();
        return Err(Error::Invalid(format!(
            "no prediction for {}",
            missing.join(", ")
        )));
    }
    let mut score = Score {
        overall: Cell::default(),
        baseline: 0,
        led: Cell::default(),
        took_member: 0,
        tiers: [None; 3],
        categories: BTreeMap::new(),
    };
    for a in Record::all(answers_name, answers) {
        let id = a.id()?;
        let p = predictions[id].as_object().ok_or_else(|| {
            Error::Invalid(format!("the prediction for {id} is not a JSON object"))
        })?;
        let tier = p
            .get("confidence")
            .and_then(Value::as_str)
            .and_then(|t| TIERS.iter().position(|tier| *tier == t))
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "{id}: confidence must be one of {}",
                    TIERS.join(", ")
                ))
            })?;
        let hit = is_hit(&mark, &a, p)?;
        score.overall.add(hit);
        score.baseline += usize::from(a.get("outcome")? == "took_rec");
        if let Some(led) = a.map.get(&pick_field).filter(|v| truthy(v)) {
            let led = led.as_str().ok_or_else(|| a.shape(&pick_field))?;
            score.led.add(hit);
            score.took_member +=
                usize::from(bare_set(&mark, &a, "picks")?.contains(&mark.bare(led)));
        }
        tally(&mut score.tiers, tier, hit);
        let name = match labels.get(id) {
            Some(label) if label.get("category").is_some() => category(labels_name, labels, id)?,
            _ => "unlabelled",
        };
        tally(
            score.categories.entry(name.to_owned()).or_default(),
            tier,
            hit,
        );
    }
    Ok(score)
}

fn tiers_json(tiers: &[Option<Cell>; 3]) -> Value {
    Value::Object(
        TIERS
            .iter()
            .zip(tiers)
            .filter_map(|(t, c)| c.map(|c| ((*t).to_owned(), Value::Object(c.json()))))
            .collect(),
    )
}

/// The Python's `json.dumps(result, indent=2)`: everything outside printable ASCII escaped.
pub fn score_json(member: &Member, s: &Score) -> Result<String> {
    let mut overall = s.overall.json();
    overall.insert(
        "baseline".into(),
        Value::from(share(s.baseline, s.overall.n)),
    );
    let mut led = s.led.json();
    led.insert(
        format!("took_{}", member.slug),
        Value::from(share(s.took_member, s.led.n)),
    );
    let categories: Map<String, Value> = s
        .categories
        .iter()
        .map(|(name, tiers)| (name.clone(), tiers_json(tiers)))
        .collect();
    let result = Map::from_iter([
        ("overall".to_owned(), Value::Object(overall)),
        (format!("{}_led", member.slug), Value::Object(led)),
        ("tiers".to_owned(), tiers_json(&s.tiers)),
        ("categories".to_owned(), Value::Object(categories)),
    ]);
    to_python_json(&Value::Object(result))
}

pub fn score_markdown(member: &Member, s: &Score) -> String {
    let o = s.overall;
    let mut lines = vec![format!(
        "Overall: {} of {} ({}); always-take-the-recommendation baseline {}",
        o.hits,
        o.n,
        percent(o.accuracy()),
        percent(share(s.baseline, o.n))
    )];
    if s.led.n > 0 {
        lines.push(format!(
            "{}-led questions: {} of {} ({}); {} took {}'s pick {}",
            member.name,
            s.led.hits,
            s.led.n,
            percent(s.led.accuracy()),
            member.principal,
            member.name,
            percent(share(s.took_member, s.led.n))
        ));
    }
    lines.extend([
        "".into(),
        "| Tier | N | Hits | Accuracy |".into(),
        "|---|---|---|---|".into(),
    ]);
    for (t, c) in TIERS.iter().zip(&s.tiers) {
        if let Some(c) = c {
            lines.push(format!(
                "| {t} | {} | {} | {} |",
                c.n,
                c.hits,
                percent(c.accuracy())
            ));
        }
    }
    lines.extend([
        "".into(),
        format!("| Category | {} |", TIERS.join(" | ")),
        format!("|---|{}", "---|".repeat(TIERS.len())),
    ]);
    for (name, tiers) in &s.categories {
        let cells: Vec<String> = tiers
            .iter()
            .map(|c| match c {
                Some(c) => format!("{}/{}", c.hits, c.n),
                None => "–".into(),
            })
            .collect();
        lines.push(format!("| {name} | {} |", cells.join(" | ")));
    }
    lines.join("\n") + "\n"
}
