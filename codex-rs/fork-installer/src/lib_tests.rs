use super::*;
use crate::test_support::Fixture;
use crate::test_support::PackageSpec;
use crate::test_support::host_target;
use crate::test_support::installer_at;
use crate::test_support::selected;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const RELEASES: &str = "/repos/mevanlc/codex/releases";

struct Server {
    server: MockServer,
    fixture: Fixture,
}

impl Server {
    async fn start() -> Self {
        Self {
            server: MockServer::start().await,
            fixture: Fixture::new(),
        }
    }

    /// A release JSON row; `package` is the version of a package to serve for
    /// this platform, or None for a release without it.
    async fn release(&self, tag: &str, published: &str, package: Option<&str>) -> Value {
        let (name, bytes) = match package {
            Some(version) => {
                let archive = self.fixture.package(&PackageSpec::new(version));
                (
                    host_target().default_asset(),
                    std::fs::read(archive).unwrap(),
                )
            }
            None => (
                "codex-other-platform.tar.zst".to_string(),
                b"other".to_vec(),
            ),
        };
        let url_path = format!("/download/{tag}/{name}");
        Mock::given(method("GET"))
            .and(path(url_path.clone()))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
            .mount(&self.server)
            .await;
        json!({
            "tag_name": tag,
            "published_at": published,
            "created_at": published,
            "draft": false,
            "prerelease": false,
            "assets": [{
                "name": name,
                "updated_at": published,
                "browser_download_url": format!("{}{url_path}", self.server.uri()),
                "size": bytes.len(),
                "digest": format!("sha256:{:x}", sha2::Sha256::digest(&bytes)),
            }],
        })
    }

    async fn pages(&self, pages: Vec<Vec<Value>>) {
        for (index, page) in pages.into_iter().enumerate() {
            Mock::given(method("GET"))
                .and(path(RELEASES))
                .and(query_param("page", (index + 1).to_string()))
                .respond_with(ResponseTemplate::new(200).set_body_json(page))
                .mount(&self.server)
                .await;
        }
    }

    async fn tag(&self, release: &Value) {
        Mock::given(method("GET"))
            .and(path(format!(
                "{RELEASES}/tags/{}",
                release["tag_name"].as_str().unwrap()
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(release))
            .mount(&self.server)
            .await;
    }

    async fn run(&self, request: Request) -> (Result<()>, String) {
        let environment = Environment {
            user_home: self.fixture.home.clone(),
            artifact_name: None,
            release_asset_name: None,
            daemon_only: false,
            guards: Guards::default(),
            token: None,
            temp_root: Some(self.fixture.dir.path().join("tmp")),
            api_url: self.server.uri(),
        };
        let result = tokio::task::spawn_blocking(move || {
            let installer = installer_at(&environment.user_home, false, Guards::default());
            let result = installer.run(&request, &environment);
            (result, installer.output.text())
        })
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_dir(self.fixture.dir.path().join("tmp"))
                .map(Iterator::count)
                .unwrap_or(0),
            0,
            "temporary downloads must be removed"
        );
        result
    }

    fn installed_tag(&self) -> Value {
        let release = selected(&self.fixture.cli_root());
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(release.join("install-receipt.json")).unwrap())
                .unwrap();
        receipt["tag"].clone()
    }

    async fn downloads(&self) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap_or_default()
            .iter()
            .filter(|request| request.url.path().starts_with("/download/"))
            .count()
    }
}

fn release_request() -> Request {
    Request {
        mode: Mode::Release,
        version: None,
        list_available: false,
        force: false,
    }
}

fn flagged(mut release: Value, flag: &str) -> Value {
    release[flag] = json!(true);
    release
}

