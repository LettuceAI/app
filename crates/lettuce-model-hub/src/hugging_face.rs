//! The Hugging Face model browser: request shapes, response parsing and the
//! error texts the app shows for each response.

use serde::Deserialize;

pub const HUGGING_FACE_ENDPOINT: &str = "https://huggingface.co";

/// One GET against the Hugging Face API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfRequest {
    pub path: String,
    pub query: Vec<(String, String)>,
}

impl HfRequest {
    fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            query: Vec::new(),
        }
    }

    fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.query.push((name.to_owned(), value.into()));
        self
    }

    fn with_author_and_search(self, author: Option<&str>, search: &str) -> Self {
        let request = match author.map(str::trim).filter(|author| !author.is_empty()) {
            Some(author) => self.with("author", author),
            None => self,
        };
        if search.is_empty() {
            request
        } else {
            request.with("search", search)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HfBrowseMode {
    Llm,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfSearch {
    pub query: String,
    pub limit: Option<u32>,
    pub sort: Option<String>,
    pub offset: Option<u32>,
    pub author: Option<String>,
    pub mode: HfBrowseMode,
    pub unfiltered: bool,
}

/// A search is one or more list requests whose results are merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfSearchPlan {
    pub requests: Vec<HfRequest>,
    pub merge: HfMerge,
    pub limit: usize,
    pub sort: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HfMerge {
    /// Every list's new models in order, sorted by the sort field and cut to
    /// the limit.
    SortedUnion,
    /// The first list, then the others' new models appended.
    Appended,
}

#[must_use]
pub fn search_plan(search: &HfSearch) -> HfSearchPlan {
    let limit = search.limit.unwrap_or(20).min(100);
    let sort = search
        .sort
        .clone()
        .unwrap_or_else(|| "trendingScore".to_owned());
    let offset = search.offset.unwrap_or(0);
    let query = search.query.trim();
    let author = search.author.as_deref();
    if !search.unfiltered && search.mode == HfBrowseMode::Image {
        let mut requests = ["text-to-image", "image-to-image"]
            .into_iter()
            .map(|task| {
                HfRequest::new("/api/models")
                    .with("pipeline_tag", task)
                    .with("limit", limit.to_string())
                    .with("sort", sort.clone())
                    .with("direction", "-1")
                    .with("offset", offset.to_string())
                    .with_author_and_search(author, query)
            })
            .collect::<Vec<_>>();
        if !query.is_empty() {
            requests.push(
                HfRequest::new("/api/models")
                    .with("limit", limit.to_string())
                    .with("sort", sort.clone())
                    .with("direction", "-1")
                    .with("offset", offset.to_string())
                    .with_author_and_search(author, query),
            );
        }
        return HfSearchPlan {
            requests,
            merge: HfMerge::SortedUnion,
            limit: limit as usize,
            sort,
        };
    }
    let list = |filtered: bool| {
        let request = HfRequest::new("/api/models");
        let request = if filtered {
            request.with("filter", "gguf")
        } else {
            request
        };
        request
            .with("limit", limit.to_string())
            .with("sort", sort.clone())
            .with("offset", offset.to_string())
            .with_author_and_search(author, query)
    };
    let mut requests = vec![list(!search.unfiltered)];
    if !search.unfiltered && query.contains('/') && !query.contains(char::is_whitespace) {
        requests.push(list(false));
    }
    HfSearchPlan {
        requests,
        merge: HfMerge::Appended,
        limit: limit as usize,
        sort,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HfSearchResult {
    pub model_id: String,
    pub author: String,
    pub likes: i64,
    pub downloads: i64,
    pub tags: Vec<String>,
    pub pipeline_tag: Option<String>,
    pub last_modified: Option<String>,
    pub trending_score: Option<f64>,
}

#[derive(Deserialize)]
struct ModelEntry {
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(default)]
    likes: i64,
    #[serde(default)]
    downloads: i64,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    pipeline_tag: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default, rename = "trendingScore")]
    trending_score: Option<f64>,
}

fn author_of(author: Option<String>, model_id: &str) -> String {
    author.unwrap_or_else(|| model_id.split('/').next().unwrap_or("unknown").to_owned())
}

pub fn parse_model_list(body: &[u8]) -> Result<Vec<HfSearchResult>, HfBrowseError> {
    let entries: Vec<ModelEntry> = serde_json::from_slice(body).map_err(|error| {
        HfBrowseError::Message(format!("Failed to parse HuggingFace response: {error}"))
    })?;
    Ok(entries
        .into_iter()
        .map(|entry| HfSearchResult {
            author: author_of(entry.author, &entry.model_id),
            model_id: entry.model_id,
            likes: entry.likes,
            downloads: entry.downloads,
            tags: entry.tags,
            pipeline_tag: entry.pipeline_tag,
            last_modified: entry.last_modified,
            trending_score: entry.trending_score,
        })
        .collect())
}

/// The plan's lists merged like the old browser merged them.
#[must_use]
pub fn merge_search_results(
    plan: &HfSearchPlan,
    lists: Vec<Vec<HfSearchResult>>,
) -> Vec<HfSearchResult> {
    let mut merged: Vec<HfSearchResult> = Vec::new();
    for result in lists.into_iter().flatten() {
        if !merged
            .iter()
            .any(|existing| existing.model_id == result.model_id)
        {
            merged.push(result);
        }
    }
    if plan.merge == HfMerge::SortedUnion {
        match plan.sort.as_str() {
            "likes" => merged.sort_by_key(|result| std::cmp::Reverse(result.likes)),
            "lastModified" => merged.sort_by(|a, b| b.last_modified.cmp(&a.last_modified)),
            "trendingScore" => merged.sort_by(|a, b| {
                b.trending_score
                    .partial_cmp(&a.trending_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            _ => merged.sort_by_key(|result| std::cmp::Reverse(result.downloads)),
        }
        merged.truncate(plan.limit);
    }
    merged
}

/// An author's GGUF models.
pub fn author_models_request(
    author: &str,
    search: Option<&str>,
    limit: Option<u32>,
    sort: Option<&str>,
    offset: Option<u32>,
) -> Result<HfRequest, HfBrowseError> {
    let author = author.trim();
    if author.is_empty() {
        return Err(HfBrowseError::Message("author is empty".to_owned()));
    }
    let request = HfRequest::new("/api/models")
        .with("author", author)
        .with("filter", "gguf")
        .with("limit", limit.unwrap_or(50).min(100).to_string())
        .with("sort", sort.unwrap_or("downloads"))
        .with("direction", "-1")
        .with("offset", offset.unwrap_or(0).to_string());
    Ok(
        match search.map(str::trim).filter(|search| !search.is_empty()) {
            Some(search) => request.with("search", search),
            None => request,
        },
    )
}

/// The user overview, then the organization overview.
pub fn author_overview_requests(author: &str) -> Result<[HfRequest; 2], HfBrowseError> {
    let author = author.trim();
    if author.is_empty() {
        return Err(HfBrowseError::Message("author is empty".to_owned()));
    }
    Ok([
        HfRequest::new(format!("/api/users/{}/overview", path_segment(author))),
        HfRequest::new(format!(
            "/api/organizations/{}/overview",
            path_segment(author)
        )),
    ])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfAuthorOverview {
    pub name: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
    pub details: Option<String>,
    pub kind: Option<String>,
    pub is_pro: bool,
    pub num_models: u64,
    pub num_datasets: u64,
    pub num_spaces: u64,
    pub num_likes: u64,
    pub num_followers: u64,
    pub num_following: u64,
    pub created_at: Option<String>,
}

#[derive(Deserialize, Default)]
struct OverviewResponse {
    #[serde(default, alias = "user")]
    name: Option<String>,
    #[serde(default)]
    fullname: Option<String>,
    #[serde(default, rename = "avatarUrl")]
    avatar_url: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default, rename = "isPro")]
    is_pro: Option<bool>,
    #[serde(default, rename = "numModels")]
    num_models: Option<u64>,
    #[serde(default, rename = "numDatasets")]
    num_datasets: Option<u64>,
    #[serde(default, rename = "numSpaces")]
    num_spaces: Option<u64>,
    #[serde(default, rename = "numLikes")]
    num_likes: Option<u64>,
    #[serde(default, rename = "numFollowers")]
    num_followers: Option<u64>,
    #[serde(default, rename = "numFollowing")]
    num_following: Option<u64>,
    #[serde(default, rename = "createdAt")]
    created_at: Option<String>,
}

pub fn parse_author_overview(
    author: &str,
    body: &[u8],
) -> Result<HfAuthorOverview, serde_json::Error> {
    let raw: OverviewResponse = serde_json::from_slice(body)?;
    Ok(HfAuthorOverview {
        name: raw.name.unwrap_or_else(|| author.trim().to_owned()),
        fullname: raw.fullname,
        avatar_url: raw.avatar_url,
        details: raw.details,
        kind: raw.kind,
        is_pro: raw.is_pro.unwrap_or(false),
        num_models: raw.num_models.unwrap_or(0),
        num_datasets: raw.num_datasets.unwrap_or(0),
        num_spaces: raw.num_spaces.unwrap_or(0),
        num_likes: raw.num_likes.unwrap_or(0),
        num_followers: raw.num_followers.unwrap_or(0),
        num_following: raw.num_following.unwrap_or(0),
        created_at: raw.created_at,
    })
}

fn path_segment(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// The organization avatar, then the user avatar.
#[must_use]
pub fn avatar_requests(author: &str) -> [HfRequest; 2] {
    [
        HfRequest::new(format!(
            "/api/organizations/{}/avatar",
            path_segment(author)
        )),
        HfRequest::new(format!("/api/users/{}/avatar", path_segment(author))),
    ]
}

#[must_use]
pub fn parse_avatar(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Avatar {
        #[serde(rename = "avatarUrl")]
        avatar_url: String,
    }
    serde_json::from_slice::<Avatar>(body)
        .ok()
        .map(|avatar| avatar.avatar_url)
}

#[must_use]
pub fn model_detail_request(model_id: &str) -> HfRequest {
    HfRequest::new(format!("/api/models/{model_id}"))
}

/// A bundle role search: the 100 top models for `sort`, optionally by
/// author, query and library filter.
#[must_use]
pub fn bundle_search_request(
    sort: &str,
    author: Option<&str>,
    query: &str,
    filter: Option<&str>,
) -> HfRequest {
    let request = HfRequest::new("/api/models")
        .with("limit", "100")
        .with("sort", sort)
        .with("direction", "-1")
        .with_author_and_search(author, query);
    match filter {
        Some(filter) => request.with("filter", filter),
        None => request,
    }
}

/// A repository's metadata as bundle assembly reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct HfRepoDetail {
    pub model_id: String,
    pub sha: Option<String>,
    pub gated: bool,
    pub tags: Vec<String>,
    pub pipeline_tag: Option<String>,
    pub card_data: serde_json::Value,
    pub likes: i64,
    pub downloads: i64,
    pub last_modified: Option<String>,
    pub trending_score: Option<f64>,
}

impl HfRepoDetail {
    #[must_use]
    pub fn search_result(&self) -> HfSearchResult {
        HfSearchResult {
            model_id: self.model_id.clone(),
            author: self
                .model_id
                .split('/')
                .next()
                .unwrap_or("unknown")
                .to_owned(),
            likes: self.likes,
            downloads: self.downloads,
            tags: self.tags.clone(),
            pipeline_tag: self.pipeline_tag.clone(),
            last_modified: self.last_modified.clone(),
            trending_score: self.trending_score,
        }
    }
}

#[derive(Deserialize)]
struct RepoDetail {
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(default)]
    sha: Option<String>,
    #[serde(default)]
    gated: serde_json::Value,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    pipeline_tag: Option<String>,
    #[serde(default, rename = "cardData")]
    card_data: serde_json::Value,
    #[serde(default)]
    likes: i64,
    #[serde(default)]
    downloads: i64,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default, rename = "trendingScore")]
    trending_score: Option<f64>,
}

pub fn parse_repo_detail(body: &[u8]) -> Result<HfRepoDetail, serde_json::Error> {
    let detail: RepoDetail = serde_json::from_slice(body)?;
    Ok(HfRepoDetail {
        model_id: detail.model_id,
        sha: detail.sha,
        gated: !detail.gated.is_null() && detail.gated != serde_json::Value::Bool(false),
        tags: detail.tags,
        pipeline_tag: detail.pipeline_tag,
        card_data: detail.card_data,
        likes: detail.likes,
        downloads: detail.downloads,
        last_modified: detail.last_modified,
        trending_score: detail.trending_score,
    })
}

/// Every file of a repository at a revision.
#[must_use]
pub fn repo_tree_request(model_id: &str, revision: &str) -> HfRequest {
    HfRequest::new(format!("/api/models/{model_id}/tree/{revision}")).with("recursive", "true")
}

/// A repository tree entry; `sha256` is the LFS object id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfTreeEntry {
    pub is_file: bool,
    pub path: String,
    pub size: u64,
    pub sha256: Option<String>,
}

