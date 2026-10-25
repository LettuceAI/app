//! Byte-oriented framing for provider response streams. Records are not
//! size-capped: image-output models stream a whole base64 picture as one
//! record.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamFormat {
    Sse,
    Ndjson,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StreamRecord {
    pub(crate) event: Option<String>,
    pub(crate) data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FramingError {
    #[error("stream record contains invalid UTF-8")]
    InvalidUtf8,
    #[error("stream record exceeds the {0}-byte limit")]
    RecordTooLarge(usize),
}

#[derive(Debug)]
pub(crate) struct StreamFramer {
    format: StreamFormat,
    line: Vec<u8>,
    record: Vec<u8>,
    max_record_bytes: Option<usize>,
    skip_lf: bool,
    at_start: bool,
}

impl StreamFramer {
    pub(crate) fn new(format: StreamFormat) -> Self {
        Self {
            format,
            line: Vec::new(),
            record: Vec::new(),
            max_record_bytes: None,
            skip_lf: false,
            at_start: true,
        }
    }

    #[cfg(test)]
    fn with_max_record_bytes(format: StreamFormat, max_record_bytes: usize) -> Self {
        Self {
            format,
            line: Vec::new(),
            record: Vec::new(),
            max_record_bytes: Some(max_record_bytes),
            skip_lf: false,
            at_start: true,
        }
    }

    pub(crate) fn records<'a>(
        &'a mut self,
        bytes: &'a [u8],
    ) -> impl Iterator<Item = Result<StreamRecord, FramingError>> + 'a {
        let mut bytes = bytes.iter();
        let mut failed = false;
        std::iter::from_fn(move || {
            if failed {
                return None;
            }
            for &byte in bytes.by_ref() {
                match self.push_byte(byte) {
                    Ok(Some(record)) => return Some(Ok(record)),
                    Ok(None) => {}
                    Err(error) => {
                        failed = true;
                        return Some(Err(error));
                    }
                }
            }
            None
        })
    }

    #[cfg(test)]
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<StreamRecord>, FramingError> {
        self.records(bytes).collect()
    }

    fn push_byte(&mut self, byte: u8) -> Result<Option<StreamRecord>, FramingError> {
        if self.skip_lf {
            self.skip_lf = false;
            if byte == b'\n' {
                return Ok(None);
            }
        }
        if let Some(max_record_bytes) = self.max_record_bytes
            && self.record.len() >= max_record_bytes
        {
            return Err(FramingError::RecordTooLarge(max_record_bytes));
        }
        if byte == b'\n' || (self.format == StreamFormat::Sse && byte == b'\r') {
            self.skip_lf = byte == b'\r';
            self.record.push(b'\n');
            self.complete_line()
        } else {
            self.record.push(byte);
            self.line.push(byte);
            Ok(None)
        }
    }

    /// The record left when the body ended without its terminating blank line
    /// (or newline) still counts; a body with no SSE fields at all (a plain JSON
    /// answer to a stream request) is returned whole as the record's data.
    pub(crate) fn finish(&mut self) -> Result<Option<StreamRecord>, FramingError> {
        let record = std::mem::take(&mut self.record);
        self.line.clear();
        let record = if self.at_start {
            self.at_start = false;
            record.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&record)
        } else {
            &record
        };
        let text = std::str::from_utf8(record).map_err(|_| FramingError::InvalidUtf8)?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        match self.format {
            StreamFormat::Sse => match parse_sse(record)? {
                Some(parsed) => Ok(Some(parsed)),
                None if text.trim_start().starts_with(['{', '[']) => Ok(Some(StreamRecord {
                    event: None,
                    data: text.trim().to_owned(),
                })),
                None => Ok(None),
            },
            StreamFormat::Ndjson => Ok(Some(StreamRecord {
                event: None,
                data: text.trim().to_owned(),
            })),
        }
    }

    fn complete_line(&mut self) -> Result<Option<StreamRecord>, FramingError> {
        if self.at_start {
            self.at_start = false;
            if self.line.starts_with(b"\xef\xbb\xbf") {
                self.line.drain(..3);
                self.record.drain(..3);
            }
        }
        let content = match self.format {
            StreamFormat::Sse => self.line.as_slice(),
            StreamFormat::Ndjson => self.line.strip_suffix(b"\r").unwrap_or(&self.line),
        };

        let result = match self.format {
            StreamFormat::Sse if content.is_empty() => {
                let record = parse_sse(&self.record)?;
                self.record.clear();
                record
            }
            StreamFormat::Sse => None,
            StreamFormat::Ndjson => {
                let line = std::str::from_utf8(content).map_err(|_| FramingError::InvalidUtf8)?;
                let record = if line.trim().is_empty() {
                    None
                } else {
                    Some(StreamRecord {
                        event: None,
                        data: line.to_owned(),
                    })
                };
                self.record.clear();
                record
            }
        };
        self.line.clear();
        Ok(result)
    }
}

