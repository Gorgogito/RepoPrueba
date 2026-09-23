use super::{ConnectedUser, PullRequestDetail, PullRequestSummary};
use serde::{de::DeserializeOwned, Deserialize};

pub const API_BASE: &str = "https://api.bitbucket.org/2.0";

/// Extracts `(workspace, repo_slug)` from a Bitbucket remote URL, mirroring
/// [`super::github::parse_remote`] for the forms git actually produces.
pub fn parse_remote(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");

    let after_host = trimmed
        .strip_prefix("git@bitbucket.org:")
        .or_else(|| trimmed.strip_prefix("ssh://git@bitbucket.org/"))
        .or_else(|| trimmed.strip_prefix("https://bitbucket.org/"))
        .or_else(|| trimmed.strip_prefix("http://bitbucket.org/"))?;

    let mut parts = after_host.splitn(2, '/');
    let workspace = parts.next()?;
    let repo_slug = parts.next()?;
    if workspace.is_empty() || repo_slug.is_empty() {
        return None;
    }
    Some((workspace.to_string(), repo_slug.to_string()))
}

/// Bitbucket app passwords are used over HTTP Basic auth, not a bearer
/// token — the OS credential store only holds one opaque string per
/// provider, so we pack/unpack the pair ourselves.
pub struct Credential {
    pub username: String,
    pub secret: String,
}

pub fn encode_credential(username: &str, secret: &str) -> String {
    format!("{username}\n{secret}")
}

pub fn decode_credential(stored: &str) -> Option<Credential> {
    let mut parts = stored.splitn(2, '\n');
    let username = parts.next()?.to_string();
    let secret = parts.next()?.to_string();
    if username.is_empty() || secret.is_empty() {
        return None;
    }
    Some(Credential { username, secret })
}

#[derive(Deserialize)]
struct RawLink {
    href: String,
}

#[derive(Deserialize)]
struct RawUser {
    display_name: String,
    #[serde(default)]
    links: RawUserLinks,
}

#[derive(Deserialize, Default)]
struct RawUserLinks {
    avatar: Option<RawLink>,
}

#[derive(Deserialize)]
struct RawBranch {
    name: String,
}

#[derive(Deserialize)]
struct RawBranchRef {
    branch: RawBranch,
}

#[derive(Deserialize)]
struct RawPrLinks {
    html: RawLink,
}

#[derive(Deserialize)]
struct RawPullRequest {
    id: u64,
    title: String,
    state: String,
    #[serde(default)]
    draft: bool,
    author: RawUser,
    source: RawBranchRef,
    destination: RawBranchRef,
    links: RawPrLinks,
    created_on: String,
    updated_on: String,
    description: Option<String>,
}

impl RawPullRequest {
    fn into_summary(self) -> PullRequestSummary {
        PullRequestSummary {
            number: self.id,
            title: self.title,
            author: self.author.display_name,
            // Bitbucket uses OPEN/MERGED/DECLINED/SUPERSEDED; normalize to
            // GitHub's lowercase open/closed so the frontend has one shape
            // to render regardless of provider.
            state: if self.state == "OPEN" { "open".to_string() } else { "closed".to_string() },
            draft: self.draft,
            base: self.destination.branch.name,
            head: self.source.branch.name,
            html_url: self.links.html.href,
            created_at: self.created_on,
            updated_at: self.updated_on,
        }
    }

    fn into_detail(self) -> PullRequestDetail {
        let merged = self.state == "MERGED";
        PullRequestDetail {
            body: self.description.clone().unwrap_or_default(),
            merged,
            // Bitbucket's PR object doesn't expose a simple mergeable
            // flag or diff stats the way GitHub's does; left unknown
            // rather than guessed.
            mergeable: None,
            mergeable_state: String::new(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            commits: 0,
            summary: self.into_summary(),
        }
    }
}

#[derive(Deserialize)]
struct RawPage<T> {
    values: Vec<T>,
}

fn auth(builder: reqwest::RequestBuilder, credential: &Credential) -> reqwest::RequestBuilder {
    builder.basic_auth(&credential.username, Some(&credential.secret))
}

/// Parses a Bitbucket API response, surfacing the API's own error message
/// (`error.message`) instead of a bare status code.
async fn handle_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, String> {
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;

    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| format!("Respuesta inesperada de Bitbucket: {e}"))
    } else {
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error")?.get("message")?.as_str().map(|s| s.to_string()))
            .unwrap_or(text);
        Err(format!("Bitbucket respondió {status}: {message}"))
    }
}

