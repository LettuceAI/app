use serde::{Deserialize, Serialize};

use super::model::Frontier;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingRange {
    pub origin_device_id: String,
    pub first_sequence: i64,
    pub last_sequence: i64,
}

pub fn outbound_ranges(local: &Frontier, remote: &Frontier) -> Vec<MissingRange> {
    local
        .iter()
        .filter_map(|(device_id, local_sequence)| {
            let remote_sequence = remote.get(device_id).copied().unwrap_or(0);
            if *local_sequence <= remote_sequence {
                return None;
            }
            Some(MissingRange {
                origin_device_id: device_id.clone(),
                first_sequence: remote_sequence + 1,
                last_sequence: *local_sequence,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{outbound_ranges, MissingRange};

    #[test]
    fn plans_only_contiguous_ranges_the_peer_is_missing() {
        let local = BTreeMap::from([
            ("device-a".to_string(), 5),
            ("device-b".to_string(), 3),
            ("device-c".to_string(), 1),
        ]);
        let remote = BTreeMap::from([
            ("device-a".to_string(), 2),
            ("device-b".to_string(), 3),
        ]);

        assert_eq!(
            outbound_ranges(&local, &remote),
            vec![
                MissingRange {
                    origin_device_id: "device-a".to_string(),
                    first_sequence: 3,
                    last_sequence: 5,
                },
                MissingRange {
                    origin_device_id: "device-c".to_string(),
                    first_sequence: 1,
                    last_sequence: 1,
                },
            ]
        );
    }
}
