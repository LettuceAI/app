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
    })
    .expect("read");
    assert!(card.warnings.is_empty());
    assert!(matches!(card.document.entity, Entity::Lorebook(_)));
    assert_eq!(
        staged,
        [("a1".into(), "image/png".into(), b"image".to_vec())]
    );
    let mut output = Vec::new();
    uec3::write(&mut output, &card.document, |_, sink| {
        std::io::Write::write_all(sink, b"image")?;
        Ok(())
    })
    .expect("write");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).expect("json"),
        source
    );
}

#[test]
fn bad_hash_fails_before_the_asset_sink_runs() {
    let mut source = lorebook();
    source["assets"]["a1"]["blake3"] = json!("0".repeat(64));
    assert!(
        matches!(uec3::read(&source.to_string(), |_, _, _| panic!("unverified asset")), Err(Error::AssetHash { key }) if key == "a1")
    );
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
    assert!(matches!(
        uec3::read(&source.to_string(), |_, _, _| panic!("unsupported")),
        Err(Error::UnsupportedVersion(_))
    ));
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
    })
    .expect("read");
    assert_eq!(count, 1025);
    assert_eq!(card.document.assets.len(), count);
}

#[test]
fn model_profile_round_trips_typed_parameters_and_portable_runtime_settings() {
    let mut source = lorebook();
    source["kind"] = json!("model_profile");
    source["assets"] = json!({});
    source["entity"] = json!({
        "displayName": "Local image model", "kind": "image",
        "provider": {"kind": "stable-diffusion", "label": "Local"}, "externalId": "model-name",
        "parameters": uec3::ModelParameters {chat: Default::default(), features: Default::default()},
        "capabilities": serde_json::to_value(uec3::ModelProfile {
            display_name:String::new(), kind:lettuce_models::ModelKind::Chat,
            provider:uec3::ModelProvider {kind:String::new(),label:String::new()}, external_id:String::new(),
            parameters:uec3::ModelParameters {chat:Default::default(),features:Default::default()},
            capabilities:Default::default(),llama_cpp:Default::default(),stable_diffusion:Default::default()
        }).expect("wire capabilities")["capabilities"],
        "llamaCpp": {}, "stableDiffusion": {}
    });
    let read = uec3::read(&source.to_string(), |_, _, _| panic!("no assets")).expect("model");
    assert!(read.warnings.is_empty());
    let mut output = Vec::new();
    uec3::write(&mut output, &read.document, |_, _| panic!("no assets")).expect("write");
    let output: Value = serde_json::from_slice(&output).expect("JSON");
    assert_eq!(
        output["entity"]["displayName"],
        source["entity"]["displayName"]
    );
    assert_eq!(
        output["entity"]["parameters"],
        source["entity"]["parameters"]
    );
    assert_eq!(
        output["entity"]["capabilities"],
        source["entity"]["capabilities"]
    );
    assert_eq!(output["entity"]["provider"], source["entity"]["provider"]);
    let again = uec3::read(&output.to_string(), |_, _, _| panic!("no assets")).expect("again");
    assert_eq!(again.document, read.document);
}

#[test]
fn prompt_round_trips_every_entry_field_without_invented_versions() {
    let mut source = lorebook();
    source["kind"] = json!("prompt");
    source["assets"] = json!({});
    source["entity"] = json!({"name":"Edited companion", "purpose":"companion_chat",
        "condense":true, "behaviorVersion":"deterministic_v2", "builtIn":"prompt_app_companion",
        "entries":[{"name":"Context", "role":"system",
            "content":"{{character}}", "enabled":false,"position":"in_chat", "depth":7,
            "conditionalMinMessages":13,"intervalTurns":5,"systemPrompt":true,
            "conditions":{"type":"all","conditions":[{"type":"chatMode","value":"direct"}]},
            "payload":{"type":"imageSlot","slot":"chatBackground"}}]});
    let read = uec3::read(&source.to_string(), |_, _, _| panic!("no assets")).expect("prompt");
    assert!(read.warnings.is_empty());
    let mut output = Vec::new();
    uec3::write(&mut output, &read.document, |_, _| panic!("no assets")).expect("write");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).expect("JSON"),
        source
    );
}

