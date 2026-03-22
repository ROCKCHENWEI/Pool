use pool_core::engine::NodeEngine;
use pool_core::models::{Connection, Node, NodeType, NodeParam, Workflow};
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

#[test]
fn test_node_engine_get_node() {
    let mut engine = NodeEngine::new();

    let node = Node {
        id: "test_node".to_string(),
        node_type: NodeType::TextPrompt,
        position: (0.0, 0.0),
        params: HashMap::new(),
    };

    engine.add_node(node);

    let retrieved = engine.get_node("test_node");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, "test_node");

    let missing = engine.get_node("nonexistent");
    assert!(missing.is_none());
}

#[test]
fn test_node_engine_get_connections() {
    let mut engine = NodeEngine::new();

    engine.add_node(Node {
        id: "a".to_string(),
        node_type: NodeType::TextPrompt,
        position: (0.0, 0.0),
        params: HashMap::new(),
    });
    engine.add_node(Node {
        id: "b".to_string(),
        node_type: NodeType::Output,
        position: (100.0, 0.0),
        params: HashMap::new(),
    });

    engine.add_connection(Connection {
        from_node: "a".to_string(),
        from_slot: 0,
        to_node: "b".to_string(),
        to_slot: 0,
    });

    let connections = engine.get_connections();
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].from_node, "a");
    assert_eq!(connections[0].to_node, "b");
}

#[test]
fn test_comfyui_workflow_conversion() {
    let mut engine = NodeEngine::new();

    // Add ComfyUI nodes
    engine.add_node(Node {
        id: "checkpoint".to_string(),
        node_type: NodeType::ComfyUILoadCheckpoint,
        position: (0.0, 0.0),
        params: {
            let mut p = HashMap::new();
            p.insert("checkpoint".to_string(), NodeParam::String("v1-5-pruned.safetensors".to_string()));
            p
        },
    });

    engine.add_node(Node {
        id: "text_encode".to_string(),
        node_type: NodeType::ComfyUITextEncode,
        position: (100.0, 0.0),
        params: {
            let mut p = HashMap::new();
            p.insert("text".to_string(), NodeParam::String("a beautiful landscape".to_string()));
            p
        },
    });

    engine.add_node(Node {
        id: "sampler".to_string(),
        node_type: NodeType::ComfyUIKSampler,
        position: (200.0, 0.0),
        params: {
            let mut p = HashMap::new();
            p.insert("seed".to_string(), NodeParam::Integer(12345));
            p.insert("steps".to_string(), NodeParam::Integer(20));
            p.insert("cfg".to_string(), NodeParam::Float(7.0));
            p
        },
    });

    let workflow = engine.to_comfyui_workflow();
    assert!(!workflow.is_empty());
}

#[test]
fn test_topological_sort_parallel_nodes() {
    let mut engine = NodeEngine::new();

    // Create parallel branches: A -> B, A -> C
    engine.add_node(Node {
        id: "a".to_string(),
        node_type: NodeType::TextPrompt,
        position: (0.0, 0.0),
        params: HashMap::new(),
    });
    engine.add_node(Node {
        id: "b".to_string(),
        node_type: NodeType::VISCCore,
        position: (100.0, 0.0),
        params: HashMap::new(),
    });
    engine.add_node(Node {
        id: "c".to_string(),
        node_type: NodeType::VISCCore,
        position: (100.0, 100.0),
        params: HashMap::new(),
    });

    engine.add_connection(Connection {
        from_node: "a".to_string(),
        from_slot: 0,
        to_node: "b".to_string(),
        to_slot: 0,
    });
    engine.add_connection(Connection {
        from_node: "a".to_string(),
        from_slot: 0,
        to_node: "c".to_string(),
        to_slot: 0,
    });

    let sorted = engine.topological_sort().unwrap();
    assert_eq!(sorted[0], "a"); // A must come first
    assert!(sorted.contains(&"b".to_string()));
    assert!(sorted.contains(&"c".to_string()));
}

#[test]
fn test_workflow_executor_creation() {
    use pool_core::engine::WorkflowExecutor;

    let workflow = Workflow::new("Test Workflow".to_string(), "shot_1".to_string());
    let executor = WorkflowExecutor::new(&workflow);

    // Validation should succeed for empty workflow
    assert!(executor.validate().is_ok());
}

#[test]
fn test_workflow_executor_with_comfyui() {
    use pool_core::engine::WorkflowExecutor;
    use pool_core::models::ComfyUIConfig;

    let workflow = Workflow::new("Test Workflow".to_string(), "shot_1".to_string());
    let config = ComfyUIConfig::default();
    let executor = WorkflowExecutor::with_comfyui(&workflow, config);

    assert!(executor.validate().is_ok());
}