fn parse_sse(record: &[u8]) -> Result<Option<StreamRecord>, FramingError> {
    let record = std::str::from_utf8(record).map_err(|_| FramingError::InvalidUtf8)?;
    let mut event = None;
    let mut data = String::new();
    let mut has_data = false;

    for raw_line in record.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = line.split_once(':').map_or((line, ""), |(field, value)| {
            (field, value.strip_prefix(' ').unwrap_or(value))
        });
        match field {
            "data" => {
                if has_data {
                    data.push('\n');
                }
                data.push_str(value);
                has_data = true;
            }
            "event" => event = Some(value.to_owned()),
            _ => {}
        }
    }

    Ok(has_data.then_some(StreamRecord { event, data }))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test fixtures assert exact framing outcomes"
)]
mod tests {
    use super::{FramingError, StreamFormat, StreamFramer};

    #[test]
    fn sse_preserves_utf8_split_across_chunks() {
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        assert!(framer.push(b"data: caf\xc3").unwrap().is_empty());
        assert!(framer.push(b"\xa9\n").unwrap().is_empty());

        let records = framer.push(b"\n").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event, None);
        assert_eq!(records[0].data, "café");
    }

    #[test]
    fn sse_waits_for_split_crlf_blank_line() {
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        assert!(framer.push(b"data: value\r").unwrap().is_empty());
        let records = framer.push(b"\n\r").unwrap();
        assert_eq!(records[0].data, "value");
        assert!(framer.push(b"\n").unwrap().is_empty());
    }

    #[test]
    fn sse_joins_data_and_ignores_other_fields() {
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        let records = framer
            .push(
                b": heartbeat\r\nid: ignored\ndata: first\ndata: second\nevent: delta\nretry: 1\nunknown: ignored\n\n",
            )
            .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event.as_deref(), Some("delta"));
        assert_eq!(records[0].data, "first\nsecond");
    }

    #[test]
    fn ndjson_emits_nonblank_lines_after_split_input() {
        let mut framer = StreamFramer::new(StreamFormat::Ndjson);
        assert!(framer.push(b"{\"value\":").unwrap().is_empty());
        assert!(framer.push(b"1}\r").unwrap().is_empty());

        let records = framer.push(b"\n\n{\"next\":true}\n").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].data, "{\"value\":1}");
        assert_eq!(records[1].data, "{\"next\":true}");
        assert!(records.iter().all(|record| record.event.is_none()));
    }

    #[test]
    fn overflow_is_rejected_before_a_record_is_committed() {
        let mut framer = StreamFramer::with_max_record_bytes(StreamFormat::Ndjson, 4);
        assert_eq!(framer.push(b"1234").unwrap().len(), 0);
        assert_eq!(
            framer.push(b"5").unwrap_err(),
            FramingError::RecordTooLarge(4)
        );
    }

    #[test]
    fn unterminated_final_records_are_kept_like_legacy_line_reading() {
        let mut sse = StreamFramer::new(StreamFormat::Sse);
        assert!(sse.push(b"data: partial\n").unwrap().is_empty());
        assert_eq!(sse.finish().unwrap().unwrap().data, "partial");

        let mut ndjson = StreamFramer::new(StreamFormat::Ndjson);
        assert!(ndjson.push(br#"{"value":1}"#).unwrap().is_empty());
        assert_eq!(ndjson.finish().unwrap().unwrap().data, r#"{"value":1}"#);

        let mut plain = StreamFramer::new(StreamFormat::Sse);
        assert!(
            plain
                .push(b"{\"choices\":[{\"message\":{\"content\":\"hi\"}}]}\n")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            plain.finish().unwrap().unwrap().data,
            r#"{"choices":[{"message":{"content":"hi"}}]}"#
        );

        let mut blank = StreamFramer::new(StreamFormat::Sse);
        assert!(blank.push(b"\r\n").unwrap().is_empty());
        assert_eq!(blank.finish().unwrap(), None);
    }

    #[test]
    fn records_are_not_size_capped_by_default() {
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        let image = format!("data: {}\n\n", "A".repeat(2 * 1024 * 1024));
        let records = framer.push(image.as_bytes()).unwrap();
        assert_eq!(records[0].data.len(), 2 * 1024 * 1024);
    }

    #[test]
    fn malformed_utf8_is_rejected_only_when_a_record_completes() {
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        assert!(framer.push(b"data: \xff").unwrap().is_empty());
        assert_eq!(framer.push(b"\n\n").unwrap_err(), FramingError::InvalidUtf8);
    }
    #[test]
    fn sse_is_independent_of_chunk_boundaries_and_line_endings() {
        let wire = "\u{feff}: heartbeat\rdata: café 🥬\rdata: second\r\revent: delta\r\ndata: next\n\n";
        for split in 0..=wire.len() {
            let mut framer = StreamFramer::new(StreamFormat::Sse);
            let mut records = framer.push(&wire.as_bytes()[..split]).unwrap();
            assert!(framer.push(b"").unwrap().is_empty());
            records.extend(framer.push(&wire.as_bytes()[split..]).unwrap());
            assert_eq!(records.len(), 2, "split {split}");
            assert_eq!(records[0].data, "café 🥬\nsecond", "split {split}");
            assert_eq!(records[1].event.as_deref(), Some("delta"));
            assert_eq!(records[1].data, "next");
            assert_eq!(framer.finish().unwrap(), None);
        }
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        let mut records = Vec::new();
        for byte in wire.as_bytes() {
            records.extend(framer.push(std::slice::from_ref(byte)).unwrap());
        }
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].data, "café 🥬\nsecond");
    }

    #[test]
    fn sse_metadata_without_data_never_becomes_json_at_eof() {
        for payload in [": heartbeat", "event: ping", "id: 123", "retry: 1000", "unknown"] {
            let mut framer = StreamFramer::new(StreamFormat::Sse);
            assert!(framer.push(payload.as_bytes()).unwrap().is_empty());
            assert_eq!(framer.finish().unwrap(), None, "{payload}");
        }
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        assert!(framer.push(b"event: ping\n\n").unwrap().is_empty());
        let records = framer.push(b"data:\n\ndata\ndata: next\n\n").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].event, None);
        assert_eq!(records[0].data, "");
        assert_eq!(records[1].data, "\nnext");
    }

    #[test]
    fn sse_strips_only_the_leading_bom() {
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        let records = framer.push("\u{feff}data: \u{feff}value\n\n".as_bytes()).unwrap();
        assert_eq!(records[0].data, "\u{feff}value");
    }

    #[test]
    fn records_are_parsed_only_as_the_consumer_reads_them() {
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        let mut records = framer.records(b"data: [DONE]\n\ndata: \xff\n\n");
        assert_eq!(records.next().unwrap().unwrap().data, "[DONE]");
        assert_eq!(records.next().unwrap(), Err(FramingError::InvalidUtf8));
    }


    #[test]
    fn every_unicode_scalar_value_roundtrips_through_both_stream_formats() {
        let expected: String = (0..=u32::from(char::MAX)).filter_map(char::from_u32).collect();
        assert_eq!(expected.chars().count(), 1_112_064);
        let payload = serde_json::to_string(&expected).unwrap();
        for format in [StreamFormat::Sse, StreamFormat::Ndjson] {
            let wire = match format {
                StreamFormat::Sse => format!("data: {payload}\n\n"),
                StreamFormat::Ndjson => format!("{payload}\n"),
            };
            let mut framer = StreamFramer::new(format);
            let mut records = Vec::new();
            for chunk in wire.as_bytes().chunks(4093) {
                records.extend(framer.records(chunk).collect::<Result<Vec<_>, _>>().unwrap());
            }
            assert_eq!(records.len(), 1, "{format:?}");
            let decoded: String = serde_json::from_str(&records[0].data).unwrap();
            assert_eq!(decoded, expected, "{format:?}");
        }
    }

    #[test]
    fn invalid_unicode_encodings_are_rejected_without_replacement_characters() {
        for invalid in [
            b"\xc0\xaf".as_slice(),
            b"\xed\xa0\x80".as_slice(),
            b"\xf4\x90\x80\x80".as_slice(),
            b"\xe2\x82".as_slice(),
        ] {
            let mut framer = StreamFramer::new(StreamFormat::Sse);
            assert!(framer.push(b"data: ").unwrap().is_empty());
            assert!(framer.push(invalid).unwrap().is_empty());
            assert_eq!(framer.push(b"\n\n"), Err(FramingError::InvalidUtf8));
        }
    }
}
