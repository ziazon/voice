use persona_builder_agent::text;

#[test]
fn the_wording_says_this_computer_never_this_mac() {
    let source = include_str!("../src/text.rs");
    assert!(!source.contains("this Mac"), "a line still says this Mac");
    assert!(!source.contains("your Mac"), "a line still says your Mac");
    for line in [text::NOT_INSTALLED, text::DOES_NOT_RUN, text::API_KEY_SET] {
        assert!(line.contains("this computer"), "{line}");
    }
}