pub fn parse_repo_tree(body: &[u8]) -> Result<Vec<HfTreeEntry>, serde_json::Error> {
    #[derive(Deserialize)]
    struct Entry {
        #[serde(rename = "type")]
        entry_type: String,
        path: String,
        #[serde(default)]
        size: u64,
        #[serde(default)]
        lfs: Option<Lfs>,
    }
    #[derive(Deserialize)]
    struct Lfs {
        #[serde(default)]
        oid: Option<String>,
        #[serde(default)]
        size: u64,
    }
    let entries: Vec<Entry> = serde_json::from_slice(body)?;
    Ok(entries
        .into_iter()
        .map(|entry| HfTreeEntry {
            is_file: entry.entry_type == "file",
            size: entry.lfs.as_ref().map_or(entry.size, |lfs| lfs.size),
            sha256: entry.lfs.and_then(|lfs| lfs.oid),
            path: entry.path,
        })
        .collect())
}

/// The repository's current revision with every file's size and digest.
#[must_use]
pub fn model_pin_request(model_id: &str) -> HfRequest {
    model_detail_request(model_id).with("blobs", "true")
}

/// The repository at `revision` with every file's size and digest.
#[must_use]
pub fn model_revision_pin_request(model_id: &str, revision: &str) -> HfRequest {
    HfRequest::new(format!("/api/models/{model_id}/revision/{revision}")).with("blobs", "true")
}

