use super::nanogpt_usage::{parse_usage, usage_url};
use serde_json::json;

#[test]
fn nano_url_shapes_and_weekly_daily_monthly_normalization() {
    assert_eq!(
        usage_url("https://nano-gpt.com"),
        "https://nano-gpt.com/api/subscription/v1/usage"
    );
    assert_eq!(
        usage_url("https://proxy.example/nano/paid/v1/"),
        "https://proxy.example/nano/subscription/v1/usage"
    );
    let usage = parse_usage(&json!({"active":true,"usage":{"weekly":{"tokens_used":"45000000","tokens_remaining":15000000,"percent_used":75,"reset_at":"window"}},"daily":{"used":5,"remaining":4995},"monthly":{"used":50,"remaining":59950},"period":{"currentPeriodEnd":1785196800}})).expect("usage");
    let weekly = usage.weekly.expect("weekly");
    assert_eq!(weekly.limit, Some(60_000_000.0));
    assert_eq!(weekly.percent_used, Some(0.75));
    assert_eq!(usage.daily.expect("daily").limit, Some(5000.0));
    assert_eq!(usage.monthly.expect("monthly").limit, Some(60000.0));
    assert_eq!(usage.current_period_end.as_deref(), Some("1785196800"));
    assert!(parse_usage(&json!({})).is_err());
    assert!(parse_usage(&json!({"weekly":{"used":"invalid"}})).is_err());
}
