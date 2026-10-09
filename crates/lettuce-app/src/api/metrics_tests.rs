use super::tests::{Reply, harness};
use lettuce_contracts::{self as dto, ApiErrorCode};

#[tokio::test]
async fn metrics_page_get_clear_and_replay_preserve_new_records() {
    let h = harness(Reply::Text("hello"));
    for index in 0..3 {
        h.context
            .backend()
            .database()
            .record_llm_generation_metrics(
                &format!("metric-{index}"),
                Some("/private/model.gguf"),
                &serde_json::json!({"tokens":index}),
                &[],
                index,
            )
            .expect("record");
    }
    let first = super::llm_metrics_list(
        &h.context,
        dto::LlmMetricsListRequest {
            cursor: None,
            limit: 2,
        },
    )
    .await
    .expect("list");
    assert_eq!(
        first
            .items
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["metric-2", "metric-1"]
    );
    let second = super::llm_metrics_list(
        &h.context,
        dto::LlmMetricsListRequest {
            cursor: first.next_cursor,
            limit: 2,
        },
    )
    .await
    .expect("page");
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].id, "metric-0");
    assert_eq!(second.next_cursor, None);
    let detail = super::llm_metrics_get(
        &h.context,
        dto::LlmMetricGetRequest {
            id: "metric-0".into(),
        },
    )
    .await
    .expect("get")
    .expect("row");
    assert_eq!(detail.samples, Some(Vec::new()));
    assert!(
        !serde_json::to_string(&detail)
            .expect("view")
            .contains("/private")
    );
    let key = lettuce_types::RequestId::new().to_string();
    let request = dto::LlmMetricsClearRequest {
        client_operation_id: key,
    };
    assert_eq!(
        super::llm_metrics_clear(&h.context, request.clone())
            .await
            .expect("clear")
            .removed,
        3
    );
    h.context
        .backend()
        .database()
        .record_llm_generation_metrics("new", None, &serde_json::json!({}), &[], 4)
        .expect("record");
    assert_eq!(
        super::llm_metrics_clear(&h.context, request)
            .await
            .expect("replay")
            .removed,
        3
    );
    assert!(
        super::llm_metrics_get(&h.context, dto::LlmMetricGetRequest { id: "new".into() })
            .await
            .expect("get")
            .is_some()
    );
    assert_eq!(
        super::llm_metrics_list(
            &h.context,
            dto::LlmMetricsListRequest {
                cursor: Some("bad cursor".into()),
                limit: 2
            }
        )
        .await
        .expect_err("cursor")
        .code,
        ApiErrorCode::InvalidInput
    );
}
