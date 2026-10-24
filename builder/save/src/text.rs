pub const CHOICE_GITHUB: &str = "Save it to a private GitHub repo";
pub const CHOICE_LOCAL: &str = "Keep it on this Mac only";
pub fn choice_existing(shown: &str) -> String {
    format!("Save it in {shown}, with the rest of your AI setup")
}
pub const NO_GITHUB_ACCOUNT: &str = "No GitHub account? A free one keeps a backup of your assistant: https://github.com/signup. Or keep it on this Mac only.";
pub fn sign_in_prompt(uri: &str, code: &str) -> String {
    format!("Go to {uri} and enter this code: {code}")
}
pub const SIGN_IN_WAITING: &str = "Waiting for you to approve Persona Builder on GitHub…";
pub fn signed_in(login: &str) -> String {
    format!("Signed in to GitHub as {login}.")
}
pub const SIGNED_OUT: &str = "Signed out of GitHub on this Mac. To remove Persona Builder's access completely, open Settings on GitHub, then Applications, then Authorized OAuth Apps.";
pub const NOT_SET_UP: &str = "GitHub sign-in isn't set up in this copy of Persona Builder yet. You can keep your assistant on this Mac only for now.";
pub const SIGN_IN_CANCELLED: &str = "GitHub sign-in was cancelled. Nothing was changed.";
pub const SIGN_IN_EXPIRED: &str =
    "The GitHub code ran out before it was used. Start signing in again for a new code.";
pub fn sign_in_failed(code: &str) -> String {
    format!(
        "GitHub sign-in didn't work (GitHub said: {code}). Try again later, or keep your assistant on this Mac only."
    )
}
pub const SIGN_IN_FIRST: &str = "Sign in to GitHub first.";
pub const NO_REPO_PERMISSION: &str = "GitHub didn't give Persona Builder permission to make private repos, so it can't save there. Sign in again and allow it, or keep your assistant on this Mac only.";
pub fn repo_public(full: &str) -> String {
    format!(
        "{full} is public, so anyone could read your assistant. Persona Builder won't save it there. Make the repo private on GitHub, or keep your assistant on this Mac only."
    )
}
pub fn repo_unconfirmed(full: &str) -> String {
    format!(
        "Persona Builder couldn't confirm that {full} is private, so it didn't save there. Check that you're signed in to the GitHub account that owns it."
    )
}
pub const NOT_ON_GITHUB: &str = "This repo isn't on GitHub. Persona Builder can only check that GitHub repos are private, so it won't save there. Keep your assistant on this Mac only, or save it to a private GitHub repo.";
pub fn repo_name_taken(name: &str) -> String {
    format!(
        "GitHub wouldn't make a repo called {name}. You may already have one with that name: pick another."
    )
}
pub const BAD_REPO_NAME: &str =
    "A repo name can use letters, numbers, dots, dashes and underscores, up to 100 characters.";
pub const NOT_OFFERED: &str =
    "Persona Builder can save only to a GitHub repo it found around your AI setup.";
pub const CHOOSE_FIRST: &str = "Choose where to save your assistant first.";
pub const KEPT_LOCAL: &str = "Your assistant is kept on this Mac only, so there's nothing to send.";
pub fn privacy_hits(places: &str) -> String {
    format!("The privacy check found something private, so nothing was sent. Look at: {places}")
}
pub const PRIVACY_NOT_BUILT: &str =
    "This copy of Persona Builder can't run the privacy check yet, so nothing was sent.";
pub fn links_in_folder(rel: &str) -> String {
    format!("Persona Builder won't save a folder that holds links to other places: {rel}")
}
pub fn saved_github(full: &str) -> String {
    format!("Saved to {full} on GitHub.")
}
pub fn saved_existing(shown: &str, branch: &str) -> String {
    format!("Saved to {shown}, on the branch {branch}.")
}
pub const UP_TO_DATE: &str = "Nothing had changed, so the saved copy was already up to date.";
pub const NEEDS_GIT: &str = "Persona Builder needs Git, which this Mac doesn't have yet. To get it, open the Terminal app and type: xcode-select --install";
pub const GIT_TOO_OLD: &str =
    "Persona Builder needs Git 2.32 or later. Update Git, then try again.";
pub fn push_failed(detail: &str) -> String {
    format!("GitHub didn't take the save: {detail}")
}
pub const MAC_ONLY: &str = "Saving to GitHub works only on a Mac for now.";
pub const KEYCHAIN_FAILED: &str =
    "Persona Builder couldn't use the Keychain on this Mac, so it didn't keep your GitHub sign-in.";
pub const GITHUB_UNREACHABLE: &str =
    "Persona Builder couldn't reach GitHub. Check the internet connection, then try again.";
pub const CHOSE_LOCAL: &str = "Your assistant will be kept on this Mac only.";
pub fn chose_existing(shown: &str) -> String {
    format!("Your assistant will be saved in {shown}.")
}
pub fn chose_github(full: &str) -> String {
    format!("Your assistant will be saved to {full} on GitHub, a private repo.")
}

pub fn branch_in_use(branch: &str) -> String {
    format!(
        "The branch {branch} is open in your repo right now, so Persona Builder didn't save. Switch to another branch, then try again."
    )
}
