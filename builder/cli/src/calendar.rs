use clap::Subcommand;
use std::path::PathBuf;
use voice_agent::{grants, text};
use voice_api::{
    calendar,
    dto::{CalendarEvent, EventStatus, SlugRequest},
};
use voice_core::error::{Error, Result};

#[derive(Subcommand)]
pub enum CalendarCommand {
    /// Choose the calendar file (or folder of them) you exported
    Choose { member_dir: PathBuf, path: PathBuf },
    /// Show what's on your calendar in the window you allowed
    Show { member_dir: PathBuf },
}

pub fn run(command: CalendarCommand) -> Result<()> {
    let member_dir = match &command {
        CalendarCommand::Choose { member_dir, .. } | CalendarCommand::Show { member_dir } => {
            member_dir
        }
    };
    let (member, home) = super::member_and_home(member_dir)?;
    match command {
        CalendarCommand::Choose { path, .. } => {
            let chosen = calendar::choose_calendar_export_in(&home, &member.slug, &path)
                .map_err(|error| Error::Invalid(error.message))?;
            println!(
                "Voice will read your calendar for {} from {}.",
                member.name,
                text::typed_name(&chosen.name)
            );
        }
        CalendarCommand::Show { .. } => {
            let answer = calendar::read_calendar(
                SlugRequest {
                    slug: member.slug.clone(),
                },
                "cli",
            )
            .map_err(|error| Error::Invalid(error.message))?;
            let grant = grants::load(&home, &member.slug)
                .map_err(|_| Error::Invalid(calendar::BAD_GRANTS.into()))?
                .into_iter()
                .find(|grant| grant.capability == grants::Capability::CalendarRead)
                .ok_or_else(|| Error::Invalid(text::no_grant(grants::Capability::CalendarRead)))?;
            for event in &answer.events {
                println!("{}", event_line(event));
            }
            let count = match answer.events.len() {
                0 => "No events".into(),
                1 => "1 event".into(),
                n => format!("{n} events"),
            };
            println!("{count} {}.", text::grant_window(grant.window));
            match answer.private_left_out {
                0 => {}
                1 => println!("1 left out because it looks private."),
                n => println!("{n} left out because they look private."),
            }
            if answer.not_understood > 0 {
                println!(
                    "{} couldn't be understood, so Voice left them out.",
                    answer.not_understood
                );
            }
            if answer.more {
                println!("Voice shows the first 2,000.");
            }
        }
    }
    Ok(())
}

// The broker has checked the reader's dates, times and one-line text before any are printed.
fn event_line(event: &CalendarEvent) -> String {
    let when = if event.all_day {
        if event.start == event.end {
            format!("{}  all day", event.start)
        } else {
            format!("{} to {}  all day", event.start, event.end)
        }
    } else {
        let (start_day, start_time) = event.start.split_once('T').expect("checked calendar start");
        let (end_day, end_time) = event.end.split_once('T').expect("checked calendar end");
        if start_day == end_day {
            format!("{start_day} {start_time}–{end_time}")
        } else {
            format!("{start_day} {start_time} to {end_day} {end_time}")
        }
    };
    let title = if event.title.is_empty() {
        "(no title)"
    } else {
        &event.title
    };
    let place = event
        .place
        .as_ref()
        .map(|place| format!(", {place}"))
        .unwrap_or_default();
    let status = match event.status {
        Some(EventStatus::Tentative) => " (tentative)",
        Some(EventStatus::Cancelled) => " (cancelled)",
        Some(EventStatus::Confirmed) | None => "",
    };
    format!("{when}  {title}{place}{status}")
}
