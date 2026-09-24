//! Byte-oriented framing for provider response streams.

const MAX_RECORD_BYTES: usize = 256 * 1024;

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
    #[error("stream ended in the middle of a record")]
    PrematureEof,
}

#[derive(Debug)]
pub(crate) struct StreamFramer {
    format: StreamFormat,
    line: Vec<u8>,
    record: Vec<u8>,
    max_record_bytes: usize,
}

impl StreamFramer {
    pub(crate) fn new(format: StreamFormat) -> Self {
        Self {
            format,
            line: Vec::new(),
            record: Vec::new(),
            max_record_bytes: MAX_RECORD_BYTES,
        }
    }

    #[cfg(test)]
    fn with_max_record_bytes(format: StreamFormat, max_record_bytes: usize) -> Self {
        Self {
            format,
            line: Vec::new(),
            record: Vec::new(),
            max_record_bytes,
        }
    }

    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<Vec<StreamRecord>, FramingError> {
        let mut records = Vec::new();
        for &byte in bytes {
            if self.record.len() >= self.max_record_bytes {
                return Err(FramingError::RecordTooLarge(self.max_record_bytes));
            }
            self.record.push(byte);
            self.line.push(byte);
            if byte == b'\n' {
                if let Some(record) = self.complete_line()? {
                    records.push(record);
                }
            }
        }
        Ok(records)
    }

    pub(crate) fn finish(&mut self) -> Result<(), FramingError> {
        if self.record.is_empty() {
            return Ok(());
        }

        let is_whitespace =
            std::str::from_utf8(&self.record).is_ok_and(|record| record.trim().is_empty());
        if !is_whitespace {
            return Err(FramingError::PrematureEof);
        }

        self.line.clear();
        self.record.clear();
        Ok(())
    }

    fn complete_line(&mut self) -> Result<Option<StreamRecord>, FramingError> {
        let content_end = self.line.len() - 1;
        let content = self.line[..content_end]
            .strip_suffix(b"\r")
            .unwrap_or(&self.line[..content_end]);

        let result = match self.format {
            StreamFormat::Sse if content.is_empty() => {
                let record = parse_sse(&self.record)?;
                self.record.clear();
                Some(record).filter(|record| record.event.is_some() || !record.data.is_empty())
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

fn parse_sse(record: &[u8]) -> Result<StreamRecord, FramingError> {
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

    Ok(StreamRecord { event, data })
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
        assert!(framer.push(b"\n\r").unwrap().is_empty());

        let records = framer.push(b"\n").unwrap();
        assert_eq!(records[0].data, "value");
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
    fn premature_eof_rejects_partial_sse_and_ndjson_records() {
        let mut sse = StreamFramer::new(StreamFormat::Sse);
        assert!(sse.push(b"data: partial\n").unwrap().is_empty());
        assert_eq!(sse.finish().unwrap_err(), FramingError::PrematureEof);

        let mut ndjson = StreamFramer::new(StreamFormat::Ndjson);
        assert!(ndjson.push(br#"{"value":1}"#).unwrap().is_empty());
        assert_eq!(ndjson.finish().unwrap_err(), FramingError::PrematureEof);
    }

    #[test]
    fn malformed_utf8_is_rejected_only_when_a_record_completes() {
        let mut framer = StreamFramer::new(StreamFormat::Sse);
        assert!(framer.push(b"data: \xff").unwrap().is_empty());
        assert_eq!(framer.push(b"\n\n").unwrap_err(), FramingError::InvalidUtf8);
    }
}
