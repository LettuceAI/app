use lettuce_app::api::{self, ApiContext, AssetRange, AssetRead};
use lettuce_contracts::{ApiError, ApiErrorCode};
use tauri::{
    Manager, Runtime, UriSchemeContext, UriSchemeResponder,
    http::{
        Request, Response, StatusCode, Uri,
        header::{
            ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
            X_CONTENT_TYPE_OPTIONS,
        },
    },
};

pub(crate) const ASSET_SCHEME: &str = "lettuce-asset";

/// Where the webview reaches the asset scheme; every `AssetRef::url` is this
/// followed by the asset id.
#[cfg(any(windows, target_os = "android"))]
pub(crate) const ASSET_URL_BASE: &str = "http://lettuce-asset.localhost/";
#[cfg(not(any(windows, target_os = "android")))]
pub(crate) const ASSET_URL_BASE: &str = "lettuce-asset://localhost/";

/// Serves `ASSET_URL_BASE<asset_id>` with the asset's bytes and MIME type.
/// A single `Range` is answered with that part (an open-ended one with at
/// most `OPEN_RANGE_CHUNK` bytes); any other request gets the whole asset.
pub(crate) fn handle<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let Some(api_context) = context
        .app_handle()
        .try_state::<ApiContext>()
        .map(|state| state.inner().clone())
    else {
        responder.respond(status(StatusCode::SERVICE_UNAVAILABLE));
        return;
    };
    let asset_id = asset_id(request.uri());
    let range = request
        .headers()
        .get(RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_range)
        .unwrap_or(AssetRange::Whole);
    tauri::async_runtime::spawn(async move {
        let read = api::read_asset(&api_context, &asset_id, range).await;
        responder.respond(response(read, range != AssetRange::Whole));
    });
}

/// A single `bytes=a-b`, `bytes=a-` or `bytes=-n` range; anything else is
/// ignored and the whole asset is served.
fn parse_range(header: &str) -> Option<AssetRange> {
    let spec = header.trim().strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let (start, end) = (start.trim(), end.trim());
    let number = |value: &str| {
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| value.parse::<u64>().ok())
            .flatten()
    };
    match (start.is_empty(), end.is_empty()) {
        (true, true) => None,
        (true, false) => Some(AssetRange::Last { len: number(end)? }),
        (false, true) => Some(AssetRange::From {
            start: number(start)?,
        }),
        (false, false) => {
            let (start, end) = (number(start)?, number(end)?);
            (start <= end).then_some(AssetRange::Between { start, end })
        }
    }
}

