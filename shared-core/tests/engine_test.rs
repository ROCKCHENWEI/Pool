use pool_core::engine::NodeEngine;
use pool_core::models::{Connection, Node, NodeType};
use std::collections::HashMap;

#[test]
fn test_topological_sort_simple() {
    let mut engine = NodeEngine::new();

    // Create nodes: A -> B -> C
    let node_a = Node {
        id: "a".to_string(),
        node_type: NodeType::TextPrompt,
        position: (0.0, 0.0),
        params: HashMap::new(),
    };
    let node_b = Node {
        id: "b".to_string(),
        node_type: NodeType::VISCCore,
        position: (100.0, 0.0),
        params: HashMap::new(),
    };
    let node_c = Node {
        id: "c".to_string(),
        node_type: NodeType::Output,
        position: (200.0, 0.0),
        params: HashMap::new(),
    };

    engine.add_node(node_a);
    engine.add_node(node_b);
    engine.add_node(node_c);

    engine.add_connection(Connection {
        from_node: "a".to_string(),
        from_slot: 0,
        to_node: "b".to_string(),
        to_slot: 0,
    });
    engine.add_connection(Connection {
        from_node: "b".to_string(),
        from_slot: 0,
        to_node: "c".to_string(),
        to_slot: 0,
    });

    let sorted = engine.topological_sort().unwrap();
    assert_eq!(sorted, vec!["a", "b", "c"]);
}

#[test]
fn test_detect_cycle() {
    let mut engine = NodeEngine::new();

    // Create cycle: A -> B -> A
    let node_a = Node {
        id: "a".to_string(),
        node_type: NodeType::TextPrompt,
        position: (0.0, 0.0),
        params: HashMap::new(),
    };
    let node_b = Node {
        id: "b".to_string(),
        node_type: NodeType::VISCCore,
        position: (100.0, 0.0),
        params: HashMap::new(),
    };

    engine.add_node(node_a);
    engine.add_node(node_b);

    engine.add_connection(Connection {
        from_node: "a".to_string(),
        from_slot: 0,
        to_node: "b".to_string(),
        to_slot: 0,
    });
    engine.add_connection(Connection {
        from_node: "b".to_string(),
        from_slot: 0,
        to_node: "a".to_string(),
        to_slot: 0,
    });

    assert!(engine.topological_sort().is_err());
}
