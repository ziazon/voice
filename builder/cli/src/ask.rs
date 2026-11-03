//! A single question, read from a file, with only the permissions chosen this time.
use std::path::PathBuf;
use voice_api::{ask::text, dto::AskRequest};
use voice_core::{
    deny::DenyList,
    error::{Error, Result},
};

pub fn run(member_dir: PathBuf, question_file: PathBuf, capabilities: Vec<String>) -> Result<()> {
    let home = crate::home()?;
    let unreadable = |_| Error::Invalid(text::READ_INPUT.into());
    DenyList::new(&home, &[])
        .map_err(unreadable)?
        .check(&question_file)
        .map_err(unreadable)?;
    let question = voice_build::draft::read_text(&question_file, 8 * 1024 * 1024)
        .map_err(unreadable)?
        .ok_or_else(|| Error::Invalid(text::READ_INPUT.into()))?;
    let member = crate::checked_member(&member_dir, &home)?;
    let answer = voice_api::ask(AskRequest {
        slug: member.slug,
        question,
        capabilities,
    })
    .map_err(|failure| Error::Invalid(failure.message))?;
    println!("{}\n\n{}", answer.answer, answer.said);
    Ok(())
}