#[tokio::test(flavor = "multi_thread")]
async fn partial_release_falls_back_across_pages_by_publication_time() {
    let server = Server::start().await;
    let mut first = Vec::new();
    for index in 0..96 {
        first.push(
            server
                .release(&format!("filler-{index}"), "2026-09-01T00:00:00Z", None)
                .await,
        );
    }
    first.push(
        server
            .release("partial", "2026-10-04T00:00:00Z", None)
            .await,
    );
    first.push(
        server
            .release(
                "fork-v1.0.0-older",
                "2026-10-01T00:00:00Z",
                Some("1.0.0-older"),
            )
            .await,
    );
    let draft = server
        .release(
            "fork-v1.0.2-draft",
            "2026-10-06T00:00:00Z",
            Some("1.0.2-draft"),
        )
        .await;
    first.push(flagged(draft, "draft"));
    let preview = server
        .release(
            "fork-v1.0.3-preview",
            "2026-10-05T00:00:00Z",
            Some("1.0.3-preview"),
        )
        .await;
    first.push(flagged(preview, "prerelease"));
    let newest = server
        .release(
            "fork-v1.0.1-newest",
            "2026-10-02T00:00:00Z",
            Some("1.0.1-newest"),
        )
        .await;
    server.pages(vec![first, vec![newest]]).await;

    let (result, output) = server.run(release_request()).await;
    result.unwrap();
    assert_eq!(server.installed_tag(), "fork-v1.0.1-newest");
    assert!(output.contains("Requested source: release assets from tag fork-v1.0.1-newest"));
    assert!(
        output.contains("Source timestamp: 2026-10-02 00:00:00 UTC (epoch: 1790899200)"),
        "{output}"
    );
    assert_eq!(server.downloads().await, 1);

    // Reinstalling the same release is a no-op that keeps following latest.
    let (result, output) = server.run(release_request()).await;
    result.unwrap();
    assert!(output.contains("already as new or newer"), "{output}");
    assert!(
        output.contains("Existing version: codex 1.0.1-newest"),
        "{output}"
    );
    assert!(output.contains("Existing Codex mtime:"), "{output}");
    // An installation that is already current downloads nothing.
    assert!(!output.contains("Downloading"), "{output}");
    assert_eq!(server.downloads().await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn list_marks_the_default_and_excludes_partial_and_draft_releases() {
    let server = Server::start().await;
    let partial = server
        .release("partial", "2026-10-04T00:00:00Z", None)
        .await;
    let preview = server
        .release(
            "fork-v1.0.3-preview",
            "2026-10-05T00:00:00Z",
            Some("1.0.3-preview"),
        )
        .await;
    let draft = server
        .release(
            "fork-v1.0.4-draft",
            "2026-10-06T00:00:00Z",
            Some("1.0.4-draft"),
        )
        .await;
    let older = server
        .release(
            "fork-v1.0.0-older",
            "2026-10-01T00:00:00Z",
            Some("1.0.0-older"),
        )
        .await;
    let newest = server
        .release(
            "fork-v1.0.1-newest",
            "2026-10-02T00:00:00Z",
            Some("1.0.1-newest"),
        )
        .await;
    server
        .pages(vec![vec![
            partial,
            flagged(preview, "prerelease"),
            flagged(draft, "draft"),
            older,
            newest,
        ]])
        .await;
    let (result, output) = server
        .run(Request {
            list_available: true,
            ..release_request()
        })
        .await;
    result.unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(
        lines,
        [
            "TAG  PUBLISHED  LAYOUT",
            "fork-v1.0.3-preview  2026-10-05T00:00:00Z  package",
            "fork-v1.0.1-newest  2026-10-02T00:00:00Z  package (default)",
            "fork-v1.0.0-older  2026-10-01T00:00:00Z  package",
        ]
    );
    assert_eq!(server.downloads().await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_asset_and_prerelease_only_fail_without_downloading() {
    for prerelease in [false, true] {
        let server = Server::start().await;
        let release = if prerelease {
            let release = server
                .release(
                    "fork-v1.0.3-preview",
                    "2026-10-05T00:00:00Z",
                    Some("1.0.3-preview"),
                )
                .await;
            flagged(release, "prerelease")
        } else {
            server
                .release("partial", "2026-10-04T00:00:00Z", None)
                .await
        };
        server.pages(vec![vec![release]]).await;
        let (result, _) = server.run(release_request()).await;
        let error = format!("{:#}", result.unwrap_err());
        assert!(
            error.contains(&format!(
                "no published non-prerelease carries {}",
                host_target().default_asset()
            )),
            "{error}"
        );
        assert_eq!(server.downloads().await, 0);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_version_can_select_a_prerelease_and_pins_it() {
    let server = Server::start().await;
    let release = server
        .release(
            "fork-v0.161.0-test",
            "2026-10-05T00:00:00Z",
            Some("0.161.0-test"),
        )
        .await;
    server.tag(&flagged(release, "prerelease")).await;
    let (result, output) = server
        .run(Request {
            version: Some("0.161.0-test".to_string()),
            ..release_request()
        })
        .await;
    result.unwrap();
    assert!(output.contains("Explicit version requested"), "{output}");
    assert_eq!(server.installed_tag(), "fork-v0.161.0-test");
    assert!(
        !server
            .fixture
            .cli_root()
            .join("auto-update-version")
            .exists()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_version_missing_asset_does_not_fall_back() {
    let server = Server::start().await;
    let available = server
        .release(
            "fork-v1.0.0-available",
            "2026-10-02T00:00:00Z",
            Some("1.0.0-available"),
        )
        .await;
    server.pages(vec![vec![available]]).await;
    let partial = server
        .release("partial", "2026-10-04T00:00:00Z", None)
        .await;
    server.tag(&partial).await;
    let (result, _) = server
        .run(Request {
            version: Some("partial".to_string()),
            ..release_request()
        })
        .await;
    let error = format!("{:#}", result.unwrap_err());
    assert!(
        error.contains("requested release in mevanlc/codex does not include asset"),
        "{error}"
    );
    assert_eq!(server.downloads().await, 0);

    let (result, _) = server
        .run(Request {
            version: Some("9.9.9".to_string()),
            ..release_request()
        })
        .await;
    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains("no release matching \"9.9.9\""), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn metadata_failures_are_reported_as_such() {
    let server = Server::start().await;
    Mock::given(method("GET"))
        .and(path(RELEASES))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .set_body_json(json!({"message": "API rate limit exceeded"})),
        )
        .mount(&server.server)
        .await;
    let (result, _) = server.run(release_request()).await;
    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains("status 403"), "{error}");
    assert!(error.contains("API rate limit exceeded"), "{error}");
    assert!(error.contains("set GH_TOKEN"), "{error}");
    assert!(!error.contains("carries"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn downloads_must_match_their_published_digest() {
    let server = Server::start().await;
    let installed = server
        .release(
            "fork-v1.0.0-older",
            "2026-10-01T00:00:00Z",
            Some("1.0.0-older"),
        )
        .await;
    server.pages(vec![vec![installed.clone()]]).await;
    server.run(release_request()).await.0.unwrap();
    let old = selected(&server.fixture.cli_root());

    for corrupt in ["digest", "missing", "unavailable"] {
        let server = Server {
            server: MockServer::start().await,
            fixture: Fixture {
                dir: tempfile::tempdir().unwrap(),
                home: server.fixture.home.clone(),
            },
        };
        let mut release = server
            .release(
                "fork-v1.0.1-newest",
                "2026-10-02T00:00:00Z",
                Some("1.0.1-newest"),
            )
            .await;
        let asset = &mut release["assets"][0];
        match corrupt {
            "digest" => asset["digest"] = json!(format!("sha256:{}", "0".repeat(64))),
            "missing" => asset["digest"] = Value::Null,
            _ => asset["browser_download_url"] = json!(format!("{}/gone", server.server.uri())),
        }
        server.pages(vec![vec![release]]).await;
        let (result, _) = server.run(release_request()).await;
        let error = format!("{:#}", result.unwrap_err());
        let expected = match corrupt {
            "digest" => "does not match its published digest",
            "missing" => "published no SHA-256 digest",
            _ => "status 404",
        };
        assert!(error.contains(expected), "{corrupt}: {error}");
        assert_eq!(selected(&server.fixture.cli_root()), old);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn artifact_contains_one_package_and_is_pinned() {
    let server = Server::start().await;
    let archive = server.fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    let asset = host_target().default_asset();
    let container = server.fixture.dir.path().join("artifact.zip");
    crate::test_support::write_archive(
        &container,
        &[(asset, std::fs::read(&archive).unwrap(), 0o644)],
    );
    let bytes = std::fs::read(&container).unwrap();
    Mock::given(method("GET"))
        .and(path("/repos/mevanlc/codex/actions/artifacts"))
        .and(query_param(
            "name",
            format!("codex-{}", host_target().triple()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "artifacts": [
                {
                    "id": 41,
                    "name": format!("codex-{}", host_target().triple()),
                    "created_at": "2026-10-01T00:00:00Z",
                    "expired": false,
                    "archive_download_url": format!("{}/artifacts/41", server.server.uri()),
                    "digest": "sha256:00",
                    "workflow_run": {"id": 6},
                },
                {
                    "id": 42,
                    "name": format!("codex-{}", host_target().triple()),
                    "created_at": "2026-10-02T00:00:00Z",
                    "expired": false,
                    "archive_download_url": format!("{}/artifacts/42", server.server.uri()),
                    "digest": format!("sha256:{:x}", sha2::Sha256::digest(&bytes)),
                    "workflow_run": {"id": 7},
                },
            ],
        })))
        .mount(&server.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/artifacts/42"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&server.server)
        .await;
    let environment = Environment {
        user_home: server.fixture.home.clone(),
        artifact_name: None,
        release_asset_name: None,
        daemon_only: false,
        guards: Guards::default(),
        token: Some("test-token".to_string()),
        temp_root: Some(server.fixture.dir.path().join("tmp")),
        api_url: server.server.uri(),
    };
    let request = Request {
        mode: Mode::Artifact,
        ..release_request()
    };
    let output = tokio::task::spawn_blocking(move || {
        let installer = installer_at(&environment.user_home, false, Guards::default());
        installer
            .run(&request, &environment)
            .map(|()| installer.output.text())
    })
    .await
    .unwrap()
    .unwrap();
    assert!(output.contains("artifact 42 from run 7"), "{output}");
    let release = selected(&server.fixture.cli_root());
    let receipt: Value =
        serde_json::from_slice(&std::fs::read(release.join("install-receipt.json")).unwrap())
            .unwrap();
    assert_eq!(receipt["artifactId"], 42);
    assert!(
        !server
            .fixture
            .cli_root()
            .join("auto-update-version")
            .exists()
    );
    let authorized = server
        .server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .all(|request| {
            request
                .headers
                .get("authorization")
                .map(reqwest::header::HeaderValue::as_bytes)
                == Some(b"Bearer test-token".as_slice())
        });
    assert!(authorized);
}

#[tokio::test(flavor = "multi_thread")]
async fn local_archives_are_pinned_without_a_currency_check() {
    let server = Server::start().await;
    let archive = server.fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    let request = Request {
        mode: Mode::Archive(archive.clone()),
        ..release_request()
    };
    for _ in 0..2 {
        let (result, output) = server.run(request.clone()).await;
        result.unwrap();
        assert!(output.contains("Local archive requested"), "{output}");
        assert!(!output.contains("already as new or newer"), "{output}");
    }
    let release = selected(&server.fixture.cli_root());
    let receipt: Value =
        serde_json::from_slice(&std::fs::read(release.join("install-receipt.json")).unwrap())
            .unwrap();
    assert_eq!(
        receipt["archive"],
        json!(archive.file_name().unwrap().to_string_lossy())
    );
    assert_eq!(receipt.get("publishedEpoch"), None);
    assert!(
        !server
            .fixture
            .cli_root()
            .join("auto-update-version")
            .exists()
    );
    assert_eq!(server.server.received_requests().await.unwrap().len(), 0);
}
