use pool_core::openclaw::NodeManager;
use pool_core::openclaw::{EmbeddingStore, EmbeddingType};
use pool_core::openclaw::{FeishuBot, TelegramBot};
use pool_core::openclaw::{McpServer, McpResource};
use pool_core::openclaw::{LlmBar, LlmProvider};
use pool_core::models::{Node, NodeType};
use std::collections::HashMap;

#[test]
fn test_node_manager_create_node() {
    let mut manager = NodeManager::new();

    let node = Node {
        id: "test_1".to_string(),
        node_type: NodeType::TextPrompt,
        position: (0.0, 0.0),
        params: HashMap::new(),
    };

    manager.create_node(node.clone()).unwrap();
    assert!(manager.get_node("test_1").is_some());
}

#[test]
fn test_node_manager_list_nodes() {
    let mut manager = NodeManager::new();

    let node1 = Node {
        id: "node_1".to_string(),
        node_type: NodeType::TextPrompt,
        position: (0.0, 0.0),
        params: HashMap::new(),
    };
    let node2 = Node {
        id: "node_2".to_string(),
        node_type: NodeType::VISCCore,
        position: (100.0, 0.0),
        params: HashMap::new(),
    };

    manager.create_node(node1).unwrap();
    manager.create_node(node2).unwrap();

    let nodes = manager.list_nodes();
    assert_eq!(nodes.len(), 2);
}

// EmbeddingStore Tests
#[test]
fn test_embedding_store_create() {
    let mut store = EmbeddingStore::new();
    let vector = vec![0.1, 0.2, 0.3, 0.4, 0.5];

    let id = store.create_embedding("hero_character", EmbeddingType::Character, vector).unwrap();
    assert!(!id.is_empty());

    let embedding = store.get_embedding(&id).unwrap();
    assert_eq!(embedding.name, "hero_character");
    assert_eq!(embedding.embedding_type, EmbeddingType::Character);
}

#[test]
fn test_embedding_store_get_by_name() {
    let mut store = EmbeddingStore::new();
    let vector = vec![1.0, 0.0, 0.0];

    store.create_embedding("cyberpunk_style", EmbeddingType::Style, vector).unwrap();

    let embedding = store.get_embedding_by_name("cyberpunk_style").unwrap();
    assert_eq!(embedding.embedding_type, EmbeddingType::Style);
}

#[test]
fn test_embedding_store_list_by_type() {
    let mut store = EmbeddingStore::new();

    store.create_embedding("char1", EmbeddingType::Character, vec![1.0]).unwrap();
    store.create_embedding("char2", EmbeddingType::Character, vec![2.0]).unwrap();
    store.create_embedding("style1", EmbeddingType::Style, vec![3.0]).unwrap();

    let characters = store.list_by_type(EmbeddingType::Character);
    assert_eq!(characters.len(), 2);

    let styles = store.list_by_type(EmbeddingType::Style);
    assert_eq!(styles.len(), 1);

    let scenes = store.list_by_type(EmbeddingType::Scene);
    assert_eq!(scenes.len(), 0);
}

#[test]
fn test_cosine_similarity() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    let c = vec![0.0, 1.0, 0.0];
    let d = vec![1.0, 1.0, 0.0];

    // Same vector
    let sim = EmbeddingStore::cosine_similarity(&a, &b);
    assert!((sim - 1.0).abs() < 0.0001);

    // Orthogonal vectors
    let sim = EmbeddingStore::cosine_similarity(&a, &c);
    assert!((sim - 0.0).abs() < 0.0001);

    // 45 degree angle
    let sim = EmbeddingStore::cosine_similarity(&a, &d);
    assert!((sim - 0.7071).abs() < 0.01);
}

// FeishuBot Tests
#[test]
fn test_feishu_bot_creation() {
    let bot = FeishuBot::new("app_123".to_string(), "secret_456".to_string());
    assert_eq!(bot.app_id(), "app_123");
}

// TelegramBot Tests
#[test]
fn test_telegram_bot_creation() {
    let bot = TelegramBot::new("bot_token_789".to_string());
    assert_eq!(bot.token(), "bot_token_789");
}

// McpServer Tests
#[test]
fn test_mcp_server_list_resources() {
    let server = McpServer::new();
    let resources = server.list_resources();

    assert_eq!(resources.len(), 2);
    assert!(resources.iter().any(|r| r.uri == "pool://status"));
    assert!(resources.iter().any(|r| r.uri == "pool://shots"));
}

#[test]
fn test_mcp_server_read_resource() {
    let server = McpServer::new();

    let status = server.read_resource("pool://status").unwrap();
    assert!(status.contains("idle"));

    let unknown = server.read_resource("pool://unknown");
    assert!(unknown.is_err());
}

// LlmBar Tests
#[test]
fn test_llm_bar_creation() {
    let bar = LlmBar::new(LlmProvider::Zhipu);
    assert_eq!(bar.provider(), LlmProvider::Zhipu);
}

#[test]
fn test_llm_bar_set_api_key() {
    let mut bar = LlmBar::new(LlmProvider::Claude);
    bar.set_api_key("sk-test-key".to_string());
    // API key is stored internally, no public getter in spec
}

#[test]
fn test_llm_providers() {
    let providers = vec![
        LlmProvider::Zhipu,
        LlmProvider::Minimax,
        LlmProvider::Kimi,
        LlmProvider::Claude,
    ];

    for provider in providers {
        let bar = LlmBar::new(provider.clone());
        assert_eq!(bar.provider(), provider);
    }
}
