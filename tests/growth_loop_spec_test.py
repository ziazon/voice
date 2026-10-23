"""Checks docs/spec/growth-loop.md against its JSON Schemas and invented examples."""
import copy
import json
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from schema_subset import UnsupportedSchema, validate

SPEC = Path(__file__).resolve().parent.parent / "docs/spec"
DOC = SPEC / "growth-loop.md"
SCHEMAS = SPEC / "growth-loop"
EXAMPLES = SCHEMAS / "examples"
KINDS = ["signal", "case", "proposal", "approval", "measurement"]
APPROVER = {"person": "subject", "product": "staff"}


def load(path):
    return json.loads(path.read_text())


def schema(kind):
    return load(SCHEMAS / f"{kind}.schema.json")


def example(kind):
    return load(EXAMPLES / f"{kind}.json")


def doc_table(kind):
    """The field table under `### <n>. <Kind>`, as {field: required}."""
    section = re.search(rf"^### \d\. {kind.capitalize()}\n(.*?)(?=^### |^## |\Z)",
                        DOC.read_text(), re.M | re.S)
    rows = re.findall(r"^\| `([a-z_]+)` \| (yes|no) \|", section.group(1), re.M)
    return {field: required == "yes" for field, required in rows}


def doc_example(kind):
    after = DOC.read_text().split(f"Example (`growth-loop/examples/{kind}.json`):\n", 1)[1]
    return json.loads(re.match(r"\s*```json\n(.*?)\n```", after, re.S).group(1))


def expected_outcome(measurement, approval):
    """Spec rule 3: keep only an approved change whose new cases pass with no regressions."""
    results = measurement["results"]
    new_pass = all(r["after"] == "pass" for r in results if r["new"])
    regressed = any(r["before"] == "pass" and r["after"] == "fail" for r in results if not r["new"])
    keep = approval["decision"] == "approved" and new_pass and not regressed
    return "keep" if keep else "revert"


class Validator(unittest.TestCase):
    OBJ = {"type": "object", "additionalProperties": False, "required": ["a"],
           "properties": {"a": {"type": "string", "enum": ["x", "y"]},
                          "b": {"type": "string", "pattern": "^ok-"},
                          "c": {"type": "array", "items": {"type": "string"},
                                "minItems": 2, "uniqueItems": True},
                          "d": {"type": "integer"},
                          "e": {"const": "v1"},
                          "f": {"type": "string", "minLength": 1}}}

    def errors(self, value, sch=None):
        return validate(value, sch or self.OBJ)

    def test_a_valid_value_has_no_errors(self):
        self.assertEqual(self.errors({"a": "x", "b": "ok-1", "c": ["p", "q"], "d": 3, "e": "v1",
                                      "f": "z"}), [])

    def test_each_broken_rule_is_reported_at_its_path(self):
        cases = [({}, "/: missing required"),
                 ({"a": "x", "z": 1}, "/z: not allowed"),
                 ({"a": "w"}, "/a: "),
                 ({"a": "x", "e": "v2"}, "/e: "),
                 ({"a": "x", "b": "no-1"}, "/b: "),
                 ({"a": "x", "c": ["p"]}, "/c: "),
                 ({"a": "x", "c": ["p", "p"]}, "/c: "),
                 ({"a": "x", "c": ["p", 3]}, "/c/1: "),
                 ({"a": "x", "d": True}, "/d: "),
                 ({"a": "x", "d": 1.5}, "/d: "),
                 ({"a": "x", "f": ""}, "/f: "),
                 ([], "/: ")]
        for value, start in cases:
            with self.subTest(value=value):
                errors = self.errors(value)
                self.assertEqual(len(errors), 1, errors)
                self.assertTrue(errors[0].startswith(start), errors)

    def test_type_lists_and_booleans(self):
        self.assertEqual(validate(None, {"type": ["string", "null"]}), [])
        self.assertEqual(validate(True, {"type": "boolean"}), [])
        self.assertEqual(len(validate(1, {"type": "boolean"})), 1)

    def test_a_local_ref_resolves_against_the_root(self):
        sch = {"$defs": {"id": {"type": "string", "pattern": "^sig-"}},
               "type": "array", "items": {"$ref": "#/$defs/id"}}
        self.assertEqual(validate(["sig-1"], sch), [])
        self.assertEqual(len(validate(["case-1"], sch)), 1)

    def test_unsupported_keywords_and_refs_raise(self):
        for sch in [{"type": "string", "format": "date-time"},
                    {"properties": {"a": {"maxLength": 3}}},
                    {"properties": {"absent": {"items": {"format": "uri"}}}},
                    {"type": "text"},
                    {"$ref": "other.json#/x"},
                    {"$ref": "#/$defs/missing"}]:
            with self.subTest(schema=sch):
                with self.assertRaises(UnsupportedSchema):
                    validate({"a": "x"}, sch)


