//! Sign-in requests, token failures and direct access reuse of the shipped gateway provider,
//! through the compiled component and the controlled GitLab.
use super::{DEFAULT_MODEL, Harness, keyed, types};
use maestro_models::*;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// The bundled client's identifier.
const CLIENT_ID: &str = "da4edff2e6ebd2bc3208611e2768bc1c1dd7be791dc5ff26ca34ca9ee44f7d4b";
/// The redirect address as a form component.
const REDIRECT: &str = "http%3A%2F%2F127.0.0.1%3A8080%2Fcallback";
/// GitLab's token-issue time for the controlled responses, in seconds.
const CREATED_AT: u32 = 1_700_000_000;

/// Answers the callback prompt with a callback derived from the authorization state.
struct Answering {
    /// The authorization addresses the sign-in published.
    published: Mutex<Vec<String>>,
    /// Builds the pasted callback address from the state the address carried.
    callback: Box<dyn Fn(&str) -> String + Send + Sync>,
}

impl Answering {
    /// Interaction whose pasted callback is `callback(state)`.
    fn new(callback: impl Fn(&str) -> String + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            published: Mutex::default(),
            callback: Box::new(callback),
        })
    }
}

impl OAuthLoginCallbacks for Answering {
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError> {
        assert!(info.instructions.is_none());
        self.published.lock().unwrap().push(info.url);
        Ok(())
    }

    fn on_prompt(&self, prompt: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        assert_eq!(prompt.message, "Paste the callback URL:");
        assert!(prompt.placeholder.is_none() && prompt.allow_empty.is_none());
        let url = self.published.lock().unwrap().last().cloned().unwrap();
        let answer = (self.callback)(url.split("&state=").nth(1).unwrap());
        Box::pin(async move { Ok(answer) })
    }
}

/// A successful token response issued at [`CREATED_AT`].
fn tokens(access: &str, refresh: &str, lifetime: u32) -> Value {
    json!({
        "access_token": access, "refresh_token": refresh,
        "expires_in": lifetime, "created_at": CREATED_AT
    })
}

/// A direct access grant carrying `token`.
fn grant(token: &str) -> Value {
    json!({"token": token, "headers": {"x-gitlab-realm": "saas", "x-gitlab-instance-id": "abc"}})
}

/// Stored credentials a refresh rotates.
fn stored(refresh: &str) -> OAuthCredentials {
    OAuthCredentials {
        refresh: refresh.into(),
        access: "access-1".into(),
        expires: 1.0,
        extra: JsonObject::default(),
    }
}

/// The request GitLab received at `index`: URL, method, headers and body.
fn request(harness: &Harness, index: usize) -> Value {
    harness.gitlab.requests.borrow()[index].clone()
}

/// How many requests GitLab has received.
fn received(harness: &Harness) -> usize {
    harness.gitlab.requests.borrow().len()
}

/// The text of `key` in an address or form body, up to the next `&`.
fn field<'a>(text: &'a str, key: &str) -> &'a str {
    text.split(&format!("{key}="))
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
}

/// Whether `state` is a version 4 UUID in its canonical spelling.
fn is_uuid_v4(state: &str) -> bool {
    let groups: Vec<&str> = state.split('-').collect();
    let lengths: Vec<usize> = groups.iter().map(|group| group.len()).collect();
    lengths == [8, 4, 4, 4, 12]
        && state
            .bytes()
            .all(|byte| byte == b'-' || byte.is_ascii_hexdigit())
        && groups[2].starts_with('4')
        && groups[3].starts_with(['8', '9', 'a', 'b'])
}