/// A repository file pinned for download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfPinnedFile {
    pub path: String,
    pub size: u64,
    pub sha256: Option<String>,
    /// The git blob id of a file stored without LFS, which checks its
    /// content once downloaded (`verify_git_blob`).
    pub git_blob_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfPinnedFiles {
    pub revision: String,
    pub files: Vec<HfPinnedFile>,
}

/// A pin response as Hugging Face sends it: the revision and every listed
/// file, unvalidated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfPinListing {
    pub revision: String,
    /// `None` when the response has no `siblings` field.
    pub siblings: Option<Vec<HfListedFile>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfListedFile {
    pub path: String,
    pub size: Option<u64>,
    pub blob_id: Option<String>,
    pub lfs: Option<HfLfsObject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfLfsObject {
    pub size: u64,
    pub sha256: String,
}

#[derive(Deserialize)]
struct PinDetail {
    sha: String,
    #[serde(default, deserialize_with = "present_siblings")]
    siblings: Option<Vec<PinSibling>>,
}

fn present_siblings<'de, D>(deserializer: D) -> Result<Option<Vec<PinSibling>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
struct PinSibling {
    rfilename: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default, rename = "blobId")]
    blob_id: Option<String>,
    #[serde(default)]
    lfs: Option<PinLfs>,
}