pub async fn fetch_user(base_url: &str, credential: &Credential) -> Result<ConnectedUser, String> {
    let client = reqwest::Client::new();
    let resp = auth(client.get(format!("{base_url}/user")), credential).send().await.map_err(|e| e.to_string())?;
    let raw: RawUser = handle_response(resp).await?;
    Ok(ConnectedUser { login: raw.display_name, avatar_url: raw.links.avatar.map(|a| a.href).unwrap_or_default() })
}

fn state_filter_values(state: &str) -> Vec<&'static str> {
    match state {
        "open" => vec!["OPEN"],
        "closed" => vec!["MERGED", "DECLINED", "SUPERSEDED"],
        _ => vec![],
    }
}

pub async fn list_pull_requests(
    base_url: &str,
    credential: &Credential,
    workspace: &str,
    repo_slug: &str,
    state: &str,
) -> Result<Vec<PullRequestSummary>, String> {
    let client = reqwest::Client::new();
    let mut query: Vec<(&str, &str)> = state_filter_values(state).into_iter().map(|s| ("state", s)).collect();
    query.push(("pagelen", "50"));

    let resp = auth(
        client.get(format!("{base_url}/repositories/{workspace}/{repo_slug}/pullrequests")).query(&query),
        credential,
    )
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let raw: RawPage<RawPullRequest> = handle_response(resp).await?;
    Ok(raw.values.into_iter().map(RawPullRequest::into_summary).collect())
}

pub async fn get_pull_request(
    base_url: &str,
    credential: &Credential,
    workspace: &str,
    repo_slug: &str,
    number: u64,
) -> Result<PullRequestDetail, String> {
    let client = reqwest::Client::new();
    let resp = auth(
        client.get(format!("{base_url}/repositories/{workspace}/{repo_slug}/pullrequests/{number}")),
        credential,
    )
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let raw: RawPullRequest = handle_response(resp).await?;
    Ok(raw.into_detail())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_pull_request(
    base_url: &str,
    credential: &Credential,
    workspace: &str,
    repo_slug: &str,
    title: &str,
    body: &str,
    head: &str,
    base: &str,
    draft: bool,
) -> Result<PullRequestDetail, String> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "title": title,
        "description": body,
        "source": { "branch": { "name": head } },
        "destination": { "branch": { "name": base } },
        "draft": draft,
    });
    let resp = auth(client.post(format!("{base_url}/repositories/{workspace}/{repo_slug}/pullrequests")), credential)
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw: RawPullRequest = handle_response(resp).await?;
    Ok(raw.into_detail())
}

/// Bitbucket only has merge_commit/squash/fast_forward strategies (no
/// separate "rebase" concept) — the closest equivalent to our rebase
/// option is a fast-forward merge, since neither produces a merge commit.
fn merge_strategy(method: &str) -> &'static str {
    match method {
        "squash" => "squash",
        "rebase" => "fast_forward",
        _ => "merge_commit",
    }
}