#[test]
fn persona_round_trips_every_authored_field_and_enabled_binding() {
    let mut source = lorebook();
    let book = source["entity"].clone();
    source["kind"] = json!("persona");
    source["entity"] = json!({"name":"Writer", "nickname":"W", "description":"Authored description",
        "design":{"description":"Authored design","references":["asset:a1"]},
        "media":{"avatar":"asset:a1","avatarCrop":{"x":0.2f32,"y":0.3f32,"scale":1.7f32}},
        "behavior":{"image":{"fileName":"ink.safetensors","sha256":"hash","strength":0.8f32},
            "lorebooks":[{"book":"dep:l1","enabled":false}]}});
    source["dependencies"] = json!([{"key":"l1","kind":"lorebook","entity":book}]);
    let read = uec3::read(&source.to_string(), |_, _, _| Ok(())).expect("persona");
    assert!(read.warnings.is_empty());
    let mut output = Vec::new();
    uec3::write(&mut output, &read.document, |_, sink| {
        std::io::Write::write_all(sink, b"image")?;
        Ok(())
    })
    .expect("write");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).expect("JSON"),
        source
    );
}

#[test]
fn standalone_starters_preserve_roles_and_all_lorebook_selection_states() {
    for books in [
        json!("inherit"),
        json!("disabled"),
        json!([]),
        json!(["dep:l1"]),
    ] {
        let mut source = lorebook();
        source["kind"] = json!("chat_template");
        source["assets"] = json!({});
        source["entity"] = json!({"key":"t1","name":"Opening","messages":[
            {"role":"user","text":"User opening"},{"role":"assistant","text":"Assistant opening"}],
            "prompt":"builtin:prompt_app_default","lorebooks":books});
        let read = uec3::read(&source.to_string(), |_, _, _| panic!("no assets")).expect("starter");
        assert!(read.warnings.is_empty());
        let mut output = Vec::new();
        uec3::write(&mut output, &read.document, |_, _| panic!("no assets")).expect("write");
        assert_eq!(
            serde_json::from_slice::<Value>(&output).expect("JSON"),
            source
        );
        source["entity"]["messages"][0]["role"] = json!("system");
        assert!(matches!(
            uec3::read(&source.to_string(), |_, _, _| Ok(())),
            Err(Error::Format(_))
        ));
    }
}

#[test]
fn model_nested_fields_are_camel_case_and_new_minor_fields_are_warnings() {
    let mut source = lorebook();
    source["schema"]["version"] = json!("3.1");
    source["kind"] = json!("model_profile");
    source["assets"] = json!({});
    source["entity"] = json!({
        "displayName":"Remote model","kind":"chat","provider":{"kind":"openai","label":"Work"},
        "externalId":"model-name","parameters":{"chat":{},"features":{}},"capabilities":{},
        "llamaCpp":{},"stableDiffusion":{}
    });
    source["entity"]["parameters"]["chat"]["topP"] = json!(0.8);
    source["entity"]["parameters"]["chat"]["future"] = json!(true);
    source["entity"]["capabilities"]["future"] = json!(true);
    let card = uec3::read(&source.to_string(), |_, _, _| panic!("no assets")).expect("minor");
    assert_eq!(
        card.warnings,
        [
            "entity.capabilities.future",
            "entity.parameters.chat.future"
        ]
    );
    let mut output = Vec::new();
    uec3::write(&mut output, &card.document, |_, _| panic!("no assets")).expect("write");
    let output: Value = serde_json::from_slice(&output).expect("JSON");
    assert_eq!(output["entity"]["parameters"]["chat"]["topP"], 0.8);
    assert!(
        output["entity"]["parameters"]["chat"]
            .get("top_p")
            .is_none()
    );
    assert_eq!(output["entity"]["capabilities"]["formatVersion"], 1);
}

