mod common;
use std::{collections::BTreeMap, fs, path::Path};
#[test]
fn template_embedded_matches_every_file_and_byte() {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/");
                if relative != "README.md" {
                    out.insert(relative, fs::read(&path).unwrap());
                }
            }
        }
    }
    let base = common::repo().join("crew/_template");
    let mut expected = BTreeMap::new();
    walk(&base, &base, &mut expected);
    let actual = persona_builder::template::FILES;
    assert_eq!(
        actual.iter().map(|(path, _)| *path).collect::<Vec<_>>(),
        expected.keys().map(String::as_str).collect::<Vec<_>>(),
        "embedded paths must match the template, sorted and without duplicates"
    );
    for (path, text) in actual {
        assert_eq!(
            text.as_bytes(),
            expected[*path],
            "embedded bytes differ: {path}"
        );
    }
}