#[test]
fn custom_gateway_provider_login_sends_the_authorization_and_exchange_requests() {
    super::super::components::local(async {
        let harness = Harness::new(false).await;
        harness.reply(200, &tokens("access-1", "refresh-1", 7200));
        let sign_in = get_oauth_provider("gitlab-duo").unwrap();
        let interaction = Answering::new(|state| {
            format!("http://127.0.0.1:8080/callback?code=a%2Fb+c%C3%A9&state={state}#done")
        });
        let credentials = sign_in.login(interaction.clone(), None).await.unwrap();
        let url = interaction.published.lock().unwrap()[0].clone();
        let (base, query) = url.split_once('?').unwrap();
        assert_eq!(base, "https://gitlab.com/oauth/authorize");
        let challenge = field(query, "code_challenge");
        let state = field(query, "state");
        assert_eq!(
            query,
            format!(
                "client_id={CLIENT_ID}&redirect_uri={REDIRECT}&response_type=code&scope=api\
                 &code_challenge={challenge}&code_challenge_method=S256&state={state}"
            )
        );
        assert!(is_uuid_v4(state), "{state}");
        let exchange = request(&harness, 0);
        assert_eq!(exchange["url"], "https://gitlab.com/oauth/token");
        assert_eq!(exchange["method"], "POST");
        assert_eq!(
            exchange["headers"],
            json!([["Content-Type", "application/x-www-form-urlencoded"]])
        );
        let body = exchange["body"].as_str().unwrap();
        let verifier = field(body, "code_verifier");
        assert_eq!(
            body,
            format!(
                "client_id={CLIENT_ID}&grant_type=authorization_code&code=a%2Fb+c%C3%A9\
                 &code_verifier={verifier}&redirect_uri={REDIRECT}"
            )
        );
        assert_ne!(verifier, challenge);
        assert_eq!(sign_in.get_api_key(&credentials).unwrap(), "access-1");
        assert_eq!(sign_in.uses_callback_server(), None);
        assert!(sign_in.modify_models(&[], &credentials).is_none());
        harness.bus.clear();
    });
}

#[test]
fn custom_gateway_provider_login_rejects_callbacks_without_a_matching_code() {
    super::super::components::local(async {
        let harness = Harness::new(false).await;
        let sign_in = get_oauth_provider("gitlab-duo").unwrap();
        for (callback, message) in [
            (
                "state={state}",
                "No authorization code found in callback URL",
            ),
            (
                "code=&state={state}",
                "No authorization code found in callback URL",
            ),
            ("code=granted", "Authorization state mismatch"),
            ("code=granted&state=another", "Authorization state mismatch"),
        ] {
            harness.reply(200, &tokens("never", "never", 1));
            let interaction = Answering::new(move |state| {
                format!(
                    "http://127.0.0.1:8080/callback?{}",
                    callback.replace("{state}", state)
                )
            });
            let error = sign_in.login(interaction, None).await.unwrap_err();
            assert!(error.to_string().contains(message), "{callback}: {error}");
            assert_eq!(received(&harness), 0, "{callback}");
        }
        let interaction = Answering::new(|_| "not a url".to_owned());
        let error = sign_in.login(interaction, None).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("No authorization code found in callback URL"),
            "{error}"
        );
        assert_eq!(received(&harness), 0);
        harness.bus.clear();
    });
}

#[test]
fn custom_gateway_provider_refresh_rotates_credentials_through_the_token_endpoint() {
    super::super::components::local(async {
        let harness = Harness::new(false).await;
        harness.reply(200, &tokens("access-2", "refresh-2", 3600));
        let sign_in = get_oauth_provider("gitlab-duo").unwrap();
        let refreshed = sign_in
            .refresh_token(stored("refresh/1 é"), None)
            .await
            .unwrap();
        let exchange = request(&harness, 0);
        assert_eq!(exchange["url"], "https://gitlab.com/oauth/token");
        assert_eq!(exchange["method"], "POST");
        assert_eq!(
            exchange["headers"],
            json!([["Content-Type", "application/x-www-form-urlencoded"]])
        );
        assert_eq!(
            exchange["body"],
            format!(
                "client_id={CLIENT_ID}&grant_type=refresh_token&refresh_token=refresh%2F1+%C3%A9"
            )
        );
        assert_eq!(
            (
                refreshed.access.as_str(),
                refreshed.refresh.as_str(),
                refreshed.expires
            ),
            ("access-2", "refresh-2", 1_700_003_300_000.0)
        );
        harness.bus.clear();
    });
}

