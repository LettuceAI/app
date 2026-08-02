//! Sync codec wire-format round-trip tests.
//!
//! These pin the framing contract: anything one peer encodes the other peer
//! must decode to an equal value, both with and without encryption. They
//! also exercise the partial-frame buffering path (Decoder must wait for a
//! full frame before yielding) and reject malformed frames.

use bytes::BytesMut;
use lettuceai_lib::sync::codec::P2PCodec;
use lettuceai_lib::sync::protocol::P2PMessage;
use tokio_util::codec::{Decoder, Encoder};

fn sample_handshake() -> P2PMessage {
    P2PMessage::Handshake {
        protocol_version: 1,
        app_version: "1.2.0".into(),
        device_name: "test-device".into(),
        device_id: "dev-1234".into(),
        salt: [7u8; 16],
        challenge: [11u8; 16],
    }
}

fn assert_matches_handshake(msg: &P2PMessage) {
    match msg {
        P2PMessage::Handshake {
            protocol_version,
            app_version,
            device_name,
            device_id,
            salt,
            challenge,
        } => {
            assert_eq!(*protocol_version, 1);
            assert_eq!(app_version, "1.2.0");
            assert_eq!(device_name, "test-device");
            assert_eq!(device_id, "dev-1234");
            assert_eq!(*salt, [7u8; 16]);
            assert_eq!(*challenge, [11u8; 16]);
        }
        other => panic!("expected Handshake, got {other:?}"),
    }
}

#[test]
fn plaintext_roundtrip_yields_identical_message() {
    let mut encoder = P2PCodec::new();
    let mut buf = BytesMut::new();
    encoder
        .encode(sample_handshake(), &mut buf)
        .expect("encode should succeed");

    let mut decoder = P2PCodec::new();
    let decoded = decoder
        .decode(&mut buf)
        .expect("decode should succeed")
        .expect("must yield a frame");
    assert_matches_handshake(&decoded);
}

#[test]
fn encrypted_roundtrip_yields_identical_message() {
    let key = [42u8; 32];
    let mut encoder = P2PCodec::new();
    encoder.set_key(&key);
    let mut buf = BytesMut::new();
    encoder
        .encode(sample_handshake(), &mut buf)
        .expect("encode");

    let mut decoder = P2PCodec::new();
    decoder.set_key(&key);
    let decoded = decoder.decode(&mut buf).expect("decode").expect("frame");
    assert_matches_handshake(&decoded);
}

#[test]
fn encrypted_frame_with_wrong_key_fails_to_decrypt() {
    let mut encoder = P2PCodec::new();
    encoder.set_key(&[1u8; 32]);
    let mut buf = BytesMut::new();
    encoder
        .encode(sample_handshake(), &mut buf)
        .expect("encode");

    let mut decoder = P2PCodec::new();
    decoder.set_key(&[2u8; 32]);
    let result = decoder.decode(&mut buf);
    assert!(
        result.is_err(),
        "decryption with wrong key must surface as an error, not a wrong message"
    );
}

#[test]
fn decoder_returns_none_for_partial_length_prefix() {
    let mut decoder = P2PCodec::new();
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&[0u8, 0u8, 0u8]); // only 3 of 4 length bytes
    let result = decoder.decode(&mut buf).expect("no error on partial");
    assert!(result.is_none(), "partial length must yield None");
    assert_eq!(
        buf.len(),
        3,
        "decoder must leave the partial bytes in place"
    );
}

#[test]
fn decoder_returns_none_for_partial_payload() {
    let mut encoder = P2PCodec::new();
    let mut full = BytesMut::new();
    encoder
        .encode(sample_handshake(), &mut full)
        .expect("encode");

    // Truncate before the payload is complete.
    let truncated_len = full.len() / 2;
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&full[..truncated_len]);

    let mut decoder = P2PCodec::new();
    let result = decoder.decode(&mut buf).expect("no error");
    assert!(result.is_none(), "incomplete payload must yield None");
}

#[test]
fn decoder_consumes_only_one_frame_per_call() {
    // Encode two messages back-to-back; decoder should yield them in order
    // across two `decode` calls without consuming the second's bytes early.
    let mut encoder = P2PCodec::new();
    let mut buf = BytesMut::new();
    encoder.encode(sample_handshake(), &mut buf).expect("e1");
    encoder.encode(sample_handshake(), &mut buf).expect("e2");

    let mut decoder = P2PCodec::new();
    let first = decoder.decode(&mut buf).expect("d1").expect("frame 1");
    assert_matches_handshake(&first);
    let second = decoder.decode(&mut buf).expect("d2").expect("frame 2");
    assert_matches_handshake(&second);
    assert!(buf.is_empty(), "all bytes should be consumed");
}

#[test]
fn frame_too_large_length_is_rejected_without_buffering() {
    // Craft a length prefix bigger than the 100 MB cap.
    let mut decoder = P2PCodec::new();
    let mut buf = BytesMut::new();
    let huge_len: u32 = 200 * 1024 * 1024;
    buf.extend_from_slice(&huge_len.to_be_bytes());
    let result = decoder.decode(&mut buf);
    assert!(
        result.is_err(),
        "a length above MAX_FRAME_SIZE must be rejected, not buffered"
    );
}

#[test]
fn plaintext_decoder_cannot_decode_encrypted_frame() {
    // An encrypted frame fed into a plaintext decoder will be interpreted as
    // bincode bytes and should fail to deserialize.
    let mut encoder = P2PCodec::new();
    encoder.set_key(&[5u8; 32]);
    let mut buf = BytesMut::new();
    encoder.encode(sample_handshake(), &mut buf).expect("e");

    let mut plain_decoder = P2PCodec::new();
    let result = plain_decoder.decode(&mut buf);
    assert!(
        result.is_err() || result.unwrap().is_none(),
        "plaintext decoder must not silently mis-parse encrypted bytes"
    );
}

#[test]
fn drip_feed_one_byte_at_a_time_decodes() {
    // Simulate a hostile network that delivers the frame one byte per `decode`
    // call. The decoder must accumulate state across calls and yield exactly
    // once the full frame is present.
    let mut encoder = P2PCodec::new();
    let mut full = BytesMut::new();
    encoder
        .encode(sample_handshake(), &mut full)
        .expect("encode");

    let mut decoder = P2PCodec::new();
    let mut acc = BytesMut::new();
    let mut decoded = None;
    for byte in &full[..] {
        acc.extend_from_slice(&[*byte]);
        match decoder.decode(&mut acc).expect("no error") {
            Some(m) => {
                decoded = Some(m);
                break;
            }
            None => continue,
        }
    }
    let msg = decoded.expect("eventually decodes");
    assert_matches_handshake(&msg);
}