#[test]
fn undefined_purpose_is_invalid_uec3_but_legacy_maps_to_direct_chat() {
    let legacy = json!({"schema":{"name":"USC","version":"1.0"},
        "kind":"system_prompt_template", "payload":{"name":"Older prompt",
        "promptType":"undefined", "content":"Authored content"}});
    let imported = lettuce_transfer::parse_prompt_import(&legacy.to_string(), None, "Entry", "Set")
        .expect("legacy prompt");
    assert_eq!(imported.purpose, lettuce_context::PromptPurpose::DirectChat);
    let mut source = lorebook();
    source["kind"] = json!("prompt");
    source["assets"] = json!({});
    source["entity"] = json!({"name":"Invalid prompt", "purpose":"undefined",
        "condense":false,"behaviorVersion":"deterministic_v2","builtIn":null,"entries":[]});
    assert!(matches!(
        uec3::read(&source.to_string(), |_, _, _| Ok(())),
        Err(Error::Format(_))
    ));
}

#[test]
fn dependencies_reject_non_library_kinds_and_duplicate_keys() {
    let mut source = lorebook();
    source["dependencies"] = json!([
        {"key":"l1","kind":"lorebook","entity":source["entity"]},
        {"key":"l1","kind":"lorebook","entity":source["entity"]}
    ]);
    assert!(matches!(
        uec3::read(&source.to_string(), |_, _, _| Ok(())),
        Err(Error::Format(_))
    ));
    source["dependencies"] = json!([{"key":"t1","kind":"chat_template","entity":{
        "key":"t1","name":"Opening","messages":[],"prompt":null,"lorebooks":"inherit"
    }}]);
    assert!(matches!(
        uec3::read(&source.to_string(), |_, _, _| Ok(())),
        Err(Error::Format(_))
    ));
}

fn character() -> Value {
    let mut source = lorebook();
    let book = source["entity"].clone();
    let native = serde_json::to_value(lettuce_characters::ChatAppearanceV1::default())
        .expect("chat appearance");
    let mut chat = serde_json::Map::new();
    for (key, value) in native.as_object().expect("object") {
        let mut parts = key.split('_');
        let mut camel = parts.next().expect("first").to_owned();
        for part in parts {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                camel.extend(first.to_uppercase());
                camel.extend(chars);
            }
        }
        chat.insert(camel, value.clone());
    }
    source["kind"] = json!("character");
    source["entity"] = json!({
        "name":"Ada","nickname":"A","description":"Keeper","definition":"Precise","scenario":"Harbour",
        "creator":"Author","creatorNotes":"Notes","creatorNotesLocalized":{"tr":"Notlar"},"sources":["https://example.org"],"tags":["keeper"],
        "design":{"description":"Coat","references":["asset:a1"]},
        "media":{"avatar":"asset:a1","banner":"asset:a1","background":"asset:a1","avatarCrop":{"x":0.2f32,"y":0.3f32,"scale":1.2f32},"bannerCrop":{"x":0.1f32,"y":0.2f32,"scale":1.3f32}},
        "appearance":{"formatVersion":1,"cardStyle":"banner","disableGradient":true,"gradientSource":"round",
            "customGradient":{"enabled":true,"colors":["#112233","#445566"]},"textColors":{"primary":"#ffffff","secondary":"#dddddd"},"chat":chat},
        "scenes":[{"key":"s1","formatVersion":1,"parts":[{"text":"Opening"},{"image":"asset:a1"}],"direction":"North","background":"asset:a1",
            "variants":[{"key":"v1","formatVersion":1,"parts":[{"text":"Alternate"},{"image":"asset:a1"}],"direction":"South"}],"selectedVariant":"v1"},
            {"key":"s2","formatVersion":1,"parts":[{"text":"Second"}],"direction":"East","background":null,"variants":[],"selectedVariant":null}],
        "defaultScene":"s1","starters":[
            {"key":"t1","name":"First","messages":[{"role":"user","text":"Hello"}],"scene":"s1","prompt":"builtin:prompt_app_default","lorebooks":"inherit"},
            {"key":"t2","name":"Second","messages":[{"role":"assistant","text":"Welcome"}],"scene":"s2","prompt":null,"lorebooks":[]},
            {"key":"t3","name":"Third","messages":[{"role":"assistant","text":"Again"}],"scene":null,"prompt":null,"lorebooks":["dep:l1"]}],
        "defaultStarter":"t1","behavior":{"interactionMode":"roleplay","memory":"dynamic","prompts":{"direct":"builtin:prompt_app_default","groupConversation":"builtin:prompt_app_group_chat","groupRoleplay":null},
            "model":{"name":"Chat","provider":"openai","externalId":"chat"},"voice":{"provider":null,"autoplay":true},
            "lorebooks":[{"book":"dep:l1","enabled":false}],"image":{"fileName":"ink.safetensors","sha256":null,"strength":0.8f32}},
        "companion":null
    });
    source["dependencies"] = json!([{"key":"l1","kind":"lorebook","entity":book}]);
    source
}

