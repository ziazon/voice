/// Each app command and the `persona-builder` commands that do the same thing.
pub struct Parity {
    pub app: &'static str,
    pub cli: &'static [&'static [&'static str]],
}
pub const PARITY: &[Parity] = &[Parity {
    app: "app_info",
    cli: &[&["--version"], &["agents"]],
}];
