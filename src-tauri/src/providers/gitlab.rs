use super::{ConnectedUser, PullRequestDetail, PullRequestSummary};
use serde::{de::DeserializeOwned, Deserialize};

pub const API_BASE: &str = "https://gitlab.com/api/v4";

const USER_AGENT: &str = "Stash-Git-Client";

/// Extracts `(owner, repo)` from a GitLab remote URL. As with GitHub and
/// Bitbucket, `repo` is "everything after the first path segment" rather
/// than a strict single segment — for github.com/bitbucket.org that's
/// always exactly one segment anyway, but GitLab projects can live under
/// nested subgroups (`group/subgroup/project`), and `project_id` below
/// rejoins owner+repo to reconstruct the full path, so this happens to
/// handle that case correctly too.
pub fn parse_remote(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");

    let after_host = trimmed
        .strip_prefix("git@gitlab.com:")
        .or_else(|| trimmed.strip_prefix("ssh://git@gitlab.com/"))
        .or_else(|| trimmed.strip_prefix("https://gitlab.com/"))
        .or_else(|| trimmed.strip_prefix("http://gitlab.com/"))?;

    let mut parts = after_host.splitn(2, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

/// GitLab's project-scoped endpoints take the URL-encoded full path
/// (`group%2Fsubgroup%2Fproject`) as `:id` — project paths only allow
/// characters that are already URL-safe besides the separating slash, so
/// escaping just that is enough.
fn project_id(owner: &str, repo: &str) -> String {
    format!("{owner}/{repo}").replace('/', "%2F")
}

#[derive(Deserialize)]
struct RawUser {
    username: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct RawMergeRequest {
    iid: u64,
    title: String,
    state: String,
    #[serde(default)]
    draft: bool,
    author: RawUser,
    source_branch: String,
    target_branch: String,
    web_url: String,
    created_at: String,
    updated_at: String,
    description: Option<String>,
    merge_status: Option<String>,
    changes_count: Option<String>,
}

impl RawMergeRequest {
    fn into_summary(self) -> PullRequestSummary {
        PullRequestSummary {
            number: self.iid,
            title: self.title,
            author: self.author.username,
            // GitLab uses opened/closed/merged/locked; normalize to
            // GitHub's lowercase open/closed like the Bitbucket client
            // does, so the frontend has one shape regardless of provider.
            state: if self.state == "opened" { "open".to_string() } else { "closed".to_string() },
            draft: self.draft,
            base: self.target_branch,
            head: self.source_branch,
            html_url: self.web_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    fn into_detail(self) -> PullRequestDetail {
        let merged = self.state == "merged";
        let mergeable = match self.merge_status.as_deref() {
            Some("can_be_merged") => Some(true),
            Some("cannot_be_merged") => Some(false),
            _ => None,
        };
        // "changes_count" is a string like "3" or "1000+" when GitLab caps
        // it; anything that doesn't parse cleanly is left at 0 rather than
        // guessed. No cheap endpoint gives additions/deletions/commits.
        let changed_files = self
            .changes_count
            .as_deref()
            .and_then(|s| s.trim_end_matches('+').parse::<i64>().ok())
            .unwrap_or(0);

        PullRequestDetail {
            body: self.description.clone().unwrap_or_default(),
            merged,
            mergeable,
            mergeable_state: self.merge_status.clone().unwrap_or_default(),
            additions: 0,
            deletions: 0,
            changed_files,
            commits: 0,
            summary: self.into_summary(),
        }
    }
}

fn auth(builder: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    builder.header("Authorization", format!("Bearer {token}")).header("User-Agent", USER_AGENT)
}

/// Parses a GitLab API response, surfacing the API's own `message` field
/// on error instead of a bare status code.
async fn handle_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, String> {
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;

    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| format!("Respuesta inesperada de GitLab: {e}"))
    } else {
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("message").map(|m| m.to_string()).or_else(|| v.get("error").map(|e| e.to_string())))
            .unwrap_or(text);
        Err(format!("GitLab respondió {status}: {message}"))
    }
}

pub async fn fetch_user(base_url: &str, token: &str) -> Result<ConnectedUser, String> {
    let client = reqwest::Client::new();
    let resp = auth(client.get(format!("{base_url}/user")), token).send().await.map_err(|e| e.to_string())?;
    let raw: RawUser = handle_response(resp).await?;
    Ok(ConnectedUser { login: raw.username, avatar_url: raw.avatar_url.unwrap_or_default() })
}

/// GitLab's `state` filter only accepts one value per request (no OR
/// filtering like Bitbucket's repeated query params), so a "closed"
/// filter — meant to mean "not open", same as the other providers —
/// takes two requests (closed + merged) concatenated together.
fn state_filter_values(state: &str) -> Vec<&'static str> {
    match state {
        "open" => vec!["opened"],
        "closed" => vec!["closed", "merged"],
        _ => vec!["all"],
    }
}

pub async fn list_pull_requests(
    base_url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    state: &str,
) -> Result<Vec<PullRequestSummary>, String> {
    let id = project_id(owner, repo);
    let client = reqwest::Client::new();

    let mut results = Vec::new();
    for state_value in state_filter_values(state) {
        let resp = auth(
            client
                .get(format!("{base_url}/projects/{id}/merge_requests"))
                .query(&[("state", state_value), ("per_page", "50")]),
            token,
        )
        .send()
        .await
        .map_err(|e| e.to_string())?;
        let raw: Vec<RawMergeRequest> = handle_response(resp).await?;
        results.extend(raw.into_iter().map(RawMergeRequest::into_summary));
    }
    Ok(results)
}

pub async fn get_pull_request(
    base_url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<PullRequestDetail, String> {
    let id = project_id(owner, repo);
    let client = reqwest::Client::new();
    let resp = auth(client.get(format!("{base_url}/projects/{id}/merge_requests/{number}")), token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw: RawMergeRequest = handle_response(resp).await?;
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
    let id = project_id(owner, repo);
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "source_branch": head,
        "target_branch": base,
        "title": title,
        "description": body,
        "draft": draft,
    });
    let resp = auth(client.post(format!("{base_url}/projects/{id}/merge_requests")), token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let raw: RawMergeRequest = handle_response(resp).await?;
    Ok(raw.into_detail())
}

/// GitLab's merge endpoint takes a `squash` boolean rather than a named
/// strategy, and has no rebase-and-merge equivalent (rebase is a
/// separate, non-merging operation there) — the closest we can do for
/// our generic "rebase" option is a plain (non-squash) merge.
fn wants_squash(method: &str) -> bool {
    method == "squash"
}

pub async fn merge_pull_request(
    base_url: &str,
    token: &str,
    owner: &str,
    repo: &str,
    number: u64,
    method: &str,
) -> Result<(), String> {
    let id = project_id(owner, repo);
    let client = reqwest::Client::new();
    let payload = serde_json::json!({ "squash": wants_squash(method) });
    let resp = auth(client.put(format!("{base_url}/projects/{id}/merge_requests/{number}/merge")), token)
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
            .and_then(|v| v.get("message").map(|m| m.to_string()))
            .unwrap_or(text);
        Err(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_mr_json(iid: u64, state: &str) -> serde_json::Value {
        json!({
            "iid": iid,
            "title": "Add feature",
            "state": state,
            "draft": false,
            "author": { "username": "jane", "avatar_url": "https://example.invalid/a.png" },
            "source_branch": "feature-x",
            "target_branch": "main",
            "web_url": "https://gitlab.com/team/project/-/merge_requests/1",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "description": "Description here",
            "merge_status": "can_be_merged",
            "changes_count": "3",
        })
    }

    #[test]
    fn recognizes_common_gitlab_remote_forms() {
        assert_eq!(parse_remote("https://gitlab.com/team/project.git"), Some(("team".to_string(), "project".to_string())));
        assert_eq!(parse_remote("git@gitlab.com:team/project.git"), Some(("team".to_string(), "project".to_string())));
        assert_eq!(
            parse_remote("ssh://git@gitlab.com/team/project.git"),
            Some(("team".to_string(), "project".to_string()))
        );
    }

    #[test]
    fn keeps_nested_subgroup_segments_in_repo() {
        assert_eq!(
            parse_remote("https://gitlab.com/team/subgroup/project.git"),
            Some(("team".to_string(), "subgroup/project".to_string()))
        );
        assert_eq!(project_id("team", "subgroup/project"), "team%2Fsubgroup%2Fproject");
    }

    #[test]
    fn rejects_non_gitlab_urls() {
        assert_eq!(parse_remote("https://github.com/team/project.git"), None);
    }

    #[tokio::test]
    async fn fetch_user_parses_username_and_avatar() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/user")
            .match_header("authorization", "Bearer test-token")
            .with_status(200)
            .with_body(json!({ "username": "jane", "avatar_url": "https://example.invalid/a.png" }).to_string())
            .create_async()
            .await;

        let user = fetch_user(&server.url(), "test-token").await.unwrap();
        assert_eq!(user.login, "jane");
    }

    #[tokio::test]
    async fn fetch_user_surfaces_the_api_error_message() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/user")
            .with_status(401)
            .with_body(json!({ "message": "401 Unauthorized" }).to_string())
            .create_async()
            .await;

        let err = fetch_user(&server.url(), "bad-token").await.unwrap_err();
        assert!(err.contains("401 Unauthorized"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn list_pull_requests_open_maps_state_and_branches() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/projects/team%2Fproject/merge_requests")
            .match_query(mockito::Matcher::UrlEncoded("state".into(), "opened".into()))
            .with_status(200)
            .with_body(json!([sample_mr_json(1, "opened")]).to_string())
            .create_async()
            .await;

        let prs = list_pull_requests(&server.url(), "tok", "team", "project", "open").await.unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].state, "open");
        assert_eq!(prs[0].head, "feature-x");
        assert_eq!(prs[0].base, "main");
    }

    #[tokio::test]
    async fn list_pull_requests_closed_combines_closed_and_merged() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/projects/team%2Fproject/merge_requests")
            .match_query(mockito::Matcher::UrlEncoded("state".into(), "closed".into()))
            .with_status(200)
            .with_body(json!([sample_mr_json(1, "closed")]).to_string())
            .create_async()
            .await;
        server
            .mock("GET", "/projects/team%2Fproject/merge_requests")
            .match_query(mockito::Matcher::UrlEncoded("state".into(), "merged".into()))
            .with_status(200)
            .with_body(json!([sample_mr_json(2, "merged")]).to_string())
            .create_async()
            .await;

        let prs = list_pull_requests(&server.url(), "tok", "team", "project", "closed").await.unwrap();
        assert_eq!(prs.len(), 2);
        assert!(prs.iter().all(|p| p.state == "closed"));
    }

    #[tokio::test]
    async fn get_pull_request_maps_merge_status_and_changes_count() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/projects/team%2Fproject/merge_requests/1")
            .with_status(200)
            .with_body(sample_mr_json(1, "opened").to_string())
            .create_async()
            .await;

        let mr = get_pull_request(&server.url(), "tok", "team", "project", 1).await.unwrap();
        assert_eq!(mr.mergeable, Some(true));
        assert_eq!(mr.changed_files, 3);
        assert_eq!(mr.body, "Description here");
    }

    #[tokio::test]
    async fn create_pull_request_sends_expected_payload() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/projects/team%2Fproject/merge_requests")
            .match_body(mockito::Matcher::PartialJson(json!({
                "source_branch": "feature-x",
                "target_branch": "main",
                "title": "Add feature",
            })))
            .with_status(201)
            .with_body(sample_mr_json(9, "opened").to_string())
            .create_async()
            .await;

        let mr = create_pull_request(&server.url(), "tok", "team", "project", "Add feature", "Description here", "feature-x", "main", false)
            .await
            .unwrap();

        assert_eq!(mr.summary.number, 9);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn merge_pull_request_succeeds_on_2xx() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("PUT", "/projects/team%2Fproject/merge_requests/1/merge")
            .match_body(mockito::Matcher::PartialJson(json!({ "squash": true })))
            .with_status(200)
            .with_body(sample_mr_json(1, "merged").to_string())
            .create_async()
            .await;

        merge_pull_request(&server.url(), "tok", "team", "project", 1, "squash").await.unwrap();
    }

    #[tokio::test]
    async fn merge_pull_request_fails_with_the_api_message() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("PUT", "/projects/team%2Fproject/merge_requests/1/merge")
            .with_status(406)
            .with_body(json!({ "message": "Branch cannot be merged" }).to_string())
            .create_async()
            .await;

        let err = merge_pull_request(&server.url(), "tok", "team", "project", 1, "merge").await.unwrap_err();
        assert!(err.contains("Branch cannot be merged"));
    }
}