#[test]
fn character_authored_fields_scenes_variants_starters_and_media_round_trip() {
    let source = character();
    let read = uec3::read(&source.to_string(), |_, _, _| Ok(())).expect("character");
    assert!(read.warnings.is_empty());
    let mut output = Vec::new();
    uec3::write(&mut output, &read.document, |_, sink| {
        std::io::Write::write_all(sink, b"image")?;
        Ok(())
    })
    .expect("write");
    assert_eq!(
        serde_json::from_slice::<Value>(&output).expect("JSON"),
        source
    );
}

#[test]
fn companion_identity_notes_and_optional_learned_state_round_trip_without_conversation_data() {
    let mut source = character();
    source["entity"]["behavior"]["interactionMode"] = json!("companion");
    source["entity"]["companion"] = json!({
        "soul":{"essence":"Essence","traits":"Traits","backstory":"Backstory","appearance":"Appearance","goals":"Goals","likes":"Likes","voice":"Voice","relationalStyle":"Style","vulnerabilities":"Vulnerable","fears":"Fears","habits":"Habits","boundaries":"Boundaries"},
        "baselineAffect":{"warmth":0.1,"trust":0.2,"calm":0.3,"vulnerability":0.4,"longing":0.5,"hurt":0.6,"tension":0.7,"irritation":0.8,"affectionIntensity":0.9,"reassuranceNeed":1.0},
        "regulationStyle":{"suppression":0.1,"volatility":0.2,"recoverySpeed":0.3,"conflictAvoidance":0.4,"reassuranceSeeking":0.5,"protestBehavior":0.6,"emotionalTransparency":0.7,"attachmentActivation":0.8,"pride":0.9},
        "relationshipDefaults":{"closeness":0.1,"trust":0.2,"affection":0.3,"tension":0.4},
        "authoredFacts":[{"key":"f1","category":"likes","value":"Sea","policy":"adaptive","slot":"likes.sea","locked":true,"weight":0.8,"confidence":0.82,"evidenceCount":3,"validFrom":1,"validUntil":20}],
        "prompting":{"prompt":"builtin:prompt_app_companion","styleNotes":"Calm"},
        "timeAwareness":true,"shareMemoryAcrossChats":true,"shareSoulGrowthAcrossChats":false,
        "scheduledNotes":[{"key":"n1","label":"Anniversary","content":"Remember","availableAt":1,"expiresAt":20,"recurrence":"yearly","recurrenceWindowMs":1000,"enabled":false}]
    });
    source["state"] = json!({"companion":{
        "soul":[{"key":"f2","category":"likes","value":"Ships","kind":"adjust","policy":"adaptive","slot":"likes.ships","confidence":0.9,"evidenceCount":2,"weight":0.8,"validFrom":2,"validUntil":null,"locked":false,"createdAt":2,"supersedes":["f1"],"supersededBy":null,"supersededAt":null,"sourceMemories":["m1"]}],
        "memory":{"items":[{"key":"m1","text":"A memory","category":"preference","origin":"model","observedAt":2,"observedPrecision":"user","supersedes":[],"supersededBy":null,"supersededAt":null,"pinned":true,"cold":false,"importance":8000,"persistenceImportance":7000,"promptImportance":6000,"volatility":1000,"createdAt":2}]}
    }});
    let read = uec3::read(&source.to_string(), |_, _, _| Ok(())).expect("companion");
    assert!(read.warnings.is_empty());
    let mut output = Vec::new();
    uec3::write(&mut output, &read.document, |_, sink| {
        std::io::Write::write_all(sink, b"image")?;
        Ok(())
    })
    .expect("write");
    let output: Value = serde_json::from_slice(&output).expect("JSON");
    assert_eq!(output, source);
    let state = output["state"].to_string();
    for forbidden in [
        "relationships",
        "sourceRole",
        "sourceMessage",
        "summary",
        "accessCount",
        "embedding",
        "tokenCount",
        "progress",
        "receipt",
    ] {
        assert!(!state.contains(forbidden), "{forbidden}");
    }
    source["state"] = Value::Null;
    let read = uec3::read(&source.to_string(), |_, _, _| Ok(())).expect("without learned");
    assert!(read.document.state.is_none());
}

