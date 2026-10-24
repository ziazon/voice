use persona_builder_save::secret::Secret;

#[test]
fn debug_hides_the_value() {
    let secret = Secret::new("invented-token".into());
    assert_eq!(format!("{secret:?}"), "Secret(hidden)");
}

#[test]
fn expose_round_trip() {
    let secret = Secret::new("invented-token._~:+/=-".into());
    assert_eq!(secret.expose(), "invented-token._~:+/=-");
}
