pub mod api;
mod remote;
mod token;

const API_BASE: &str = "https://api.github.com";

fn resolve_owner_repo(path: &str) -> Result<(String, String), String> {
    let repo = git2::Repository::open(path).map_err(crate::git::err_msg)?;
    let remote = repo
        .find_remote("origin")
        .map_err(|_| "El repositorio no tiene un remoto 'origin'".to_string())?;
    let url = remote.url().map_err(crate::git::err_msg)?;
    remote::parse_github_remote(url).ok_or_else(|| "El remoto 'origin' no es un repositorio de GitHub".to_string())
}

fn require_token() -> Result<String, String> {
    token::get_token().ok_or_else(|| "Conecta tu cuenta de GitHub primero".to_string())
}

#[derive(serde::Serialize)]
pub struct GithubRepoStatus {
    has_token: bool,
    owner: Option<String>,
    repo: Option<String>,
}

#[tauri::command]
pub fn github_status(path: String) -> GithubRepoStatus {
    let (owner, repo) = resolve_owner_repo(&path).map(|(o, r)| (Some(o), Some(r))).unwrap_or((None, None));
    GithubRepoStatus { has_token: token::get_token().is_some(), owner, repo }
}

/// Verifies the token against GitHub's API and only persists it once
/// confirmed valid, so a typo never silently "connects" the app to nothing.
#[tauri::command]
pub async fn github_connect(token: String) -> Result<api::GithubUser, String> {
    let user = api::fetch_user(API_BASE, &token).await?;
    token::set_token(&token)?;
    Ok(user)
}

#[tauri::command]
pub fn github_disconnect() -> Result<(), String> {
    token::clear_token()
}

#[tauri::command]
pub async fn github_list_pull_requests(path: String, state: String) -> Result<Vec<api::PullRequestSummary>, String> {
    let (owner, repo) = resolve_owner_repo(&path)?;
    let token = require_token()?;
    api::list_pull_requests(API_BASE, &token, &owner, &repo, &state).await
}

#[tauri::command]
pub async fn github_get_pull_request(path: String, number: u64) -> Result<api::PullRequestDetail, String> {
    let (owner, repo) = resolve_owner_repo(&path)?;
    let token = require_token()?;
    api::get_pull_request(API_BASE, &token, &owner, &repo, number).await
}

#[tauri::command]
pub async fn github_create_pull_request(
    path: String,
    title: String,
    body: String,
    head: String,
    base: String,
    draft: bool,
) -> Result<api::PullRequestDetail, String> {
    let (owner, repo) = resolve_owner_repo(&path)?;
    let token = require_token()?;
    api::create_pull_request(API_BASE, &token, &owner, &repo, &title, &body, &head, &base, draft).await
}

#[tauri::command]
pub async fn github_merge_pull_request(path: String, number: u64, method: String) -> Result<(), String> {
    let (owner, repo) = resolve_owner_repo(&path)?;
    let token = require_token()?;
    api::merge_pull_request(API_BASE, &token, &owner, &repo, number, &method).await
}
