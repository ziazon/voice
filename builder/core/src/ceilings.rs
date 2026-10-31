// Tests only in this dispatch. tests/ceilings.rs loads them until this module is exported.
#[cfg(test)]
mod tests {
    use crate::ceilings::{Ceiling, Ceilings, Source};
    use crate::member::Member;
    use serde_json::json;

    #[test]
    fn sources_match_every_row_and_round_trip_only_exact_ids() {
        use Ceiling::{Anthropic, LocalOnly};
        use Source::*;
        let rows = [
            (
                ClaudeHistory,
                "claude-history",
                "your Claude history (your Claude Code sessions and your claude.ai export)",
                Anthropic,
                true,
            ),
            (
                CrewFolder,
                "crew-folder",
                "your assistant's own folder",
                Anthropic,
                true,
            ),
            (
                Typed,
                "typed",
                "what you type into Persona Builder (interview answers, notes on answers, a panel's question)",
                Anthropic,
                true,
            ),
            (
                SkillMirror,
                "skill-mirror",
                "the copy of your skills",
                Anthropic,
                false,
            ),
            (Mail, "mail", "your mail", LocalOnly, false),
            (Calendar, "calendar", "your calendar", LocalOnly, false),
            (Contacts, "contacts", "your contacts", LocalOnly, false),
            (
                FamilySkills,
                "family-skills",
                "your family skills",
                LocalOnly,
                false,
            ),
            (
                LegalSkills,
                "legal-skills",
                "your legal skills",
                LocalOnly,
                false,
            ),
        ];
        assert_eq!(Source::ALL, rows.map(|row| row.0));
        for (source, id, name, default, read_today) in rows {
            assert_eq!(source.id(), id);
            assert_eq!(source.name(), name);
            assert_eq!(source.default_ceiling(), default);
            assert_eq!(source.read_today(), read_today);
            assert_eq!(Source::from_id(id), Some(source));
        }
        for id in ["Mail", "mail ", ""] {
            assert_eq!(Source::from_id(id), None);
        }
    }

    #[test]
    fn ceilings_match_ids_words_and_narrowest_first_order() {
        use Ceiling::*;
        for (ceiling, id, words) in [
            (LocalOnly, "local-only", "on your own machines"),
            (
                Anthropic,
                "anthropic",
                "on your own machines or with Anthropic",
            ),
            (
                AnyConsented,
                "any-consented",
                "with any provider you've agreed to",
            ),
        ] {
            assert_eq!(ceiling.id(), id);
            assert_eq!(ceiling.words(), words);
            assert_eq!(Ceiling::from_id(id), Some(ceiling));
        }
        for id in ["Mail", "mail ", "", "Anthropic", "anthropic "] {
            assert_eq!(Ceiling::from_id(id), None);
        }
        assert!(LocalOnly < Anthropic);
        assert!(Anthropic < AnyConsented);
    }

    fn member() -> Member {
        serde_json::from_str(include_str!("../../../crew/_template/member.json")).unwrap()
    }

    #[test]
    fn an_unset_member_uses_all_defaults_without_personal_settings() {
        let ceilings = Ceilings::for_member(&member()).unwrap();
        assert_eq!(ceilings, Ceilings::defaults());
        for source in Source::ALL {
            assert_eq!(ceilings.get(source), source.default_ceiling());
            assert!(!ceilings.is_set(source));
        }
    }

    #[test]
    fn a_mail_setting_changes_only_mail_and_marks_it_set() {
        let mut member = member();
        member
            .extra
            .insert("ceilings".into(), json!({"mail": "anthropic"}));
        let ceilings = Ceilings::for_member(&member).unwrap();
        for source in Source::ALL {
            assert_eq!(ceilings.is_set(source), source == Source::Mail);
            assert_eq!(
                ceilings.get(source),
                if source == Source::Mail {
                    Ceiling::Anthropic
                } else {
                    source.default_ceiling()
                }
            );
        }
    }
}
