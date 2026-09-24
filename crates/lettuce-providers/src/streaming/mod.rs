pub(crate) mod stream_framing;
pub(crate) mod stream_normalize;
#[expect(
    clippy::module_inception,
    reason = "the stream consumer belongs to the streaming domain"
)]
pub(crate) mod streaming;
