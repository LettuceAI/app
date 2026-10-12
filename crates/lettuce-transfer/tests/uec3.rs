use lettuce_transfer::uec3::{self, Entity, Error};
use serde_json::{Value, json};

fn lorebook() -> Value {
    json!({
        "schema": {"name": "UEC", "version": "3.0"}, "kind": "lorebook",
        "meta": {"exportedAt": 30, "app": "LettuceAI 3.x", "createdAt": 10,
            "updatedAt": 20, "authors": ["Ada"], "license": "CC0", "sources": ["https://example.org"]},
        "entity": {"name": "Harbour", "icon": "asset:a1", "detection": "latest_user_message",
            "behaviorVersion": "deterministic_v2", "entries": [{"title": "Ships", "enabled": false,
                "alwaysActive": true, "keywords": ["ship.*", " sea "], "caseSensitive": true,
                "matchMode": "regex", "content": "The harbour", "priority": -7}]},
        "dependencies": [], "state": null,
        "assets": {"a1": {"mime": "image/png", "blake3": blake3::hash(b"image").to_hex().as_str(), "data": "aW1hZ2U="}}
    })
}

#[test]
fn every_lorebook_field_round_trips() {
    let source = lorebook();
    let mut staged = Vec::new();
    let card = uec3::read(&source.to_string(), |key, mime, bytes| {
        staged.push((key.to_owned(), mime.to_owned(), bytes.to_vec()));
        Ok(())
    }).expect("read");
    assert!(card.warnings.is_empty());
    assert!(matches!(card.document.entity, Entity::Lorebook(_)));
    assert_eq!(staged, [("a1".into(), "image/png".into(), b"image".to_vec())]);
    let mut output = Vec::new();
    uec3::write(&mut output, &card.document, |_, sink| {
        std::io::Write::write_all(sink, b"image")?;
        Ok(())
    }).expect("write");
    assert_eq!(serde_json::from_slice::<Value>(&output).expect("json"), source);
}

#[test]
fn bad_hash_fails_before_the_asset_sink_runs() {
    let mut source = lorebook();
    source["assets"]["a1"]["blake3"] = json!("0".repeat(64));
    assert!(matches!(uec3::read(&source.to_string(), |_, _, _| panic!("unverified asset")), Err(Error::AssetHash { key }) if key == "a1"));
}

#[test]
fn newer_minor_unknown_fields_warn_and_a_new_major_is_unsupported() {
    let mut source = lorebook();
    source["schema"]["version"] = json!("3.1");
    source["entity"]["future"] = json!({"nested": true});
    source["entity"]["entries"][0]["future"] = json!(1);
    let card = uec3::read(&source.to_string(), |_, _, _| Ok(())).expect("3.1");
    assert_eq!(card.warnings, ["entity.entries[0].future", "entity.future"]);
    source["schema"]["version"] = json!("4.0");
    assert!(matches!(uec3::read(&source.to_string(), |_, _, _| panic!("unsupported")), Err(Error::UnsupportedVersion(_))));
}

#[test]
fn asset_decode_is_sequential_without_a_count_cap() {
    let mut source = lorebook();
    let asset = source["assets"]["a1"].clone();
    source["assets"] = json!({});
    for index in 0..1025 {
        source["assets"][format!("a{index}")] = asset.clone();
    }
    let mut count = 0;
    let card = uec3::read(&source.to_string(), |_, _, bytes| {
        assert_eq!(bytes, b"image");
        count += 1;
        Ok(())
    }).expect("read");
    assert_eq!(count, 1025);
    assert_eq!(card.document.assets.len(), count);
}
