use pool_core::openclaw::NodeManager;
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
