//! The Hugging Face model browser, signed in with the saved access token.

use std::collections::HashMap;
use std::sync::Mutex;

use futures_util::StreamExt;
use lettuce_model_hub::{
    HUGGING_FACE_ENDPOINT, HfAuthStatus, HfAuthorOverview, HfBrowseError, HfBrowseMode,
    HfModelInfo, HfRequest, HfResource, HfSearch, HfSearchResult,
};
use lettuce_network::{
    JsonAuth, JsonClient, JsonClientError, JsonQueryParameter, JsonResponse, RequestPolicy,
};
use lettuce_settings::{
    SecretPurpose, SecretRecord, SecretRef, SecretState, SecretStore, SecretStoreError, SecretValue,
};

const AVATAR_CONCURRENCY: usize = 6;

/// Browses Hugging Face; avatars are cached for the browser's lifetime.
#[derive(Debug)]
pub struct HuggingFaceBrowser {
    client: JsonClient,
    endpoint: String,
    avatars: Mutex<HashMap<String, String>>,
}

fn message(text: impl Into<String>) -> HfBrowseError {
    HfBrowseError::Message(text.into())
}

fn token_reference() -> (SecretRef, SecretPurpose) {
    let purpose = SecretPurpose::HuggingFaceAccessToken;
    let reference = purpose
        .app_secret_ref()
        .expect("the Hugging Face token has a fixed reference");
    (reference, purpose)
}

fn store_error(error: &SecretStoreError) -> HfBrowseError {
    message(format!("The Hugging Face token could not be read: {error}"))
}

async fn present_generation<S: SecretStore + ?Sized>(
    secrets: &S,
    reference: &SecretRef,
    purpose: &SecretPurpose,
) -> Result<Option<u64>, HfBrowseError> {
    let status = secrets
        .status(reference, purpose)
        .await
        .map_err(|error| store_error(&error))?;
    match status.state {
        SecretState::Present => Ok(Some(status.generation)),
        SecretState::Missing => Ok(None),
        SecretState::Unavailable { reason } => {
            Err(store_error(&SecretStoreError::Unavailable(reason)))
        }
    }
}

fn bearer(token: &str) -> Result<SecretValue, HfBrowseError> {
    if token
        .bytes()
        .any(|byte| byte.is_ascii_control() || !byte.is_ascii())
    {
        return Err(message("The Hugging Face token is malformed."));
    }
    SecretValue::new(token).map_err(|_| message("The Hugging Face token is malformed."))
}

impl HuggingFaceBrowser {
    #[must_use]
    pub fn new(client: JsonClient) -> Self {
        Self::with_endpoint(client, HUGGING_FACE_ENDPOINT)
    }

    #[must_use]
    pub fn with_endpoint(client: JsonClient, endpoint: impl Into<String>) -> Self {
        Self {
            client,
            endpoint: endpoint.into(),
            avatars: Mutex::new(HashMap::new()),
        }
    }

    async fn saved_token<S: SecretStore + ?Sized>(
        secrets: &S,
    ) -> Result<Option<SecretValue>, HfBrowseError> {
        let (reference, purpose) = token_reference();
        match secrets.load(&reference, &purpose).await {
            Ok(value) => value.with(|token| {
                let token = token.trim();
                if token.is_empty() {
                    Ok(None)
                } else {
                    bearer(token)
                        .map(Some)
                        .map_err(|_| message("The saved Hugging Face token is malformed."))
                }
            }),
            Err(SecretStoreError::Missing) => Ok(None),
            Err(error) => Err(store_error(&error)),
        }
    }

    async fn get(
        &self,
        request: &HfRequest,
        token: Option<&SecretValue>,
    ) -> Result<JsonResponse, JsonClientError> {
        let query = request
            .query
            .iter()
            .map(|(name, value)| JsonQueryParameter { name, value })
            .collect::<Vec<_>>();
        let auth = token.map_or(JsonAuth::None, |token| {
            token
                .with(|token| SecretValue::new(token))
                .map_or(JsonAuth::None, JsonAuth::Bearer)
        });
        self.client
            .get_json_with_query(
                &self.endpoint,
                &request.path,
                &query,
                &[lettuce_network::JsonStaticHeader {
                    name: "user-agent",
                    value: "LettuceAI/1.0",
                }],
                auth,
                Vec::new(),
                RequestPolicy::PROBE,
            )
            .await
    }