fn response(read: Result<AssetRead, ApiError>, ranged: bool) -> Response<Vec<u8>> {
    let builder = Response::builder()
        .header(ACCEPT_RANGES, "bytes")
        .header(X_CONTENT_TYPE_OPTIONS, "nosniff");
    let built = match read {
        Ok(AssetRead::Bytes(asset)) => {
            let len = asset.bytes.len() as u64;
            let builder = builder
                .header(CONTENT_TYPE, asset.mime_type)
                .header(CONTENT_LENGTH, len);
            if ranged {
                builder
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header(
                        CONTENT_RANGE,
                        format!(
                            "bytes {}-{}/{}",
                            asset.start,
                            (asset.start + len).saturating_sub(1),
                            asset.total_len
                        ),
                    )
                    .body(asset.bytes)
            } else {
                builder.status(StatusCode::OK).body(asset.bytes)
            }
        }
        Ok(AssetRead::Unsatisfiable { total_len }) => builder
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(CONTENT_RANGE, format!("bytes */{total_len}"))
            .header(CONTENT_LENGTH, 0)
            .body(Vec::new()),
        Err(error) => {
            tracing::debug!(code = ?error.code, message = %error.message, "asset request failed");
            builder
                .status(match error.code {
                    ApiErrorCode::NotFound => StatusCode::NOT_FOUND,
                    ApiErrorCode::InvalidInput => StatusCode::BAD_REQUEST,
                    ApiErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                })
                .header(CONTENT_LENGTH, 0)
                .body(Vec::new())
        }
    };
    built.unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

/// The last path segment, else the host (`lettuce-asset://<asset_id>`).
fn asset_id(uri: &Uri) -> String {
    uri.path()
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .or_else(|| uri.host())
        .unwrap_or_default()
        .to_owned()
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    response
}

#[cfg(test)]
mod tests {
    use lettuce_app::api::AssetBytes;

    use super::*;

    #[test]
    fn asset_ids_come_from_the_path_or_the_host() {
        for (uri, expected) in [
            ("lettuce-asset://localhost/abc", "abc"),
            ("http://lettuce-asset.localhost/abc", "abc"),
            ("lettuce-asset://abc", "abc"),
            ("lettuce-asset://localhost/abc/", "abc"),
        ] {
            assert_eq!(asset_id(&uri.parse().expect("uri")), expected);
        }
    }

    #[test]
    fn single_byte_ranges_parse_and_anything_else_is_ignored() {
        for (header, expected) in [
            (
                "bytes=0-99",
                Some(AssetRange::Between { start: 0, end: 99 }),
            ),
            (
                " bytes= 10 - 20 ",
                Some(AssetRange::Between { start: 10, end: 20 }),
            ),
            ("bytes=100-", Some(AssetRange::From { start: 100 })),
            ("bytes=-500", Some(AssetRange::Last { len: 500 })),
            ("bytes=-", None),
            ("bytes=20-10", None),
            ("bytes=0-1,5-6", None),
            ("bytes=a-b", None),
            ("bytes=+1-2", None),
            ("items=0-1", None),
            ("bytes=99999999999999999999-", None),
        ] {
            assert_eq!(parse_range(header), expected, "{header}");
        }
    }

    #[test]
    fn ranges_resolve_against_the_asset_size() {
        assert_eq!(AssetRange::Whole.resolve(10), Some((0, 10)));
        assert_eq!(
            AssetRange::Between { start: 2, end: 4 }.resolve(10),
            Some((2, 3))
        );
        assert_eq!(
            AssetRange::Between { start: 8, end: 400 }.resolve(10),
            Some((8, 2))
        );
        assert_eq!(AssetRange::Between { start: 10, end: 12 }.resolve(10), None);
        assert_eq!(AssetRange::From { start: 3 }.resolve(10), Some((3, 7)));
        assert_eq!(AssetRange::From { start: 10 }.resolve(10), None);
        assert_eq!(
            AssetRange::From { start: 1 }.resolve(api::OPEN_RANGE_CHUNK * 3),
            Some((1, api::OPEN_RANGE_CHUNK))
        );
        assert_eq!(AssetRange::Last { len: 4 }.resolve(10), Some((6, 4)));
        assert_eq!(AssetRange::Last { len: 40 }.resolve(10), Some((0, 10)));
        assert_eq!(AssetRange::Last { len: 0 }.resolve(10), None);
        assert_eq!(AssetRange::Last { len: 1 }.resolve(0), None);
    }

    fn header<'a>(response: &'a Response<Vec<u8>>, name: &str) -> Option<&'a str> {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    }

    #[test]
    fn partial_reads_answer_206_with_their_content_range() {
        let response = response(
            Ok(AssetRead::Bytes(AssetBytes {
                mime_type: "image/png".into(),
                total_len: 10,
                start: 2,
                bytes: vec![1, 2, 3],
            })),
            true,
        );
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&response, "content-range"), Some("bytes 2-4/10"));
        assert_eq!(header(&response, "content-length"), Some("3"));
        assert_eq!(header(&response, "content-type"), Some("image/png"));
        assert_eq!(header(&response, "accept-ranges"), Some("bytes"));
        assert_eq!(header(&response, "x-content-type-options"), Some("nosniff"));
        assert_eq!(response.body(), &vec![1, 2, 3]);
    }

    #[test]
    fn whole_reads_answer_200_and_unsatisfiable_ranges_416() {
        let whole = response(
            Ok(AssetRead::Bytes(AssetBytes {
                mime_type: "image/webp".into(),
                total_len: 2,
                start: 0,
                bytes: vec![7, 8],
            })),
            false,
        );
        assert_eq!(whole.status(), StatusCode::OK);
        assert_eq!(header(&whole, "content-length"), Some("2"));
        assert_eq!(header(&whole, "content-range"), None);
        let unsatisfiable = response(Ok(AssetRead::Unsatisfiable { total_len: 2 }), true);
        assert_eq!(unsatisfiable.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(header(&unsatisfiable, "content-range"), Some("bytes */2"));
    }

    #[test]
    fn bad_and_unknown_ids_answer_400_and_404() {
        let error = |code| ApiError {
            code,
            message: String::new(),
            details: None,
        };
        assert_eq!(
            response(Err(error(ApiErrorCode::InvalidInput)), false).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            response(Err(error(ApiErrorCode::NotFound)), true).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            response(Err(error(ApiErrorCode::Unavailable)), false).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
