pub mod bitbucket;
pub mod github;
pub mod gitlab;
mod token;

use git2::Repository;
use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Github,
    Bitbucket,
    Gitlab,
}

impl Provider {
    fn parse(s: &str) -> Result<Provider, String> {
        match s {
            "github" => Ok(Provider::Github),
            "bitbucket" => Ok(Provider::Bitbucket),
            "gitlab" => Ok(Provider::Gitlab),
            other => Err(format!("Proveedor desconocido: {other}")),
        }
    }

    fn token_account(self) -> &'static str {
        match self {
            Provider::Github => "github_token",
            Provider::Bitbucket => "bitbucket_token",
            Provider::Gitlab => "gitlab_token",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Provider::Github => "GitHub",
            Provider::Bitbucket => "Bitbucket",
            Provider::Gitlab => "GitLab",
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ConnectedUser {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct PullRequestSummary {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub state: String,
    pub draft: bool,
    pub base: String,
    pub head: String,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct PullRequestDetail {
    #[serde(flatten)]
    pub summary: PullRequestSummary,
    pub body: String,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub mergeable_state: String,
    pub additions: i64,
    pub deletions: i64,
    pub changed_files: i64,
    pub commits: i64,
}

struct RemoteRef {
    provider: Provider,
    owner: String,
    repo: String,
}

/// Detects which provider (if any) a remote URL belongs to and extracts
/// its owner/repo (GitHub, GitLab) or workspace/repo_slug (Bitbucket) —
/// same shape either way, so the rest of the dispatch code doesn't need
/// to care which one it's talking to.
fn parse_remote(url: &str) -> Option<RemoteRef> {
    if let Some((owner, repo)) = github::parse_remote(url) {
        return Some(RemoteRef { provider: Provider::Github, owner, repo });
    }
    if let Some((owner, repo)) = bitbucket::parse_remote(url) {
        return Some(RemoteRef { provider: Provider::Bitbucket, owner, repo });
    }
    if let Some((owner, repo)) = gitlab::parse_remote(url) {
        return Some(RemoteRef { provider: Provider::Gitlab, owner, repo });
    }
    None
}

fn resolve(path: &str) -> Result<RemoteRef, String> {
    let repo = Repository::open(path).map_err(crate::git::err_msg)?;
    let remote = repo
        .find_remote("origin")
        .map_err(|_| "El repositorio no tiene un remoto 'origin'".to_string())?;
    let url = remote.url().map_err(crate::git::err_msg)?;
    parse_remote(url)
        .ok_or_else(|| "El remoto 'origin' no es de un proveedor soportado (GitHub, GitLab o Bitbucket)".to_string())
}

#[derive(Serialize)]
pub struct ProviderStatus {
    provider: Option<Provider>,
    provider_label: Option<String>,
    has_token: bool,
    owner: Option<String>,
    repo: Option<String>,
}

#[tauri::command]
pub fn provider_status(path: String) -> ProviderStatus {
    match resolve(&path) {
        Ok(r) => ProviderStatus {
            provider: Some(r.provider),
            provider_label: Some(r.provider.label().to_string()),
            has_token: token::get_token(r.provider).is_some(),
            owner: Some(r.owner),
            repo: Some(r.repo),
        },
        Err(_) => ProviderStatus { provider: None, provider_label: None, has_token: false, owner: None, repo: None },
    }
}

/// Verifies the credential against the provider's own API and only
/// persists it once confirmed valid, so a typo never silently "connects"
/// to nothing. GitHub and GitLab take a bearer `token`; Bitbucket takes a
/// `username` + app-password `secret` pair (HTTP Basic auth).
#[tauri::command]
pub async fn provider_connect(
    provider: String,
    token: Option<String>,
    username: Option<String>,
    secret: Option<String>,
) -> Result<ConnectedUser, String> {
    let provider = Provider::parse(&provider)?;
    match provider {
        Provider::Github => {
            let token = token.filter(|t| !t.is_empty()).ok_or("Falta el token")?;
            let user = github::fetch_user(github::API_BASE, &token).await?;
            self::token::set_token(provider, &token)?;
            Ok(user)
        }
        Provider::Gitlab => {
            let token = token.filter(|t| !t.is_empty()).ok_or("Falta el token")?;
            let user = gitlab::fetch_user(gitlab::API_BASE, &token).await?;
            self::token::set_token(provider, &token)?;
            Ok(user)
        }
        Provider::Bitbucket => {
            let username = username.filter(|u| !u.is_empty()).ok_or("Falta el usuario")?;
            let secret = secret.filter(|s| !s.is_empty()).ok_or("Falta el app password")?;
            let credential = bitbucket::Credential { username: username.clone(), secret: secret.clone() };
            let user = bitbucket::fetch_user(bitbucket::API_BASE, &credential).await?;
            self::token::set_token(provider, &bitbucket::encode_credential(&username, &secret))?;
            Ok(user)
        }
    }
}

#[tauri::command]
pub fn provider_disconnect(provider: String) -> Result<(), String> {
    self::token::clear_token(Provider::parse(&provider)?)
}

fn require_bitbucket_credential(provider: Provider) -> Result<bitbucket::Credential, String> {
    let stored = self::token::get_token(provider).ok_or_else(|| format!("Conecta tu cuenta de {} primero", provider.label()))?;
    bitbucket::decode_credential(&stored).ok_or_else(|| "Credencial de Bitbucket inválida; reconecta tu cuenta".to_string())
}

fn require_bearer_token(provider: Provider) -> Result<String, String> {
    self::token::get_token(provider).ok_or_else(|| format!("Conecta tu cuenta de {} primero", provider.label()))
}

#[tauri::command]
pub async fn provider_list_pull_requests(path: String, state: String) -> Result<Vec<PullRequestSummary>, String> {
    let r = resolve(&path)?;
    match r.provider {
        Provider::Github => {
            let token = require_bearer_token(r.provider)?;
            github::list_pull_requests(github::API_BASE, &token, &r.owner, &r.repo, &state).await
        }
        Provider::Gitlab => {
            let token = require_bearer_token(r.provider)?;
            gitlab::list_pull_requests(gitlab::API_BASE, &token, &r.owner, &r.repo, &state).await
        }
        Provider::Bitbucket => {
            let credential = require_bitbucket_credential(r.provider)?;
            bitbucket::list_pull_requests(bitbucket::API_BASE, &credential, &r.owner, &r.repo, &state).await
        }
    }
}

#[tauri::command]
pub async fn provider_get_pull_request(path: String, number: u64) -> Result<PullRequestDetail, String> {
    let r = resolve(&path)?;
    match r.provider {
        Provider::Github => {
            let token = require_bearer_token(r.provider)?;
            github::get_pull_request(github::API_BASE, &token, &r.owner, &r.repo, number).await
        }
        Provider::Gitlab => {
            let token = require_bearer_token(r.provider)?;
            gitlab::get_pull_request(gitlab::API_BASE, &token, &r.owner, &r.repo, number).await
        }
        Provider::Bitbucket => {
            let credential = require_bitbucket_credential(r.provider)?;
            bitbucket::get_pull_request(bitbucket::API_BASE, &credential, &r.owner, &r.repo, number).await
        }
    }
}

#[tauri::command]
pub async fn provider_create_pull_request(
    path: String,
    title: String,
    body: String,
    head: String,
    base: String,
    draft: bool,
) -> Result<PullRequestDetail, String> {
    let r = resolve(&path)?;
    match r.provider {
        Provider::Github => {
            let token = require_bearer_token(r.provider)?;
            github::create_pull_request(github::API_BASE, &token, &r.owner, &r.repo, &title, &body, &head, &base, draft).await
        }
        Provider::Gitlab => {
            let token = require_bearer_token(r.provider)?;
            gitlab::create_pull_request(gitlab::API_BASE, &token, &r.owner, &r.repo, &title, &body, &head, &base, draft).await
        }
        Provider::Bitbucket => {
            let credential = require_bitbucket_credential(r.provider)?;
            bitbucket::create_pull_request(bitbucket::API_BASE, &credential, &r.owner, &r.repo, &title, &body, &head, &base, draft)
                .await
        }
    }
}

#[tauri::command]
pub async fn provider_merge_pull_request(path: String, number: u64, method: String) -> Result<(), String> {
    let r = resolve(&path)?;
    match r.provider {
        Provider::Github => {
            let token = require_bearer_token(r.provider)?;
            github::merge_pull_request(github::API_BASE, &token, &r.owner, &r.repo, number, &method).await
        }
        Provider::Gitlab => {
            let token = require_bearer_token(r.provider)?;
            gitlab::merge_pull_request(gitlab::API_BASE, &token, &r.owner, &r.repo, number, &method).await
        }
        Provider::Bitbucket => {
            let credential = require_bitbucket_credential(r.provider)?;
            bitbucket::merge_pull_request(bitbucket::API_BASE, &credential, &r.owner, &r.repo, number, &method).await
        }
    }
}