class Spec(unittest.TestCase):
    def test_the_folders_hold_exactly_the_five_kinds(self):
        self.assertEqual(sorted(p.name for p in SCHEMAS.iterdir() if p.is_file()),
                         sorted(f"{k}.schema.json" for k in KINDS))
        self.assertEqual(sorted(p.name for p in EXAMPLES.iterdir()),
                         sorted(f"{k}.json" for k in KINDS))

    def test_each_example_validates(self):
        for kind in KINDS:
            with self.subTest(kind=kind):
                self.assertEqual(validate(example(kind), schema(kind)), [])

    def test_each_schema_rejects_a_broken_example(self):
        for kind in KINDS:
            sch = schema(kind)
            for field in sch["required"]:
                with self.subTest(kind=kind, missing=field):
                    broken = example(kind)
                    broken.pop(field, None)
                    self.assertTrue(validate(broken, sch))
            with self.subTest(kind=kind, extra=True):
                self.assertTrue(validate({**example(kind), "extra": 1}, sch))

    def test_ids_and_paths_are_constrained(self):
        bad = [("signal", "id", "case-2026-1"), ("signal", "at", "2026-03-04 15:20"),
               ("signal", "kind", "complaint"), ("case", "from_signals", []),
               ("approval", "proposal", "Prop-1"), ("measurement", "outcome", "maybe")]
        for kind, field, value in bad:
            with self.subTest(kind=kind, field=field, value=value):
                self.assertTrue(validate({**example(kind), field: value}, schema(kind)))
        for path in ["/etc/passwd", "../decisions.md", "notes/../../x", "a\\b"]:
            with self.subTest(file=path):
                broken = example("proposal")
                broken["changes"][0]["file"] = path
                self.assertTrue(validate(broken, schema("proposal")))
        ok = example("proposal")
        ok["changes"][0]["file"] = "briefs/backtest..md"
        self.assertEqual(validate(ok, schema("proposal")), [])

    def test_schemas_follow_the_doc_tables(self):
        for kind in KINDS:
            with self.subTest(kind=kind):
                table, sch = doc_table(kind), schema(kind)
                self.assertTrue(table)
                self.assertEqual(set(sch["properties"]), set(table))
                self.assertEqual(set(sch["required"]), {f for f, req in table.items() if req})
                self.assertEqual(sch["properties"]["schema"]["const"], f"growth-loop/{kind}/v1")

    def test_doc_examples_equal_the_files(self):
        for kind in KINDS:
            with self.subTest(kind=kind):
                self.assertEqual(doc_example(kind), example(kind))

    def test_examples_obey_the_cross_record_rules(self):
        sig, case, prop, appr, meas = (example(k) for k in KINDS)
        self.assertIn(sig["id"], case["from_signals"])
        self.assertIn(case["id"], prop["cases"])
        self.assertEqual({sig["persona"], case["persona"]}, {prop["persona"]})
        self.assertEqual(appr["proposal"], prop["id"])
        self.assertEqual(meas["proposal"], prop["id"])
        self.assertEqual(meas["approval"], appr["id"])
        self.assertEqual(meas["base"], prop["base"])
        self.assertEqual(appr["role"], APPROVER[prop["persona_kind"]])
        self.assertEqual({r["case"] for r in meas["results"] if r["new"]}, set(prop["cases"]))
        self.assertIn(case["expected"], case["options"])
        self.assertEqual(expected_outcome(meas, appr), meas["outcome"])

    def test_rule_three_reverts_regressions_failures_and_rejections(self):
        meas, appr = example("measurement"), example("approval")
        regressed = copy.deepcopy(meas)
        next(r for r in regressed["results"] if not r["new"] and r["before"] == "pass")["after"] = "fail"
        failing = copy.deepcopy(meas)
        next(r for r in failing["results"] if r["new"])["after"] = "fail"
        self.assertEqual(expected_outcome(meas, appr), "keep")
        self.assertEqual(expected_outcome(regressed, appr), "revert")
        self.assertEqual(expected_outcome(failing, appr), "revert")
        self.assertEqual(expected_outcome(meas, {**appr, "decision": "rejected"}), "revert")


if __name__ == "__main__":
    unittest.main()
