use super::{ConnectedUser, PullRequestDetail, PullRequestSummary};
use serde::{de::DeserializeOwned, Deserialize};

pub const API_BASE: &str = "https://api.github.com";

const USER_AGENT: &str = "Stash-Git-Client";

/// Extracts `(owner, repo)` from a GitHub remote URL, supporting the forms
/// git actually produces: `https://github.com/owner/repo.git`,
/// `git@github.com:owner/repo.git`, and `ssh://git@github.com/owner/repo`.
pub fn parse_remote(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");

    let after_host = trimmed
        .strip_prefix("git@github.com:")
        .or_else(|| trimmed.strip_prefix("ssh://git@github.com/"))
        .or_else(|| trimmed.strip_prefix("https://github.com/"))
        .or_else(|| trimmed.strip_prefix("http://github.com/"))?;

    let mut parts = after_host.splitn(2, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

#[derive(Deserialize)]
struct RawUser {
    login: String,
    avatar_url: String,
}

#[derive(Deserialize)]
struct RawBranchRef {
    #[serde(rename = "ref")]
    ref_: String,
}

#[derive(Deserialize)]
struct RawPullRequest {
    number: u64,
    title: String,
    state: String,
    draft: bool,
    user: RawUser,
    base: RawBranchRef,
    head: RawBranchRef,
    html_url: String,
    created_at: String,
    updated_at: String,
    body: Option<String>,
    merged: Option<bool>,
    mergeable: Option<bool>,
    mergeable_state: Option<String>,
    additions: Option<i64>,
    deletions: Option<i64>,
    changed_files: Option<i64>,
    commits: Option<i64>,
}

impl RawPullRequest {
    fn into_summary(self) -> PullRequestSummary {
        PullRequestSummary {
            number: self.number,
            title: self.title,
            author: self.user.login,
            state: self.state,
            draft: self.draft,
            base: self.base.ref_,
            head: self.head.ref_,
            html_url: self.html_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    fn into_detail(self) -> PullRequestDetail {
        PullRequestDetail {
            body: self.body.clone().unwrap_or_default(),
            merged: self.merged.unwrap_or(false),
            mergeable: self.mergeable,
            mergeable_state: self.mergeable_state.clone().unwrap_or_default(),
            additions: self.additions.unwrap_or(0),
            deletions: self.deletions.unwrap_or(0),
            changed_files: self.changed_files.unwrap_or(0),
            commits: self.commits.unwrap_or(0),
            summary: self.into_summary(),
        }
    }
}

fn auth(builder: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    builder
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
}

/// Parses a GitHub API response, surfacing the API's own `message` field on
/// error instead of a bare status code, since that's what actually explains
/// things like an expired token or an unmerged-because-of-conflicts PR.
async fn handle_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, String> {
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;

    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| format!("Respuesta inesperada de GitHub: {e}"))
    } else {
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
            .unwrap_or(text);
        Err(format!("GitHub respondió {status}: {message}"))
    }
}

pub async fn fetch_user(base_url: &str, token: &str) -> Result<ConnectedUser, String> {
    let client = reqwest::Client::new();
    let resp = auth(client.get(format!("{base_url}/user")), token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw: RawUser = handle_response(resp).await?;
    Ok(ConnectedUser { login: raw.login, avatar_url: raw.avatar_url })
}

pub async fn list_pull_requests(
    base_url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    state: &str,
) -> Result<Vec<PullRequestSummary>, String> {
    let client = reqwest::Client::new();
    let resp = auth(
        client
            .get(format!("{base_url}/repos/{owner}/{repo}/pulls"))
            .query(&[("state", state), ("per_page", "50")]),
        token,
    )
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let raw: Vec<RawPullRequest> = handle_response(resp).await?;
    Ok(raw.into_iter().map(RawPullRequest::into_summary).collect())
}

pub async fn get_pull_request(
    base_url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<PullRequestDetail, String> {
    let client = reqwest::Client::new();
    let resp = auth(client.get(format!("{base_url}/repos/{owner}/{repo}/pulls/{number}")), token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw: RawPullRequest = handle_response(resp).await?;
    Ok(raw.into_detail())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_pull_request(
    base_url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    title: &str,
    body: &str,
    head: &str,
    base: &str,
    draft: bool,
) -> Result<PullRequestDetail, String> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "title": title,
        "body": body,
        "head": head,
        "base": base,
        "draft": draft,
    });
    let resp = auth(client.post(format!("{base_url}/repos/{owner}/{repo}/pulls")), token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw: RawPullRequest = handle_response(resp).await?;
    Ok(raw.into_detail())
}

#[derive(Deserialize)]
struct MergeResult {
    merged: bool,
    message: String,
}

pub async fn merge_pull_request(
    base_url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    number: u64,
    method: &str,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({ "merge_method": method });
    let resp = auth(
        client.put(format!("{base_url}/repos/{owner}/{repo}/pulls/{number}/merge")),
        token,
    )
    .json(&payload)
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let result: MergeResult = handle_response(resp).await?;
    if result.merged {
        Ok(())
    } else {
        Err(result.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_pr_json(number: u64) -> serde_json::Value {
        json!({
            "number": number,
            "title": "Add feature",
            "state": "open",
            "draft": false,
            "user": { "login": "octocat", "avatar_url": "https://example.invalid/a.png" },
            "base": { "ref": "main" },
            "head": { "ref": "feature-x" },
            "html_url": "https://github.com/octocat/hello/pull/1",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "body": "Description here",
            "merged": false,
            "mergeable": true,
            "mergeable_state": "clean",
            "additions": 10,
            "deletions": 2,
            "changed_files": 3,
            "commits": 1,
        })
    }

    #[test]
    fn recognizes_common_github_remote_forms() {
        assert_eq!(parse_remote("https://github.com/facebook/react.git"), Some(("facebook".to_string(), "react".to_string())));
        assert_eq!(parse_remote("https://github.com/facebook/react"), Some(("facebook".to_string(), "react".to_string())));
        assert_eq!(parse_remote("git@github.com:facebook/react.git"), Some(("facebook".to_string(), "react".to_string())));
        assert_eq!(
            parse_remote("ssh://git@github.com/facebook/react.git"),
            Some(("facebook".to_string(), "react".to_string()))
        );
        assert_eq!(parse_remote("https://github.com/facebook/react/"), Some(("facebook".to_string(), "react".to_string())));
    }

    #[test]
    fn rejects_non_github_or_malformed_urls() {
        assert_eq!(parse_remote("https://bitbucket.org/owner/repo.git"), None);
        assert_eq!(parse_remote("C:\\repos\\local-repo"), None);
        assert_eq!(parse_remote("https://github.com/only-owner"), None);
    }

    #[tokio::test]
    async fn fetch_user_parses_login_and_avatar() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/user")
            .match_header("authorization", "Bearer test-token")
            .with_status(200)
            .with_body(json!({ "login": "octocat", "avatar_url": "https://example.invalid/a.png" }).to_string())
            .create_async()
            .await;

        let user = fetch_user(&server.url(), "test-token").await.unwrap();
        assert_eq!(user.login, "octocat");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_user_surfaces_the_api_error_message() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/user")
            .with_status(401)
            .with_body(json!({ "message": "Bad credentials" }).to_string())
            .create_async()
            .await;

        let err = fetch_user(&server.url(), "bad-token").await.unwrap_err();
        assert!(err.contains("Bad credentials"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn list_pull_requests_maps_state_and_branches() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/repos/octocat/hello/pulls")
            .match_query(mockito::Matcher::UrlEncoded("state".into(), "open".into()))
            .with_status(200)
            .with_body(json!([sample_pr_json(1)]).to_string())
            .create_async()
            .await;

        let prs = list_pull_requests(&server.url(), "tok", "octocat", "hello", "open").await.unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 1);
        assert_eq!(prs[0].base, "main");
        assert_eq!(prs[0].head, "feature-x");
        assert_eq!(prs[0].author, "octocat");
    }

    #[tokio::test]
    async fn get_pull_request_includes_detail_fields() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/repos/octocat/hello/pulls/1")
            .with_status(200)
            .with_body(sample_pr_json(1).to_string())
            .create_async()
            .await;

        let pr = get_pull_request(&server.url(), "tok", "octocat", "hello", 1).await.unwrap();
        assert_eq!(pr.summary.number, 1);
        assert_eq!(pr.body, "Description here");
        assert_eq!(pr.additions, 10);
        assert_eq!(pr.mergeable, Some(true));
    }

    #[tokio::test]
    async fn create_pull_request_sends_expected_payload() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/repos/octocat/hello/pulls")
            .match_body(mockito::Matcher::PartialJson(json!({
                "title": "Add feature",
                "head": "feature-x",
                "base": "main",
                "draft": false,
            })))
            .with_status(201)
            .with_body(sample_pr_json(7).to_string())
            .create_async()
            .await;

        let pr = create_pull_request(&server.url(), "tok", "octocat", "hello", "Add feature", "Description here", "feature-x", "main", false)
            .await
            .unwrap();

        assert_eq!(pr.summary.number, 7);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn merge_pull_request_succeeds_when_merged_is_true() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("PUT", "/repos/octocat/hello/pulls/1/merge")
            .with_status(200)
            .with_body(json!({ "merged": true, "message": "Pull Request successfully merged" }).to_string())
            .create_async()
            .await;

        merge_pull_request(&server.url(), "tok", "octocat", "hello", 1, "squash").await.unwrap();
    }

    #[tokio::test]
    async fn merge_pull_request_fails_when_merged_is_false() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("PUT", "/repos/octocat/hello/pulls/1/merge")
            .with_status(200)
            .with_body(json!({ "merged": false, "message": "Merge conflict" }).to_string())
            .create_async()
            .await;

        let err = merge_pull_request(&server.url(), "tok", "octocat", "hello", 1, "merge").await.unwrap_err();
        assert_eq!(err, "Merge conflict");
    }
}
