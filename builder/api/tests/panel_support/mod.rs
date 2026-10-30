//! Isolated invented assistants and a Claude Code substitute shared by panel boundary tests.
use persona_builder_api::{agree_consent_in, dto::*, new_assistant_in};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub struct Home {
    pub path: PathBuf,
    pub bin: PathBuf,
}
impl Home {
    pub fn new(consent: bool) -> Self {
        let path = std::env::temp_dir().join(format!(
            "voice-panel-boundary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = path.canonicalize().unwrap();
        for name in ["Pebble", "Quill"] {
            let assistant = new_assistant_in(
                &path,
                NewAssistantRequest {
                    name: name.into(),
                    pronouns: "she/her".into(),
                    principal: "Rowan Vale".into(),
                    principal_pronouns: "they/them".into(),
                },
            )
            .unwrap();
            fs::write(
                path.join(".claude/crew")
                    .join(&assistant.slug)
                    .join("decisions.md"),
                "Prefer a short, reversible trial.",
            )
            .unwrap();
            if consent {
                agree_consent_in(
                    &path,
                    SlugRequest {
                        slug: assistant.slug,
                    },
                )
                .unwrap();
            }
        }
        let bin = path.join("bin");
        fs::create_dir(&bin).unwrap();
        let python = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(
            python.status.success(),
            "python3 is required for the Claude substitute"
        );
        let stub = bin.join("claude");
        fs::write(
            &stub,
            format!(
                "#!{} -I\n{}",
                String::from_utf8(python.stdout).unwrap().trim(),
                include_str!("claude.py")
            ),
        )
        .unwrap();
        fs::set_permissions(stub, fs::Permissions::from_mode(0o755)).unwrap();
        Self { path, bin }
    }
    pub fn request(&self) -> ConvenePanelRequest {
        ConvenePanelRequest {
            question: "Which trial should we try?".into(),
            background: "Keep it easy to undo.".into(),
            kind: "list:1".into(),
            seats: vec!["pebble".into(), "quill".into()],
            time_box: Some("15m".into()),
        }
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
