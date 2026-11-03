//! Dated read permissions, the chosen calendar export, and their content-free log.
use crate::{
    assistants::home,
    calendar,
    dto::*,
    history::{self, failure},
};
use std::{fs, path::Path, time::SystemTime};
use voice_agent::{
    grants::{self, Capability, Window},
    text,
};
use voice_core::error::Error;
use voice_ingest::private;

type Result<T> = std::result::Result<T, Failure>;
pub const UNKNOWN: &str = "Voice can't be given that permission.";
pub const DAMAGED: &str = "Voice's permissions file for this assistant is damaged, so it reads nothing that needs your permission. To see where it is, run voice grants show in a terminal, delete it, then give permission again.";
pub const PERMISSION_SAVE: &str =
    "Voice couldn't save this. Check there's space on this computer, then try again.";
pub const AUDIT_READ: &str = "Voice couldn't read this assistant's log. Try again.";

fn clean_calendar(mut export: CalendarExport) -> CalendarExport {
    export.name = text::typed_name(&export.name);
    export
}

pub fn list_permissions_in(home: &Path, request: SlugRequest) -> Result<Permissions> {
    calendar::member_in(home, &request.slug)?;
    let grants = grants::load(home, &request.slug).map_err(|_| failure(DAMAGED))?;
    let permissions = Capability::ALL
        .into_iter()
        .map(|capability| {
            let grant = grants.iter().find(|grant| grant.capability == capability);
            let window = grant.map_or(Window::DEFAULT, |grant| grant.window);
            Permission {
                id: capability.id().into(),
                words: capability.words().into(),
                on: grant.is_some(),
                given_on: grant.map(|grant| grant.given_on.clone()),
                past_days: window.past_days(),
                next_days: window.next_days(),
                window: grant.map(|grant| text::grant_window(grant.window)),
            }
        })
        .collect();
    let (calendar, calendar_problem) = match calendar::calendar_export_in(home, &request.slug) {
        Ok(calendar) => (calendar.map(clean_calendar), None),
        Err(f) if f.message == calendar::GONE || f.message == calendar::BAD => {
            (None, Some(f.message))
        }
        Err(f) => return Err(f),
    };
    Ok(Permissions {
        permissions,
        calendar,
        calendar_problem,
    })
}

fn lock(home: &Path, slug: &str) -> Result<private::StateLock> {
    private::lock_existing_state(home, slug).map_err(|error| match error {
        Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            failure(text::NO_STATE_FOR_GRANTS)
        }
        Error::Invalid(text) if text == private::BUSY => failure(history::BUSY),
        _ => failure(PERMISSION_SAVE),
    })
}

pub fn give_permission_in(
    home: &Path,
    request: GivePermissionRequest,
) -> Result<PermissionsChanged> {
    let member = calendar::member_in(home, &request.slug)?;
    let capability = Capability::from_id(&request.id).ok_or_else(|| failure(UNKNOWN))?;
    let window = Window::new(request.past_days, request.next_days)
        .map_err(|_| failure(text::WINDOW_RANGE))?;
    let _lock = lock(home, &request.slug)?;
    grants::load(home, &request.slug).map_err(|_| failure(DAMAGED))?;
    let grant = grants::give(home, &request.slug, capability, window, SystemTime::now()).map_err(
        |error| match error {
            Error::Invalid(text) if text == text::GRANT_NOT_LOGGED => failure(&text),
            _ => failure(PERMISSION_SAVE),
        },
    )?;
    Ok(PermissionsChanged {
        said: text::grant_given(&member.name, &grant),
        permissions: list_permissions_in(home, SlugRequest { slug: request.slug })?,
    })
}

pub fn turn_off_permission_in(
    home: &Path,
    request: PermissionRequest,
) -> Result<PermissionsChanged> {
    let member = calendar::member_in(home, &request.slug)?;
    let capability = Capability::from_id(&request.id).ok_or_else(|| failure(UNKNOWN))?;
    let _lock = lock(home, &request.slug)?;
    grants::load(home, &request.slug).map_err(|_| failure(DAMAGED))?;
    let changed = grants::revoke(home, &request.slug, capability, SystemTime::now())
        .map_err(|_| failure(PERMISSION_SAVE))?;
    Ok(PermissionsChanged {
        said: text::grant_revoked(&member.name, capability, changed),
        permissions: list_permissions_in(home, SlugRequest { slug: request.slug })?,
    })
}

pub fn check_calendar_choice_in(home: &Path, request: SlugRequest) -> Result<()> {
    calendar::member_in(home, &request.slug)?;
    match fs::symlink_metadata(home.join(".claude/state").join(&request.slug)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(failure(calendar::NO_STATE))
        }
        _ => Ok(()),
    }
}

pub fn use_calendar_export_in(home: &Path, slug: &str, path: &Path) -> Result<CalendarExport> {
    calendar::choose_calendar_export_in(home, slug, path).map(clean_calendar)
}

pub fn audit_log_in(home: &Path, request: AuditRequest) -> Result<AuditLog> {
    calendar::member_in(home, &request.slug)?;
    if !(1..=1000).contains(&request.last) {
        return Err(failure(text::AUDIT_LAST));
    }
    let audit = grants::audit(home, &request.slug, request.last as usize)
        .map_err(|_| failure(AUDIT_READ))?;
    Ok(AuditLog {
        lines: audit.entries.iter().map(text::audit_entry).collect(),
        unreadable: history::count(audit.unreadable),
        older: audit.older,
    })
}

pub fn list_permissions(request: SlugRequest) -> Result<Permissions> {
    list_permissions_in(&home(history::FIND)?, request)
}
pub fn give_permission(request: GivePermissionRequest) -> Result<PermissionsChanged> {
    give_permission_in(&home(PERMISSION_SAVE)?, request)
}
pub fn turn_off_permission(request: PermissionRequest) -> Result<PermissionsChanged> {
    turn_off_permission_in(&home(PERMISSION_SAVE)?, request)
}
pub fn check_calendar_choice(request: SlugRequest) -> Result<()> {
    check_calendar_choice_in(&home(history::FIND)?, request)
}
pub fn use_calendar_export(slug: &str, path: &Path) -> Result<CalendarExport> {
    use_calendar_export_in(&home(history::FIND)?, slug, path)
}
pub fn audit_log(request: AuditRequest) -> Result<AuditLog> {
    audit_log_in(&home(AUDIT_READ)?, request)
}