pub async fn merge_pull_request(
    base_url: &str,
    credential: &Credential,
    workspace: &str,
    repo_slug: &str,
    number: u64,
    method: &str,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({ "merge_strategy": merge_strategy(method) });
    let resp = auth(
        client.post(format!("{base_url}/repositories/{workspace}/{repo_slug}/pullrequests/{number}/merge")),
        credential,
    )
    .json(&payload)
    .send()
    .await
    .map_err(|e| e.to_string())?;

    let status = resp.status();
    if status.is_success() {
        Ok(())
    } else {
        let text = resp.text().await.map_err(|e| e.to_string())?;
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error")?.get("message")?.as_str().map(|s| s.to_string()))
            .unwrap_or(text);
        Err(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_credential() -> Credential {
        Credential { username: "user".to_string(), secret: "app-password".to_string() }
    }

    fn sample_pr_json(id: u64, state: &str) -> serde_json::Value {
        json!({
            "id": id,
            "title": "Add feature",
            "state": state,
            "draft": false,
            "author": { "display_name": "Jane Doe", "links": { "avatar": { "href": "https://example.invalid/a.png" } } },
            "source": { "branch": { "name": "feature-x" } },
            "destination": { "branch": { "name": "main" } },
            "links": { "html": { "href": "https://bitbucket.org/team/repo/pull-requests/1" } },
            "created_on": "2026-01-01T00:00:00.000000+00:00",
            "updated_on": "2026-01-02T00:00:00.000000+00:00",
            "description": "Description here",
        })
    }

    #[test]
    fn recognizes_common_bitbucket_remote_forms() {
        assert_eq!(parse_remote("https://bitbucket.org/team/repo.git"), Some(("team".to_string(), "repo".to_string())));
        assert_eq!(parse_remote("git@bitbucket.org:team/repo.git"), Some(("team".to_string(), "repo".to_string())));
        assert_eq!(
            parse_remote("ssh://git@bitbucket.org/team/repo.git"),
            Some(("team".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn rejects_non_bitbucket_urls() {
        assert_eq!(parse_remote("https://github.com/team/repo.git"), None);
    }

    #[test]
    fn credential_round_trips_through_encoding() {
        let encoded = encode_credential("user", "s3cr3t");
        let decoded = decode_credential(&encoded).unwrap();
        assert_eq!(decoded.username, "user");
        assert_eq!(decoded.secret, "s3cr3t");
    }

    #[tokio::test]
    async fn fetch_user_parses_display_name_and_avatar() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/user")
            .with_status(200)
            .with_body(
                json!({ "display_name": "Jane Doe", "links": { "avatar": { "href": "https://example.invalid/a.png" } } })
                    .to_string(),
            )
            .create_async()
            .await;

        let user = fetch_user(&server.url(), &test_credential()).await.unwrap();
        assert_eq!(user.login, "Jane Doe");
        assert_eq!(user.avatar_url, "https://example.invalid/a.png");
    }

    #[tokio::test]
    async fn fetch_user_surfaces_the_api_error_message() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/user")
            .with_status(401)
            .with_body(json!({ "type": "error", "error": { "message": "Invalid credentials" } }).to_string())
            .create_async()
            .await;

        let err = fetch_user(&server.url(), &test_credential()).await.unwrap_err();
        assert!(err.contains("Invalid credentials"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn list_pull_requests_normalizes_state_and_branches() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/repositories/team/repo/pullrequests")
            .match_query(mockito::Matcher::UrlEncoded("state".into(), "OPEN".into()))
            .with_status(200)
            .with_body(json!({ "values": [sample_pr_json(1, "OPEN")] }).to_string())
            .create_async()
            .await;

        let prs = list_pull_requests(&server.url(), &test_credential(), "team", "repo", "open").await.unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].state, "open");
        assert_eq!(prs[0].head, "feature-x");
        assert_eq!(prs[0].base, "main");
    }

    #[tokio::test]
    async fn get_pull_request_marks_merged_state() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/repositories/team/repo/pullrequests/1")
            .with_status(200)
            .with_body(sample_pr_json(1, "MERGED").to_string())
            .create_async()
            .await;

        let pr = get_pull_request(&server.url(), &test_credential(), "team", "repo", 1).await.unwrap();
        assert!(pr.merged);
        assert_eq!(pr.summary.state, "closed");
        assert_eq!(pr.body, "Description here");
    }

    #[tokio::test]
    async fn create_pull_request_sends_expected_payload() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/repositories/team/repo/pullrequests")
            .match_body(mockito::Matcher::PartialJson(json!({
                "title": "Add feature",
                "source": { "branch": { "name": "feature-x" } },
                "destination": { "branch": { "name": "main" } },
            })))
            .with_status(201)
            .with_body(sample_pr_json(9, "OPEN").to_string())
            .create_async()
            .await;

        let pr = create_pull_request(
            &server.url(),
            &test_credential(),
            "team",
            "repo",
            "Add feature",
            "Description here",
            "feature-x",
            "main",
            false,
        )
        .await
        .unwrap();

        assert_eq!(pr.summary.number, 9);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn merge_pull_request_succeeds_on_2xx() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/repositories/team/repo/pullrequests/1/merge")
            .match_body(mockito::Matcher::PartialJson(json!({ "merge_strategy": "squash" })))
            .with_status(200)
            .with_body(sample_pr_json(1, "MERGED").to_string())
            .create_async()
            .await;

        merge_pull_request(&server.url(), &test_credential(), "team", "repo", 1, "squash").await.unwrap();
    }

    #[tokio::test]
    async fn merge_pull_request_fails_with_the_api_message_on_conflict() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/repositories/team/repo/pullrequests/1/merge")
            .with_status(400)
            .with_body(json!({ "type": "error", "error": { "message": "Merge conflict" } }).to_string())
            .create_async()
            .await;

        let err = merge_pull_request(&server.url(), &test_credential(), "team", "repo", 1, "merge").await.unwrap_err();
        assert_eq!(err, "Merge conflict");
    }
}