    async fn whoami(&self, token: &SecretValue) -> Result<String, HfBrowseError> {
        let response = self
            .get(&lettuce_model_hub::whoami_request(), Some(token))
            .await
            .map_err(|error| {
                message(format!(
                    "Could not validate the Hugging Face token: {error}"
                ))
            })?;
        lettuce_model_hub::whoami_username(
            response.status,
            &lettuce_network::status_text(response.status),
            &response.body,
        )
    }

    /// Whether a token is saved and still accepted.
    pub async fn auth_status<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
    ) -> Result<HfAuthStatus, HfBrowseError> {
        let Some(token) = Self::saved_token(secrets).await? else {
            return Ok(HfAuthStatus::missing());
        };
        Ok(match self.whoami(&token).await {
            Ok(username) => HfAuthStatus::valid(username),
            Err(_) => HfAuthStatus::invalid(),
        })
    }

    /// Saves the token once Hugging Face accepts it.
    pub async fn save_token<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        token: &str,
    ) -> Result<HfAuthStatus, HfBrowseError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(message("Enter a Hugging Face token."));
        }
        let value = bearer(token)?;
        let (reference, purpose) = token_reference();
        let expected = present_generation(secrets, &reference, &purpose).await?;
        let username = self.whoami(&value).await?;
        secrets
            .put(SecretRecord::new(reference, purpose), value, expected)
            .await
            .map_err(|error| {
                message(format!(
                    "The Hugging Face token could not be saved: {error}"
                ))
            })?;
        Ok(HfAuthStatus::valid(username))
    }

    pub async fn clear_token<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
    ) -> Result<(), HfBrowseError> {
        let (reference, purpose) = token_reference();
        let Some(generation) = present_generation(secrets, &reference, &purpose).await? else {
            return Ok(());
        };
        match secrets.delete(&reference, &purpose, Some(generation)).await {
            Ok(_) | Err(SecretStoreError::Missing) => Ok(()),
            Err(error) => Err(message(format!(
                "The Hugging Face token could not be removed: {error}"
            ))),
        }
    }

    async fn list(
        &self,
        request: &HfRequest,
        token: Option<&SecretValue>,
    ) -> Result<Vec<HfSearchResult>, HfBrowseError> {
        let response = self
            .get(request, token)
            .await
            .map_err(|error| message(format!("HuggingFace API request failed: {error}")))?;
        if let Some(error) =
            lettuce_model_hub::access_error(response.status, HfResource::List, "", token.is_some())
        {
            return Err(error);
        }
        if !(200..300).contains(&response.status) {
            return Err(message(format!(
                "HuggingFace API error {}: {}",
                lettuce_network::status_text(response.status),
                String::from_utf8_lossy(&response.body)
            )));
        }
        lettuce_model_hub::parse_model_list(&response.body)
    }

    pub async fn search<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        search: &HfSearch,
    ) -> Result<Vec<HfSearchResult>, HfBrowseError> {
        let token = Self::saved_token(secrets).await?;
        let plan = lettuce_model_hub::search_plan(search);
        let mut lists = Vec::with_capacity(plan.requests.len());
        for request in &plan.requests {
            lists.push(self.list(request, token.as_ref()).await?);
        }
        Ok(lettuce_model_hub::merge_search_results(&plan, lists))
    }

    pub async fn author_models<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        author: &str,
        search: Option<&str>,
        limit: Option<u32>,
        sort: Option<&str>,
        offset: Option<u32>,
    ) -> Result<Vec<HfSearchResult>, HfBrowseError> {
        let request =
            lettuce_model_hub::author_models_request(author, search, limit, sort, offset)?;
        let token = Self::saved_token(secrets).await?;
        self.list(&request, token.as_ref()).await
    }

    /// The author as a user, else as an organization.
    pub async fn author_overview<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        author: &str,
    ) -> Result<HfAuthorOverview, HfBrowseError> {
        let requests = lettuce_model_hub::author_overview_requests(author)?;
        let token = Self::saved_token(secrets).await?;
        let mut last_error = String::new();
        for request in &requests {
            match self.get(request, token.as_ref()).await {
                Ok(response) if (200..300).contains(&response.status) => {
                    match lettuce_model_hub::parse_author_overview(author, &response.body) {
                        Ok(overview) => return Ok(overview),
                        Err(error) => last_error = error.to_string(),
                    }
                }
                Ok(response) => {
                    last_error = format!(
                        "{}{} -> {}",
                        self.endpoint,
                        request.path,
                        lettuce_network::status_text(response.status)
                    );
                }
                Err(error) => last_error = error.to_string(),
            }
        }
        Err(message(format!(
            "Failed to fetch author overview for {}: {last_error}",
            author.trim()
        )))
    }

    async fn avatar(&self, author: &str, token: Option<&SecretValue>) -> String {
        for request in lettuce_model_hub::avatar_requests(author) {
            if let Ok(response) = self.get(&request, token).await
                && (200..300).contains(&response.status)
                && let Some(url) = lettuce_model_hub::parse_avatar(&response.body)
            {
                return url;
            }
        }
        String::new()
    }

    /// Each author's avatar URL, empty when neither lookup found one.
    pub async fn avatars<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        authors: &[String],
    ) -> Result<HashMap<String, String>, HfBrowseError> {
        let mut result = HashMap::new();
        let mut missing: Vec<&str> = Vec::new();
        {
            let cache = self
                .avatars
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            for author in authors {
                if let Some(url) = cache.get(author) {
                    result.insert(author.clone(), url.clone());
                } else if !missing.contains(&author.as_str()) {
                    missing.push(author);
                }
            }
        }
        if !missing.is_empty() {
            let token = Self::saved_token(secrets).await?;
            let fetched = futures_util::stream::iter(missing)
                .map(|author| {
                    let token = token.as_ref();
                    async move { (author.to_owned(), self.avatar(author, token).await) }
                })
                .buffer_unordered(AVATAR_CONCURRENCY)
                .collect::<Vec<_>>()
                .await;
            let mut cache = self
                .avatars
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            for (author, url) in fetched {
                cache.insert(author.clone(), url.clone());
                result.insert(author, url);
            }
        }
        Ok(result)
    }

    /// The repository's downloadable files with sizes and the GGUF summary.
    pub async fn model_files<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        model_id: &str,
        mode: HfBrowseMode,
    ) -> Result<HfModelInfo, HfBrowseError> {
        let token = Self::saved_token(secrets).await?;
        let detail = self
            .get(
                &lettuce_model_hub::model_detail_request(model_id),
                token.as_ref(),
            )
            .await
            .map_err(|error| message(format!("Failed to fetch model detail: {error}")))?;
        if let Some(error) = lettuce_model_hub::access_error(
            detail.status,
            HfResource::Model,
            model_id,
            token.is_some(),
        ) {
            return Err(error);
        }
        if !(200..300).contains(&detail.status) {
            return Err(message(format!(
                "Model not found ({}): {model_id}",
                lettuce_network::status_text(detail.status)
            )));
        }
        let tree = self
            .get(
                &lettuce_model_hub::model_tree_request(model_id, mode),
                token.as_ref(),
            )
            .await
            .map_err(|error| message(format!("Failed to fetch file tree: {error}")))?;
        if let Some(error) = lettuce_model_hub::access_error(
            tree.status,
            HfResource::Repository,
            model_id,
            token.is_some(),
        ) {
            return Err(error);
        }
        let tree = (200..300)
            .contains(&tree.status)
            .then_some(tree.body.as_slice());
        lettuce_model_hub::model_info(model_id, &detail.body, tree, mode)
    }

    /// The model card without its front matter.
    pub async fn readme<S: SecretStore + ?Sized>(
        &self,
        secrets: &S,
        model_id: &str,
    ) -> Result<String, HfBrowseError> {
        let token = Self::saved_token(secrets).await?;
        let response = self
            .get(&lettuce_model_hub::readme_request(model_id), token.as_ref())
            .await
            .map_err(|error| message(format!("Failed to fetch README: {error}")))?;
        if let Some(error) = lettuce_model_hub::access_error(
            response.status,
            HfResource::Model,
            model_id,
            token.is_some(),
        ) {
            return Err(error);
        }
        if !(200..300).contains(&response.status) {
            return Err(message(format!(
                "README not found (HTTP {})",
                response.status
            )));
        }
        Ok(lettuce_model_hub::readme_body(&String::from_utf8_lossy(
            &response.body,
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lettuce_settings::InMemorySecretStore;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    async fn server(responses: Vec<(u16, &'static str)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = seen.clone();
        tokio::spawn(async move {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.expect("accept");
                let mut bytes = Vec::new();
                let mut buffer = [0_u8; 4096];
                while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut buffer).await.expect("read");
                    if read == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..read]);
                }
                captured
                    .lock()
                    .expect("seen")
                    .push(String::from_utf8_lossy(&bytes).into_owned());
                let response = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.expect("write");
            }
        });
        (format!("http://{address}"), seen)
    }

    fn browser(endpoint: String) -> HuggingFaceBrowser {
        HuggingFaceBrowser::with_endpoint(JsonClient::new().expect("client"), endpoint)
    }

    #[tokio::test]
    async fn a_token_is_saved_only_after_hugging_face_accepts_it() {
        let secrets = InMemorySecretStore::default();
        let (endpoint, seen) = server(vec![
            (401, "{}"),
            (200, r#"{"name": "ada"}"#),
            (200, r#"{"name": "ada"}"#),
        ])
        .await;
        let browser = browser(endpoint);
        assert_eq!(
            browser.auth_status(&secrets).await.expect("status"),
            HfAuthStatus::missing()
        );
        assert_eq!(
            browser.save_token(&secrets, " bad ").await,
            Err(message("The Hugging Face token is invalid or expired."))
        );
        assert_eq!(
            browser.auth_status(&secrets).await.expect("status"),
            HfAuthStatus::missing()
        );
        assert_eq!(
            browser.save_token(&secrets, " good ").await,
            Ok(HfAuthStatus::valid("ada".to_owned()))
        );
        assert_eq!(
            browser.auth_status(&secrets).await.expect("status"),
            HfAuthStatus::valid("ada".to_owned())
        );
        let requests = seen.lock().expect("seen").clone();
        assert!(requests[1].contains("GET /api/whoami-v2 "));
        assert!(
            requests[2]
                .to_lowercase()
                .contains("authorization: bearer good\r\n")
        );
        browser.clear_token(&secrets).await.expect("clear");
        browser.clear_token(&secrets).await.expect("clear twice");
        assert_eq!(
            browser.auth_status(&secrets).await.expect("status"),
            HfAuthStatus::missing()
        );
        assert_eq!(
            browser.save_token(&secrets, "  ").await,
            Err(message("Enter a Hugging Face token."))
        );
    }

    #[tokio::test]
    async fn gated_models_and_missing_readmes_show_the_old_texts() {
        let secrets = InMemorySecretStore::default();
        let (endpoint, seen) = server(vec![
            (
                200,
                r#"{"modelId": "org/m", "siblings": [{"rfilename": "m-Q4_0.gguf"}]}"#,
            ),
            (500, "boom"),
            (403, "{}"),
            (200, "---\ntags: []\n---\n# Card"),
            (404, ""),
            (200, r#"[{"modelId": "org/a"}]"#),
        ])
        .await;
        let browser = browser(endpoint);
        let info = browser
            .model_files(&secrets, "org/m", HfBrowseMode::Llm)
            .await
            .expect("files");
        assert_eq!(info.files.len(), 1);
        assert_eq!(info.files[0].size, 0);
        assert_eq!(
            browser.readme(&secrets, "org/m").await,
            Err(message(
                "Accept access to org/m on Hugging Face, then retry."
            ))
        );
        assert_eq!(
            browser.readme(&secrets, "org/m").await.as_deref(),
            Ok("# Card")
        );
        assert_eq!(
            browser.readme(&secrets, "org/m").await,
            Err(message("README not found (HTTP 404)"))
        );
        let found = browser
            .author_models(&secrets, " org ", None, None, None, None)
            .await
            .expect("author models");
        assert_eq!(found[0].model_id, "org/a");
        let requests = seen.lock().expect("seen").clone();
        assert!(requests[1].starts_with("GET /api/models/org/m/tree/main?recursive=false "));
        assert!(requests[5].starts_with(
            "GET /api/models?author=org&filter=gguf&limit=50&sort=downloads&direction=-1&offset=0 "
        ));
    }
}