#[test]
fn newer_minor_prompt_condition_and_payload_fields_are_format_errors() {
    for (conditions, payload) in [
        (
            json!({"type":"all","future":1,"conditions":[]}),
            json!({"type":"imageSlot","slot":"character"}),
        ),
        (
            json!({"type":"all","conditions":[{"type":"chatMode","value":"direct","future":2}]}),
            json!({"type":"imageSlot","slot":"character"}),
        ),
        (
            json!({"type":"all","conditions":[]}),
            json!({"type":"imageSlot","slot":"character","future":3}),
        ),
    ] {
        let mut source = lorebook();
        source["kind"] = json!("prompt");
        source["schema"]["version"] = json!("3.1");
        source["assets"] = json!({});
        source["entity"] = json!({"name":"Program","purpose":"direct_chat","condense":false,
        "behaviorVersion":"deterministic_v2","builtIn":null,"entries":[{
            "name":"Entry","role":"system","content":"Authored","enabled":true,"position":"relative","depth":0,
            "conditionalMinMessages":null,"intervalTurns":null,"systemPrompt":false,
            "conditions":conditions,"payload":payload
        }]});
        assert!(matches!(
            uec3::read(&source.to_string(), |_, _, _| Ok(())),
            Err(uec3::Error::Format(_))
        ));
    }
}

#[test]
fn newer_minor_crop_and_widget_fields_are_named_warnings() {
    let mut source = character();
    source["schema"]["version"] = json!("3.1");
    source["entity"]["media"]["avatarCrop"]["future"] = json!(true);
    source["entity"]["appearance"]["chat"]["chatWidgetSlots"]["left"] = json!([
        {"type":"character_info","id":"info","design":"minimal","characterId":"self","future":true},
        {"type":"image","id":"image","design":"default","title":"Picture","description":"Authored","source":{"kind":"logical_asset","assetId":"asset:a1","future":true},"shape":"wide"}
    ]);
    let read = uec3::read(&source.to_string(), |_, _, _| Ok(())).expect("new minor");
    assert_eq!(
        read.warnings,
        [
            "entity.appearance.chat.chatWidgetSlots.left[0].future",
            "entity.appearance.chat.chatWidgetSlots.left[1].source.future",
            "entity.media.avatarCrop.future"
        ]
    );
}

#[test]
fn non_character_files_reject_character_learned_state() {
    let mut source = lorebook();
    source["state"] = json!({"companion":{"soul":[],"memory":{"items":[]}}});
    assert!(matches!(
        uec3::read(&source.to_string(), |_, _, _| Ok(())),
        Err(uec3::Error::Format(_))
    ));
}