#[test]
fn custom_gateway_provider_token_failures_report_the_response_text() {
    super::super::components::local(async {
        let harness = Harness::new(false).await;
        let sign_in = get_oauth_provider("gitlab-duo").unwrap();
        harness.reply(400, &json!("invalid_grant"));
        let interaction = Answering::new(|state| {
            format!("http://127.0.0.1:8080/callback?code=granted&state={state}")
        });
        assert!(
            sign_in
                .login(interaction, None)
                .await
                .unwrap_err()
                .to_string()
                .contains("Token exchange failed: invalid_grant")
        );
        harness.reply(400, &json!("invalid_grant"));
        assert!(
            sign_in
                .refresh_token(stored("r"), None)
                .await
                .unwrap_err()
                .to_string()
                .contains("Token refresh failed: invalid_grant")
        );
        harness.reply_unreadable(400, "body interrupted");
        assert!(
            sign_in
                .refresh_token(stored("r"), None)
                .await
                .unwrap_err()
                .to_string()
                .contains("Token refresh failed: body interrupted")
        );
        harness.bus.clear();
    });
}

/// The `Authorization` header the last Anthropic request was delegated with.
fn delegated_authorization(harness: &Harness) -> Value {
    harness.anthropic.calls.lock().unwrap().last().unwrap()["common"]["headers"]["Authorization"]
        .clone()
}

/// Streams one greeting with the supplied GitLab credential and checks that it completed.
async fn greet(harness: &Harness, credential: &str) {
    let (events, _) = harness.stream(DEFAULT_MODEL, keyed(Some(credential))).await;
    assert_eq!(types(&events).last(), Some(&"done"));
}

#[test]
fn custom_gateway_provider_direct_access_follows_the_supplied_credential() {
    super::super::components::local(async {
        let harness = Harness::new(false).await;
        for (index, credential) in ["account-a", "account-b", "account-a"]
            .into_iter()
            .enumerate()
        {
            let token = format!("direct-{index}");
            harness.reply(200, &grant(&token));
            greet(&harness, credential).await;
            assert_eq!(received(&harness), index + 1, "{credential}");
            assert_eq!(
                request(&harness, index)["headers"][0][1],
                format!("Bearer {credential}")
            );
            assert_eq!(delegated_authorization(&harness), format!("Bearer {token}"));
        }
        harness.bus.clear();
    });
}

#[test]
fn custom_gateway_provider_new_credentials_expire_the_direct_access_token() {
    super::super::components::local(async {
        let harness = Harness::new(false).await;
        let sign_in = get_oauth_provider("gitlab-duo").unwrap();
        harness.reply(200, &grant("direct-1"));
        greet(&harness, "glpat-user").await;
        greet(&harness, "glpat-user").await;
        assert_eq!(received(&harness), 1, "the first grant is reused");
        harness.reply(200, &tokens("access-2", "refresh-2", 3600));
        sign_in.refresh_token(stored("r"), None).await.unwrap();
        harness.reply(200, &grant("direct-2"));
        greet(&harness, "glpat-user").await;
        assert_eq!(
            received(&harness),
            3,
            "rotated credentials ask for a new grant"
        );
        assert_eq!(delegated_authorization(&harness), "Bearer direct-2");
        harness.reply(400, &json!("invalid_grant"));
        assert!(sign_in.refresh_token(stored("r"), None).await.is_err());
        greet(&harness, "glpat-user").await;
        assert_eq!(received(&harness), 4, "a failed refresh keeps the grant");
        assert_eq!(delegated_authorization(&harness), "Bearer direct-2");
        harness.bus.clear();
    });
}

#[test]
fn custom_gateway_provider_unreadable_direct_access_answers_name_the_cause() {
    super::super::components::local(async {
        let harness = Harness::new(false).await;
        harness.reply_unreadable(502, "body interrupted");
        let (events, result) = harness
            .stream(DEFAULT_MODEL, keyed(Some("glpat-user")))
            .await;
        assert_eq!(types(&events), ["error"]);
        assert_eq!(
            result["errorMessage"],
            "Failed to get direct access token: 502 body interrupted"
        );
        harness.bus.clear();
    });
}
