use persona_builder_grow::version::version;
#[test]
fn git_blobs() {
    assert_eq!(version(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    assert_eq!(
        version(b"hello\n"),
        "ce013625030ba8dba906f756967f9e9ca394464a"
    );
}
