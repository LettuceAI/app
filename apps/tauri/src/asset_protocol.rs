use lettuce_app::api::{self, ApiContext};
use lettuce_contracts::ApiErrorCode;
use tauri::{
    Manager, Runtime, UriSchemeContext, UriSchemeResponder,
    http::{Request, Response, StatusCode, Uri, header::CONTENT_TYPE},
};

pub(crate) const ASSET_SCHEME: &str = "lettuce-asset";

/// Serves `lettuce-asset://localhost/<asset_id>` (on Windows and Android
/// `http://lettuce-asset.localhost/<asset_id>`, which `convertFileSrc(id,
/// "lettuce-asset")` builds) with the asset's bytes and MIME type.
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
    tauri::async_runtime::spawn(async move {
        let response = match api::read_asset(&api_context, &asset_id).await {
            Ok(asset) => Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, asset.mime_type)
                .body(asset.bytes)
                .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
            Err(error) => {
                tracing::debug!(code = ?error.code, message = %error.message, "asset request failed");
                status(match error.code {
                    ApiErrorCode::NotFound => StatusCode::NOT_FOUND,
                    ApiErrorCode::InvalidInput => StatusCode::BAD_REQUEST,
                    ApiErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                })
            }
        };
        responder.respond(response);
    });
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
}
