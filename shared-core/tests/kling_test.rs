use pool_core::api::providers::KlingAdapter;

#[test]
fn test_kling_adapter_name() {
    let adapter = KlingAdapter::new("test_api_key".to_string());
    assert_eq!(adapter.name(), "kling");
}