#[derive(Deserialize)]
struct PinLfs {
    size: u64,
    sha256: String,
}

/// Parses a `model_pin_request` or `model_revision_pin_request` response.
pub fn parse_pin_listing(body: &[u8]) -> Result<HfPinListing, serde_json::Error> {
    let detail: PinDetail = serde_json::from_slice(body)?;
    Ok(HfPinListing {
        revision: detail.sha,
        siblings: detail.siblings.map(|siblings| {
            siblings
                .into_iter()
                .map(|sibling| HfListedFile {
                    path: sibling.rfilename,
                    size: sibling.size,
                    blob_id: sibling.blob_id,
                    lfs: sibling.lfs.map(|lfs| HfLfsObject {
                        size: lfs.size,
                        sha256: lfs.sha256,
                    }),
                })
                .collect()
        }),
    })
}

/// `filenames` of `model_id` at the revision the pin response names, with the
/// size and SHA-256 Hugging Face lists for each, or the git blob id of a file
/// stored without LFS.
pub fn pinned_files(
    model_id: &str,
    body: &[u8],
    filenames: &[&str],
) -> Result<HfPinnedFiles, HfBrowseError> {
    let listing = parse_pin_listing(body).map_err(|error| {
        HfBrowseError::Message(format!("Failed to parse model detail: {error}"))
    })?;
    if listing.revision.len() != 40
        || !listing
            .revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(HfBrowseError::Message(format!(
            "Hugging Face returned no revision for {model_id}."
        )));
    }
    let siblings = listing.siblings.unwrap_or_default();
    let files = filenames
        .iter()
        .map(|filename| {
            let sibling = siblings
                .iter()
                .find(|sibling| sibling.path == *filename)
                .ok_or_else(|| {
                    HfBrowseError::Message(format!("{filename} is not in {model_id}."))
                })?;
            let (size, sha256, git_blob_id) = match &sibling.lfs {
                Some(lfs) => (lfs.size, Some(lfs.sha256.to_ascii_lowercase()), None),
                None => (
                    sibling.size.unwrap_or(0),
                    None,
                    sibling
                        .blob_id
                        .as_deref()
                        .filter(|id| {
                            id.len() == 40 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
                        })
                        .map(str::to_ascii_lowercase),
                ),
            };
            if size == 0 {
                return Err(HfBrowseError::Message(format!(
                    "Hugging Face lists no size for {filename}."
                )));
            }
            Ok(HfPinnedFile {
                path: (*filename).to_owned(),
                size,
                sha256,
                git_blob_id,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HfPinnedFiles {
        revision: listing.revision.to_ascii_lowercase(),
        files,
    })
}

const MAX_RESOLVE_SEGMENT_BYTES: usize = 256;
const MAX_RESOLVE_FILENAME_BYTES: usize = 1024;
const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the Hugging Face file reference is invalid")]
pub struct HfInvalidFileReference;

/// The download URL of `filename` in `repository` at `revision`, a branch or
/// commit.
pub fn resolve_url(
    repository: &str,
    revision: &str,
    filename: &str,
) -> Result<String, HfInvalidFileReference> {
    if !valid_repository(repository)
        || !valid_path_segment(revision)
        || !valid_artifact_filename(filename)
    {
        return Err(HfInvalidFileReference);
    }
    Ok(resolve_url_unchecked(repository, revision, filename))
}

/// `resolve_url` for a full 40-hex commit id.
pub fn pinned_resolve_url(
    repository: &str,
    commit: &str,
    filename: &str,
) -> Result<String, HfInvalidFileReference> {
    if !valid_repository(repository)
        || commit.len() != 40
        || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !valid_artifact_filename(filename)
    {
        return Err(HfInvalidFileReference);
    }
    Ok(resolve_url_unchecked(repository, commit, filename))
}

/// Joins the segments as a WHATWG URL path-segment setter does: `.` and `..`
/// segments are dropped and every other segment is percent-encoded with the
/// special-scheme path-segment set.
fn resolve_url_unchecked(repository: &str, revision: &str, filename: &str) -> String {
    let segments = repository
        .split('/')
        .chain(["resolve", revision])
        .chain(filename.split('/'))
        .filter(|segment| !matches!(*segment, "." | ".."));
    let mut url = format!("{HUGGING_FACE_ENDPOINT}/");
    for (index, segment) in segments.enumerate() {
        if index > 0 {
            url.push('/');
        }
        for byte in segment.bytes() {
            if !(0x20..0x7f).contains(&byte)
                || matches!(
                    byte,
                    b' ' | b'"'
                        | b'<'
                        | b'>'
                        | b'`'
                        | b'#'
                        | b'?'
                        | b'{'
                        | b'}'
                        | b'/'
                        | b'%'
                        | b'\\'
                )
            {
                url.push('%');
                url.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
                url.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
            } else {
                url.push(char::from(byte));
            }
        }
    }
    url
}

fn valid_repository(repository: &str) -> bool {
    let mut segments = repository.split('/');
    matches!((segments.next(), segments.next(), segments.next()), (Some(owner), Some(name), None) if valid_path_segment(owner) && valid_path_segment(name))
}

fn valid_artifact_filename(filename: &str) -> bool {
    filename.len() <= MAX_RESOLVE_FILENAME_BYTES
        && filename.split('/').all(|segment| {
            !segment.is_empty()
                && segment.len() <= MAX_RESOLVE_SEGMENT_BYTES
                && segment != "."
                && segment != ".."
                && !segment.contains(['\\', ':'])
                && !segment.ends_with(['.', ' '])
                && !segment.chars().any(char::is_control)
        })
}

fn valid_path_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RESOLVE_SEGMENT_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// `value` without a leading `https://huggingface.co/`.
#[must_use]
pub fn strip_endpoint_prefix(value: &str) -> &str {
    value
        .strip_prefix(HUGGING_FACE_ENDPOINT)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(value)
}

#[must_use]
pub fn model_tree_request(model_id: &str, mode: HfBrowseMode) -> HfRequest {
    HfRequest::new(format!("/api/models/{model_id}/tree/main")).with(
        "recursive",
        if mode == HfBrowseMode::Image {
            "true"
        } else {
            "false"
        },
    )
}

#[must_use]
pub fn readme_request(model_id: &str) -> HfRequest {
    HfRequest::new(format!("/{model_id}/raw/main/README.md"))
}

#[must_use]
pub fn whoami_request() -> HfRequest {
    HfRequest::new("/api/whoami-v2")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfModelFile {
    pub filename: String,
    pub size: u64,
    pub quantization: String,
    pub is_mmproj: bool,
    pub is_mtp: bool,
    pub imatrix: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfModelInfo {
    pub model_id: String,
    pub author: String,
    pub likes: i64,
    pub downloads: i64,
    pub tags: Vec<String>,
    pub architecture: Option<String>,
    pub context_length: Option<u64>,
    pub parameter_count: Option<u64>,
    pub files: Vec<HfModelFile>,
}

#[derive(Deserialize)]
struct ModelDetail {
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(default)]
    likes: i64,
    #[serde(default)]
    downloads: i64,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    siblings: Vec<Sibling>,
    #[serde(default)]
    gguf: Option<GgufSummary>,
}

#[derive(Deserialize)]
struct Sibling {
    rfilename: String,
}

#[derive(Deserialize, Default)]
struct GgufSummary {
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    architecture: Option<String>,
    #[serde(default)]
    context_length: Option<u64>,
}

#[derive(Deserialize)]
struct TreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    lfs: Option<TreeLfs>,
}

#[derive(Deserialize)]
struct TreeLfs {
    #[serde(default)]
    size: u64,
}

/// The repository's GGUF files (and, for image models, safetensors), smallest
/// first, sized from the file tree; `tree` is `None` when the tree could not
/// be read.
pub fn model_info(
    model_id: &str,
    detail: &[u8],
    tree: Option<&[u8]>,
    mode: HfBrowseMode,
) -> Result<HfModelInfo, HfBrowseError> {
    let detail: ModelDetail = serde_json::from_slice(detail).map_err(|error| {
        HfBrowseError::Message(format!("Failed to parse model detail: {error}"))
    })?;
    let sizes = tree
        .and_then(|tree| serde_json::from_slice::<Vec<TreeEntry>>(tree).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| entry.entry_type == "file")
        .map(|entry| {
            let size = entry.lfs.as_ref().map_or(entry.size, |lfs| lfs.size);
            (entry.path, size)
        })
        .collect::<std::collections::HashMap<_, _>>();
    let image = mode == HfBrowseMode::Image;
    let mut files = detail
        .siblings
        .iter()
        .filter(|sibling| {
            let lower = sibling.rfilename.to_lowercase();
            !is_imatrix_data(&sibling.rfilename)
                && (lower.ends_with(".gguf")
                    || (image && (lower.ends_with(".safetensors") || lower.ends_with(".sft"))))
        })
        .map(|sibling| HfModelFile {
            filename: sibling.rfilename.clone(),
            size: sizes.get(&sibling.rfilename).copied().unwrap_or(0),
            quantization: extract_quantization(&sibling.rfilename),
            is_mmproj: sibling.rfilename.to_lowercase().contains("mmproj"),
            is_mtp: is_mtp_asset(&sibling.rfilename),
            imatrix: is_imatrix_quant(&sibling.rfilename),
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|file| file.size);
    let gguf = detail.gguf.unwrap_or_default();
    Ok(HfModelInfo {
        author: author_of(detail.author, model_id),
        model_id: detail.model_id,
        likes: detail.likes,
        downloads: detail.downloads,
        tags: detail.tags,
        architecture: gguf.architecture,
        context_length: gguf.context_length,
        parameter_count: gguf.total,
        files,
    })
}

/// The README without its YAML front matter.
#[must_use]
pub fn readme_body(raw: &str) -> String {
    if let Some(stripped) = raw.strip_prefix("---")
        && let Some(end) = stripped.find("---")
    {
        return stripped[end + 3..]
            .trim_start_matches('\n')
            .trim_start_matches('\r')
            .to_owned();
    }
    raw.to_owned()
}

const QUANTIZATIONS: [&str; 52] = [
    "IQ1_S",
    "IQ1_M",
    "IQ2_XXS",
    "IQ2_XS",
    "IQ2_S",
    "IQ2_M",
    "IQ3_XXS",
    "IQ3_XS",
    "IQ3_S",
    "IQ3_M",
    "IQ4_XS",
    "IQ4_NL",
    "Q2_K_S",
    "Q2_K_M",
    "Q2_K_L",
    "Q2_K_XL",
    "Q2_K",
    "Q3_K_S",
    "Q3_K_M",
    "Q3_K_L",
    "Q3_K_XL",
    "Q3_K",
    "Q4_K_S",
    "Q4_K_M",
    "Q4_K_L",
    "Q4_K_XL",
    "Q4_K",
    "Q4_0",
    "Q4_1",
    "Q5_K_S",
    "Q5_K_M",
    "Q5_K_L",
    "Q5_K_XL",
    "Q5_K",
    "Q5_0",
    "Q5_1",
    "Q6_K_S",
    "Q6_K_L",
    "Q6_K_XL",
    "Q6_K",
    "Q8_K_S",
    "Q8_K_L",
    "Q8_K_XL",
    "Q8_K",
    "Q8_0",
    "TQ1_0",
    "TQ2_0",
    "MXFP4_MOE",
    "MXFP4",
    "BF16",
    "F16",
    "F32",
];

/// The quantization named in a GGUF file name, `UD-` prefixed for Unsloth
/// dynamic quants (`UD-Q4_K_XL`, `UD-IQ2_M`, `UD-TQ1_0`), else `Unknown`.
#[must_use]
pub fn extract_quantization(filename: &str) -> String {
    let upper = filename.to_uppercase();
    QUANTIZATIONS
        .iter()
        .find(|pattern| upper.contains(*pattern))
        .map_or_else(
            || "Unknown".to_owned(),
            |pattern| {
                if upper.contains(&format!("UD-{pattern}")) {
                    format!("UD-{pattern}")
                } else {
                    (*pattern).to_owned()
                }
            },
        )
}

/// A quant made with an importance matrix, as its file name says: an `i1`
/// part (`model.i1-Q4_K_M.gguf`) or an `imat`/`imatrix` part next to a known
/// quantization. The importance matrix data file itself is not a quant.
#[must_use]
pub fn is_imatrix_quant(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    extract_quantization(filename) != "Unknown"
        && basename
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|part| part == "i1" || part == "imat" || part == "imatrix")
}

fn is_imatrix_data(filename: &str) -> bool {
    filename.to_lowercase().contains("imatrix") && extract_quantization(filename) == "Unknown"
}

/// A multi-token-prediction head shipped next to a model.
#[must_use]
pub fn is_mtp_asset(name: &str) -> bool {
    let lower = name.to_lowercase();
    let basename = lower.rsplit('/').next().unwrap_or(&lower);
    basename.starts_with("mtp-")
        || basename.contains("-mtp.")
        || basename.contains("_mtp.")
        || lower.contains("/mtp/")
        || lower.starts_with("mtp/")
}

/// Whether a saved token works, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfAuthStatus {
    pub saved: bool,
    pub valid: bool,
    pub username: Option<String>,
    pub error_kind: Option<HfAuthErrorKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HfAuthErrorKind {
    MissingToken,
    InvalidOrExpired,
}

impl HfAuthStatus {
    #[must_use]
    pub const fn missing() -> Self {
        Self {
            saved: false,
            valid: false,
            username: None,
            error_kind: Some(HfAuthErrorKind::MissingToken),
        }
    }

    #[must_use]
    pub const fn invalid() -> Self {
        Self {
            saved: true,
            valid: false,
            username: None,
            error_kind: Some(HfAuthErrorKind::InvalidOrExpired),
        }
    }

    #[must_use]
    pub const fn valid(username: String) -> Self {
        Self {
            saved: true,
            valid: true,
            username: Some(username),
            error_kind: None,
        }
    }
}

/// The account name a whoami response names, or the text shown when the
/// token was refused.
pub fn whoami_username(
    status: u16,
    status_text: &str,
    body: &[u8],
) -> Result<String, HfBrowseError> {
    let message = match status {
        401 => "The Hugging Face token is invalid or expired.".to_owned(),
        200..=299 => {
            #[derive(Deserialize)]
            struct WhoAmI {
                name: Option<String>,
            }
            return serde_json::from_slice::<WhoAmI>(body)
                .map_err(|error| {
                    HfBrowseError::Message(format!(
                        "Could not read the Hugging Face account: {error}"
                    ))
                })?
                .name
                .ok_or_else(|| {
                    HfBrowseError::Message(
                        "Hugging Face did not return an account name.".to_owned(),
                    )
                });
        }
        _ => format!("Hugging Face token validation failed with status {status_text}."),
    };
    Err(HfBrowseError::Message(message))
}

/// Which request a status error came from; each shows its own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HfResource {
    List,
    Model,
    Repository,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HfBrowseError {
    #[error("{0}")]
    Message(String),
}

/// The text for an unauthorized or forbidden response, `None` for others.
#[must_use]
pub fn access_error(
    status: u16,
    resource: HfResource,
    model_id: &str,
    token_saved: bool,
) -> Option<HfBrowseError> {
    let message = match status {
        401 if token_saved => "The saved Hugging Face token is invalid or expired.".to_owned(),
        401 => match resource {
            HfResource::List => "This Hugging Face file requires an access token.",
            HfResource::Model => "This Hugging Face model requires an access token.",
            HfResource::Repository => "This Hugging Face repository requires an access token.",
        }
        .to_owned(),
        403 => match resource {
            HfResource::List => {
                "Accept the gated model license on Hugging Face, then retry.".to_owned()
            }
            HfResource::Model | HfResource::Repository => {
                format!("Accept access to {model_id} on Hugging Face, then retry.")
            }
        },
        _ => return None,
    };
    Some(HfBrowseError::Message(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(request: &HfRequest) -> Vec<(&str, &str)> {
        request
            .query
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect()
    }

    #[test]
    fn searches_follow_the_old_browser() {
        let plan = search_plan(&HfSearch {
            query: " unsloth/Qwen ".to_owned(),
            limit: Some(500),
            sort: None,
            offset: None,
            author: Some(" ".to_owned()),
            mode: HfBrowseMode::Llm,
            unfiltered: false,
        });
        assert_eq!(plan.requests.len(), 2);
        assert_eq!(
            query(&plan.requests[0]),
            vec![
                ("filter", "gguf"),
                ("limit", "100"),
                ("sort", "trendingScore"),
                ("offset", "0"),
                ("search", "unsloth/Qwen")
            ]
        );
        assert_eq!(
            plan.requests[1].query[0],
            ("limit".to_owned(), "100".to_owned())
        );
        let image = search_plan(&HfSearch {
            query: String::new(),
            limit: None,
            sort: Some("likes".to_owned()),
            offset: Some(20),
            author: Some("stabilityai".to_owned()),
            mode: HfBrowseMode::Image,
            unfiltered: false,
        });
        assert_eq!(image.requests.len(), 2);
        assert_eq!(image.merge, HfMerge::SortedUnion);
        let merged = merge_search_results(
            &image,
            vec![
                parse_model_list(br#"[{"modelId": "a/one", "likes": 1}]"#).expect("list"),
                parse_model_list(
                    br#"[{"modelId": "a/one", "likes": 1}, {"modelId": "b/two", "likes": 9}]"#,
                )
                .expect("list"),
            ],
        );
        assert_eq!(
            merged
                .iter()
                .map(|result| (result.model_id.as_str(), result.author.as_str()))
                .collect::<Vec<_>>(),
            vec![("b/two", "b"), ("a/one", "a")]
        );
    }

    #[test]
    fn model_files_are_sized_from_the_tree_and_named_by_quantization() {
        let info = model_info(
            "org/model",
            br#"{"modelId": "org/model", "siblings": [
                {"rfilename": "model-UD-Q4_K_M.gguf"},
                {"rfilename": "mmproj-F16.gguf"},
                {"rfilename": "imatrix.gguf"},
                {"rfilename": "imatrix_unsloth.dat"},
                {"rfilename": "model.i1-IQ3_M.gguf"},
                {"rfilename": "mtp/draft-Q8_0.gguf"},
                {"rfilename": "README.md"}
            ], "gguf": {"total": 7, "architecture": "llama", "context_length": 8192}}"#,
            Some(br#"[{"type": "file", "path": "model-UD-Q4_K_M.gguf", "size": 1, "lfs": {"size": 500}},
                     {"type": "file", "path": "mmproj-F16.gguf", "size": 100}]"#),
            HfBrowseMode::Llm,
        )
        .expect("info");
        assert_eq!(info.author, "org");
        assert_eq!(info.parameter_count, Some(7));
        let files = info
            .files
            .iter()
            .map(|file| {
                (
                    file.filename.as_str(),
                    file.size,
                    file.quantization.as_str(),
                    file.is_mmproj,
                    file.is_mtp,
                    file.imatrix,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            files,
            vec![
                ("model.i1-IQ3_M.gguf", 0, "IQ3_M", false, false, true),
                ("mtp/draft-Q8_0.gguf", 0, "Q8_0", false, true, false),
                ("mmproj-F16.gguf", 100, "F16", true, false, false),
                (
                    "model-UD-Q4_K_M.gguf",
                    500,
                    "UD-Q4_K_M",
                    false,
                    false,
                    false
                ),
            ]
        );
    }

    #[test]
    fn quantizations_name_unsloth_dynamic_and_importance_matrix_quants() {
        for (filename, quantization, imatrix) in [
            ("Qwen3-30B-A3B-UD-Q4_K_XL.gguf", "UD-Q4_K_XL", false),
            (
                "UD-IQ1_S/DeepSeek-R1-UD-IQ1_S-00001-of-00003.gguf",
                "UD-IQ1_S",
                false,
            ),
            ("Kimi-K2-UD-TQ1_0.gguf", "UD-TQ1_0", false),
            ("gpt-oss-20b-mxfp4.gguf", "MXFP4", false),
            ("model-BF16.gguf", "BF16", false),
            ("model-f16.gguf", "F16", false),
            ("Model.i1-Q4_K_M.gguf", "Q4_K_M", true),
            ("model-Q5_K_S-imat.gguf", "Q5_K_S", true),
            ("model-imatrix-Q6_K.gguf", "Q6_K", true),
            ("imatrix.gguf", "Unknown", false),
        ] {
            assert_eq!(extract_quantization(filename), quantization, "{filename}");
            assert_eq!(is_imatrix_quant(filename), imatrix, "{filename}");
        }
    }

    #[test]
    fn downloads_are_pinned_to_the_listed_revision_size_and_digest() {
        let body = br#"{"sha": "D24C4CF2A0CD98A42F23467E27E3D76EE9438B8E", "siblings": [
            {"rfilename": "m-Q4_K_M.gguf", "size": 5, "blobId": "0123456789012345678901234567890123456789", "lfs": {"size": 5, "sha256": "AB"}},
            {"rfilename": "config.json", "size": 12, "blobId": "CE013625030BA8DBA906F756967F9E9CA394464A"}
        ]}"#;
        let pinned =
            pinned_files("org/m", body, &["m-Q4_K_M.gguf", "config.json"]).expect("pinned");
        assert_eq!(pinned.revision, "d24c4cf2a0cd98a42f23467e27e3d76ee9438b8e");
        assert_eq!(pinned.files[0].sha256.as_deref(), Some("ab"));
        assert_eq!(pinned.files[1].sha256, None);
        assert_eq!(pinned.files[0].git_blob_id, None);
        assert_eq!(
            pinned.files[1].git_blob_id.as_deref(),
            Some("ce013625030ba8dba906f756967f9e9ca394464a")
        );
        assert_eq!(
            pinned_files("org/m", body, &["missing.gguf"]),
            Err(HfBrowseError::Message(
                "missing.gguf is not in org/m.".to_owned()
            ))
        );
        assert_eq!(query(&model_pin_request("org/m")), vec![("blobs", "true")]);
    }

    #[test]
    fn pin_listings_keep_every_sibling_as_listed() {
        let listing = parse_pin_listing(
            br#"{"sha": "AB", "siblings": [
                {"rfilename": "a.bin", "size": 5, "lfs": {"size": 5, "sha256": "CD"}},
                {"rfilename": "b.json", "blobId": "EF"}
            ]}"#,
        )
        .expect("listing");
        assert_eq!(listing.revision, "AB");
        let siblings = listing.siblings.expect("siblings");
        assert_eq!(
            siblings[0].lfs,
            Some(HfLfsObject {
                size: 5,
                sha256: "CD".to_owned()
            })
        );
        assert_eq!(siblings[1].size, None);
        assert_eq!(siblings[1].blob_id.as_deref(), Some("EF"));
        assert_eq!(
            parse_pin_listing(br#"{"sha": "AB"}"#)
                .expect("listing")
                .siblings,
            None
        );
        assert!(parse_pin_listing(br#"{"sha": "AB", "siblings": null}"#).is_err());
        let request = model_revision_pin_request("org/m", "abc");
        assert_eq!(request.path, "/api/models/org/m/revision/abc");
        assert_eq!(query(&request), vec![("blobs", "true")]);
    }

    #[test]
    fn resolve_urls_validate_and_encode_like_a_url_path_setter() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            pinned_resolve_url("ggerganov/whisper.cpp", commit, "ggml-base.bin"),
            Ok(format!(
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/{commit}/ggml-base.bin"
            ))
        );
        assert_eq!(
            pinned_resolve_url("org/m", "main", "a.bin"),
            Err(HfInvalidFileReference)
        );
        assert_eq!(
            resolve_url("org/m", "main", "dir/a b#?%2e{}ü.gguf"),
            Ok(
                "https://huggingface.co/org/m/resolve/main/dir/a%20b%23%3F%252e%7B%7D%C3%BC.gguf"
                    .to_owned()
            )
        );
        assert_eq!(
            resolve_url("../m", "..", "a.bin"),
            Ok("https://huggingface.co/m/resolve/a.bin".to_owned())
        );
        assert!(valid_repository("ggerganov/whisper.cpp"));
        assert!(!valid_repository("ggerganov/whisper.cpp/extra"));
        assert!(!valid_artifact_filename("../ggml-base.bin"));
        assert!(valid_artifact_filename("onnx/model_quantized.onnx"));
        assert!(valid_artifact_filename("split_files/vae/ae.safetensors"));
        assert!(!valid_artifact_filename("split_files/../ae.safetensors"));
        assert!(!valid_artifact_filename("/ae.safetensors"));
        assert!(!valid_artifact_filename("split_files//ae.safetensors"));
        assert_eq!(
            strip_endpoint_prefix("https://huggingface.co/org/m"),
            "org/m"
        );
        assert_eq!(strip_endpoint_prefix("org/m"), "org/m");
    }

    #[test]
    fn readmes_lose_their_front_matter_and_errors_name_the_resource() {
        assert_eq!(readme_body("---\nlicense: mit\n---\n# Model"), "# Model");
        assert_eq!(readme_body("# Plain"), "# Plain");
        assert_eq!(
            access_error(403, HfResource::Model, "org/m", false),
            Some(HfBrowseError::Message(
                "Accept access to org/m on Hugging Face, then retry.".to_owned()
            ))
        );
        assert_eq!(access_error(404, HfResource::List, "", true), None);
        assert_eq!(
            whoami_username(200, "200 OK", br#"{"name": "ada"}"#),
            Ok("ada".to_owned())
        );
        assert_eq!(
            whoami_username(200, "200 OK", b"{}"),
            Err(HfBrowseError::Message(
                "Hugging Face did not return an account name.".to_owned()
            ))
        );
    }
}