#[test]
fn every_chat_appearance_field_and_widget_kind_survives_native_conversion() {
    let mut value = character()["entity"]["appearance"]["chat"].clone();
    for (key, field) in value.as_object_mut().expect("chat") {
        if let Some(flag) = field.as_bool() {
            *field = json!(!flag);
        }
        if key.ends_with("ColorHex") {
            *field = json!("#123456");
        }
    }
    for (key, choice) in [
        ("fontSize", "xlarge"),
        ("lineSpacing", "relaxed"),
        ("bubbleStyle", "bordered"),
        ("bubbleRadius", "pill"),
        ("bubbleMaxWidth", "compact"),
        ("bubblePadding", "spacious"),
        ("timestampFormat", "datetime"),
        ("messageHeaderPlacement", "above"),
        ("messageInfoPlacement", "inside_bubble"),
        ("messageInfoSize", "large"),
        ("messageGap", "relaxed"),
        ("avatarShape", "rounded"),
        ("avatarSize", "large"),
        ("chatColumnWidth", "custom"),
        ("chatColumnAlign", "right"),
        ("participantsBarAvatarSize", "large"),
        ("participantsBarAvatarShape", "boxed"),
        ("participantsBarBackground", "fading"),
        ("participantsBarGap", "tight"),
        ("participantsBarAlign", "left"),
        ("participantsBarHintPosition", "hidden"),
        ("chatWidgetCenterMode", "left"),
        ("userBubbleColor", "warning"),
        ("assistantBubbleColor", "secondary"),
        ("bubbleBlur", "heavy"),
        ("textMode", "dark"),
    ] {
        value[key] = json!(choice);
    }
    value["chatColumnWidthPx"] = json!(700);
    value["backgroundDim"] = json!(0.4f32);
    value["backgroundBlur"] = json!(0.2f32);
    value["bubbleOpacity"] = json!(60.0f32);
    let variants = [
        json!({"type":"divider","style":"space"}),
        json!({"type":"box","variant":"warning","title":"Box","description":"Description","children":[]}),
        json!({"type":"character_info","characterId":"self"}),
        json!({"type":"persona_info"}),
        json!({"type":"scratch_pad","title":"Scratch","description":"Description","content":"Authored"}),
        json!({"type":"image","title":"Image","description":"Description","source":{"kind":"unresolved_legacy","token":"original-token"},"shape":"wide"}),
        json!({"type":"selector","kind":"author_note","title":"Selector","description":"Description"}),
        json!({"type":"button","action":"toggle_voice_autoplay","title":"Button","description":"Description"}),
        json!({"type":"stat_tracker","title":"Stats","description":"Description","stats":[{"id":"stat","label":"Value","value":0.5,"min":0.0,"max":1.0}]}),
        json!({"type":"quick_snippets","title":"Snippets","description":"Description","snippets":[{"id":"snippet","label":"Hello","text":"Authored"}]}),
        json!({"type":"dice","title":"Dice","description":"Description","notation":"2d6"}),
        json!({"type":"memory","title":"Memory","limit":12}),
        json!({"type":"companion_state","title":"State"}),
        json!({"type":"session_info","title":"Session"}),
        json!({"type":"author_note","title":"Author","description":"Description"}),
        json!({"type":"time","title":"Time","hourFormat":"h24","showSeconds":true,"showDate":true}),
    ];
    let character_id = lettuce_types::CharacterId::new();
    let mut wire_slots = uec3::ChatWidgetSlots {
        left: vec![],
        right: vec![],
    };
    let mut native_slots = lettuce_characters::ChatWidgetSlots::default();
    for (index, mut widget) in variants.into_iter().enumerate() {
        widget["id"] = json!(format!("widget-{index}"));
        widget["design"] = json!("outline");
        let node: uec3::WidgetNode = serde_json::from_value(widget.clone()).expect("widget");
        let expected = serde_json::to_value(&node).expect("canonical widget");
        let native = node
            .clone()
            .into_native(&mut |_| panic!("no logical image"), &mut |key| {
                assert_eq!(key, "self");
                Ok(Some(character_id))
            })
            .expect("native widget");
        native.validate().expect("valid widget");
        let again = uec3::WidgetNode::from_native(
            &native,
            &mut |_| panic!("no logical image"),
            &mut |id| {
                assert_eq!(id, character_id);
                Ok("self".into())
            },
        )
        .expect("portable widget");
        assert_eq!(serde_json::to_value(&again).expect("widget"), expected);
        wire_slots.left.push(node);
        native_slots.left.push(native);
    }
    value["chatWidgetSlots"] = serde_json::to_value(&wire_slots).expect("slots");
    let portable: uec3::ChatAppearance = serde_json::from_value(value.clone()).expect("appearance");
    let native = portable.into_native(native_slots);
    native.validate().expect("valid appearance");
    assert_eq!(
        serde_json::to_value(uec3::ChatAppearance::from_native(&native, wire_slots))
            .expect("round trip"),
        value
    );
}
