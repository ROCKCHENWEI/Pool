//! Swift FFI Bindings
//!
//! C-compatible functions for Swift interop on macOS.
//! All strings are passed as UTF-8 C strings (null-terminated).
//! Complex data types are serialized to JSON for transfer.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

/// Create a new project and return it as a JSON string.
///
/// # Safety
/// The `name` parameter must be a valid null-terminated C string.
///
/// # Returns
/// A newly allocated C string containing the JSON representation of the project.
/// The caller must free this string using `pool_string_free`.
#[no_mangle]
pub extern "C" fn pool_project_create(name: *const c_char) -> *mut c_char {
    if name.is_null() {
        let error = r#"{"error": "name parameter is null"}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error": "invalid UTF-8 in name"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let project = crate::models::Project::new(name_str.to_string());

    match serde_json::to_string(&project) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error": "failed to serialize project"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

/// Free a string returned by the library.
///
/// # Safety
/// The `s` parameter must be a pointer previously returned by one of the
/// library's allocation functions, or null. Passing any other pointer
/// results in undefined behavior.
#[no_mangle]
pub extern "C" fn pool_string_free(s: *mut c_char) {
    unsafe {
        if !s.is_null() {
            drop(CString::from_raw(s));
        }
    }
}

/// Get the library version.
///
/// # Returns
/// A newly allocated C string containing the version (e.g., "0.1.0").
/// The caller must free this string using `pool_string_free`.
#[no_mangle]
pub extern "C" fn pool_version() -> *mut c_char {
    CString::new(env!("CARGO_PKG_VERSION")).unwrap().into_raw()
}

/// Create a new shot for a project and return it as a JSON string.
///
/// # Safety
/// The `project_id` and `name` parameters must be valid null-terminated C strings.
///
/// # Returns
/// A newly allocated C string containing the JSON representation of the shot.
/// The caller must free this string using `pool_string_free`.
#[no_mangle]
pub extern "C" fn pool_shot_create(project_id: *const c_char, name: *const c_char) -> *mut c_char {
    if project_id.is_null() || name.is_null() {
        let error = r#"{"error": "parameter is null"}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let project_id_str = match unsafe { CStr::from_ptr(project_id) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error": "invalid UTF-8 in project_id"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error": "invalid UTF-8 in name"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let shot = crate::models::Shot::new(name_str.to_string()).with_project(project_id_str.to_string());

    match serde_json::to_string(&shot) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error": "failed to serialize shot"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

/// Create a new workflow and return it as a JSON string.
///
/// # Safety
/// The `name` parameter must be a valid null-terminated C string.
///
/// # Returns
/// A newly allocated C string containing the JSON representation of the workflow.
/// The caller must free this string using `pool_string_free`.
#[no_mangle]
pub extern "C" fn pool_workflow_create(name: *const c_char) -> *mut c_char {
    if name.is_null() {
        let error = r#"{"error": "name parameter is null"}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error": "invalid UTF-8 in name"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let workflow = crate::models::Workflow::new(name_str.to_string(), String::new());

    match serde_json::to_string(&workflow) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error": "failed to serialize workflow"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

// ============================================================================
// ComfyUI FFI Bindings
// ============================================================================

/// Global tokio runtime for async FFI calls
static FFI_RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Get or create the global runtime
fn get_runtime() -> &'static Runtime {
    FFI_RUNTIME.get_or_init(|| {
        Runtime::new().expect("Failed to create tokio runtime for FFI")
    })
}

/// Global ComfyUI configuration storage
static COMFYUI_CONFIG: OnceLock<std::sync::Mutex<crate::models::ComfyUIConfig>> = OnceLock::new();

fn get_comfyui_config() -> &'static std::sync::Mutex<crate::models::ComfyUIConfig> {
    COMFYUI_CONFIG.get_or_init(|| {
        std::sync::Mutex::new(crate::models::ComfyUIConfig::default())
    })
}

/// Set ComfyUI configuration.
///
/// # Safety
/// All string parameters must be valid null-terminated C strings.
///
/// # Returns
/// JSON string with success status or error message.
#[no_mangle]
pub extern "C" fn pool_comfyui_set_config(
    server_url: *const c_char,
    timeout_secs: u64,
    auto_reconnect: bool,
    max_retries: u32,
) -> *mut c_char {
    let server_url_str = if server_url.is_null() {
        "http://127.0.0.1:8188".to_string()
    } else {
        match unsafe { CStr::from_ptr(server_url) }.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                let error = r#"{"success":false,"error":"invalid UTF-8 in server_url"}"#;
                return CString::new(error).unwrap().into_raw();
            }
        }
    };

    let mut config = crate::models::ComfyUIConfig::new(server_url_str);
    config.timeout_secs = timeout_secs;
    config.auto_reconnect = auto_reconnect;
    config.max_retries = max_retries;

    // Validate configuration
    if let Err(e) = config.validate() {
        let error = format!(r#"{{"success":false,"error":"{}"}}"#, e);
        return CString::new(error).unwrap().into_raw();
    }

    // Store configuration
    if let Ok(mut cfg) = get_comfyui_config().lock() {
        *cfg = config;
    }

    let result = r#"{"success":true}"#;
    CString::new(result).unwrap().into_raw()
}

/// Get current ComfyUI configuration.
///
/// # Returns
/// JSON string with configuration or error message.
#[no_mangle]
pub extern "C" fn pool_comfyui_get_config() -> *mut c_char {
    let config = if let Ok(cfg) = get_comfyui_config().lock() {
        cfg.clone()
    } else {
        crate::models::ComfyUIConfig::default()
    };

    match serde_json::to_string(&config) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error":"failed to serialize config"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

/// Test connection to ComfyUI server.
///
/// # Returns
/// JSON string with connection status:
/// - {"status":"connected","message":"Connection successful"}
/// - {"status":"error","message":"Connection failed: ..."}
#[no_mangle]
pub extern "C" fn pool_comfyui_test_connection() -> *mut c_char {
    let config = if let Ok(cfg) = get_comfyui_config().lock() {
        cfg.clone()
    } else {
        let error = r#"{"status":"error","message":"Failed to get config"}"#;
        return CString::new(error).unwrap().into_raw();
    };

    let runtime = get_runtime();
    let result = runtime.block_on(async {
        let client = crate::comfyui::ComfyUIClient::new(&config.server_url);
        match client.get_system_stats().await {
            Ok(stats) => {
                let stats_json = serde_json::to_string(&stats).unwrap_or_default();
                format!(r#"{{"status":"connected","message":"Connection successful","stats":{}}}"#, stats_json)
            }
            Err(e) => {
                format!(r#"{{"status":"error","message":"Connection failed: {}"}}"#, e)
            }
        }
    });

    CString::new(result).unwrap().into_raw()
}

/// Connection status for callback
#[repr(C)]
pub struct PoolConnectionStatus {
    pub connected: bool,
    pub message: *mut c_char,
}

/// Connect to ComfyUI server.
///
/// # Returns
/// JSON string with connection status.
#[no_mangle]
pub extern "C" fn pool_comfyui_connect() -> *mut c_char {
    let config = if let Ok(cfg) = get_comfyui_config().lock() {
        cfg.clone()
    } else {
        let error = r#"{"success":false,"error":"Failed to get config"}"#;
        return CString::new(error).unwrap().into_raw();
    };

    let runtime = get_runtime();
    let result = runtime.block_on(async {
        let client = crate::comfyui::ComfyUIClient::new(&config.server_url);

        // Test connection first
        match client.get_system_stats().await {
            Ok(_) => {
                // Connection successful
                r#"{"success":true,"message":"Connected to ComfyUI server"}"#.to_string()
            }
            Err(e) => {
                format!(r#"{{"success":false,"error":"Connection failed: {}"}}"#, e)
            }
        }
    });

    CString::new(result).unwrap().into_raw()
}

/// Get ComfyUI workflow templates.
///
/// # Returns
/// JSON string with array of templates.
#[no_mangle]
pub extern "C" fn pool_comfyui_get_templates() -> *mut c_char {
    let templates = crate::models::ComfyUITemplateLibrary::get_templates();

    match serde_json::to_string(&templates) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error":"failed to serialize templates"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

// ============================================================================
// Workflow Execution FFI Bindings
// ============================================================================

/// Global workflow execution manager
static WORKFLOW_MANAGER: OnceLock<std::sync::Arc<crate::engine::WorkflowExecutionManager>> = OnceLock::new();

fn get_workflow_manager() -> &'static std::sync::Arc<crate::engine::WorkflowExecutionManager> {
    WORKFLOW_MANAGER.get_or_init(|| {
        std::sync::Arc::new(crate::engine::WorkflowExecutionManager::new())
    })
}

/// Execute a workflow.
///
/// # Safety
/// The `workflow_json` parameter must be a valid null-terminated C string
/// containing a JSON representation of the workflow.
///
/// # Returns
/// JSON string with execution result:
/// - {"success":true,"execution_id":"...","status":"running"}
/// - {"success":false,"error":"..."}
#[no_mangle]
pub extern "C" fn pool_workflow_execute(workflow_json: *const c_char) -> *mut c_char {
    if workflow_json.is_null() {
        let error = r#"{"success":false,"error":"workflow_json is null"}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let json_str = match unsafe { CStr::from_ptr(workflow_json) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"success":false,"error":"invalid UTF-8 in workflow_json"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let workflow: crate::models::Workflow = match serde_json::from_str(json_str) {
        Ok(w) => w,
        Err(e) => {
            let error = format!(r#"{{"success":false,"error":"Failed to parse workflow: {}"}}"#, e);
            return CString::new(error).unwrap().into_raw();
        }
    };

    // Get ComfyUI config and create executor
    let comfyui_config = if let Ok(cfg) = get_comfyui_config().lock() {
        cfg.clone()
    } else {
        crate::models::ComfyUIConfig::default()
    };

    let runtime = get_runtime();
    let result = runtime.block_on(async {
        let executor = crate::engine::WorkflowExecutor::with_comfyui(&workflow, comfyui_config);

        // Try to connect to ComfyUI first
        if let Err(e) = executor.connect_comfyui().await {
            return format!(r#"{{"success":false,"error":"Failed to connect to ComfyUI: {}"}}"#, e);
        }

        // Execute the workflow
        match executor.execute().await {
            Ok(result) => {
                match serde_json::to_string(&result) {
                    Ok(json) => format!(r#"{{"success":true,"result":{}}}"#, json),
                    Err(_) => r#"{"success":false,"error":"Failed to serialize result"}"#.to_string()
                }
            }
            Err(e) => {
                format!(r#"{{"success":false,"error":"Execution failed: {}"}}"#, e)
            }
        }
    });

    CString::new(result).unwrap().into_raw()
}

/// Get node types available for workflow creation.
///
/// # Returns
/// JSON string with array of node type information.
#[no_mangle]
pub extern "C" fn pool_workflow_get_node_types() -> *mut c_char {
    use crate::models::NodeType;

    let node_types: Vec<serde_json::Value> = vec![
        serde_json::json!({"name": "TextPrompt", "category": "Input", "description": "Text input for prompts"}),
        serde_json::json!({"name": "VISCCore", "category": "AI", "description": "VISC Core processing"}),
        serde_json::json!({"name": "SuperResolution", "category": "Processing", "description": "Upscale and enhance"}),
        serde_json::json!({"name": "HDR", "category": "Processing", "description": "HDR tone mapping"}),
        serde_json::json!({"name": "ColorGrade", "category": "Processing", "description": "Color grading"}),
        serde_json::json!({"name": "Subtitle", "category": "Processing", "description": "Add subtitles"}),
        serde_json::json!({"name": "Output", "category": "Output", "description": "Output node"}),
        serde_json::json!({"name": "ComfyUITextEncode", "category": "ComfyUI", "description": "CLIP text encoding"}),
        serde_json::json!({"name": "ComfyUIKSampler", "category": "ComfyUI", "description": "KSampler for diffusion"}),
        serde_json::json!({"name": "ComfyUIVAEDecode", "category": "ComfyUI", "description": "VAE decode latent to image"}),
        serde_json::json!({"name": "ComfyUISaveImage", "category": "ComfyUI", "description": "Save image to disk"}),
        serde_json::json!({"name": "ComfyUILoadCheckpoint", "category": "ComfyUI", "description": "Load model checkpoint"}),
        serde_json::json!({"name": "ComfyUIEmptyLatentImage", "category": "ComfyUI", "description": "Create empty latent"}),
    ];

    match serde_json::to_string(&node_types) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error":"failed to serialize node types"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

/// Create a sample workflow with ComfyUI nodes.
///
/// # Returns
/// JSON string with a sample text-to-image workflow.
#[no_mangle]
pub extern "C" fn pool_workflow_create_sample() -> *mut c_char {
    use crate::models::{Node, NodeParam, Connection, NodeType};
    use std::collections::HashMap;

    let workflow = crate::models::Workflow {
        id: uuid::Uuid::new_v4().to_string(),
        shot_id: String::new(),
        name: "Sample Text-to-Image Workflow".to_string(),
        nodes: vec![
            Node {
                id: "checkpoint".to_string(),
                node_type: NodeType::ComfyUILoadCheckpoint,
                position: (100.0, 100.0),
                params: {
                    let mut m = HashMap::new();
                    m.insert("checkpoint".to_string(), NodeParam::String("v1-5-pruned.safetensors".to_string()));
                    m
                },
            },
            Node {
                id: "latent".to_string(),
                node_type: NodeType::ComfyUIEmptyLatentImage,
                position: (100.0, 250.0),
                params: {
                    let mut m = HashMap::new();
                    m.insert("width".to_string(), NodeParam::Integer(512));
                    m.insert("height".to_string(), NodeParam::Integer(512));
                    m
                },
            },
            Node {
                id: "positive".to_string(),
                node_type: NodeType::ComfyUITextEncode,
                position: (300.0, 100.0),
                params: {
                    let mut m = HashMap::new();
                    m.insert("text".to_string(), NodeParam::String("a beautiful landscape".to_string()));
                    m
                },
            },
            Node {
                id: "negative".to_string(),
                node_type: NodeType::ComfyUITextEncode,
                position: (300.0, 250.0),
                params: {
                    let mut m = HashMap::new();
                    m.insert("text".to_string(), NodeParam::String("ugly, blurry, low quality".to_string()));
                    m
                },
            },
            Node {
                id: "sampler".to_string(),
                node_type: NodeType::ComfyUIKSampler,
                position: (500.0, 175.0),
                params: {
                    let mut m = HashMap::new();
                    m.insert("seed".to_string(), NodeParam::Integer(123456789));
                    m.insert("steps".to_string(), NodeParam::Integer(20));
                    m.insert("cfg".to_string(), NodeParam::Float(7.0));
                    m
                },
            },
            Node {
                id: "decode".to_string(),
                node_type: NodeType::ComfyUIVAEDecode,
                position: (700.0, 175.0),
                params: HashMap::new(),
            },
            Node {
                id: "save".to_string(),
                node_type: NodeType::ComfyUISaveImage,
                position: (900.0, 175.0),
                params: {
                    let mut m = HashMap::new();
                    m.insert("filename_prefix".to_string(), NodeParam::String("Pool_".to_string()));
                    m
                },
            },
        ],
        connections: vec![
            Connection {
                from_node: "checkpoint".to_string(),
                from_slot: 0,
                to_node: "sampler".to_string(),
                to_slot: 0,
            },
            Connection {
                from_node: "positive".to_string(),
                from_slot: 0,
                to_node: "sampler".to_string(),
                to_slot: 1,
            },
            Connection {
                from_node: "negative".to_string(),
                from_slot: 0,
                to_node: "sampler".to_string(),
                to_slot: 2,
            },
            Connection {
                from_node: "latent".to_string(),
                from_slot: 0,
                to_node: "sampler".to_string(),
                to_slot: 3,
            },
            Connection {
                from_node: "sampler".to_string(),
                from_slot: 0,
                to_node: "decode".to_string(),
                to_slot: 0,
            },
            Connection {
                from_node: "decode".to_string(),
                from_slot: 0,
                to_node: "save".to_string(),
                to_slot: 0,
            },
        ],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    match serde_json::to_string(&workflow) {
        Ok(json) => CString::new(json).unwrap().into_raw(),
        Err(_) => {
            let error = r#"{"error":"failed to serialize workflow"}"#;
            CString::new(error).unwrap().into_raw()
        }
    }
}

// ============================================================================
// Real-time Progress FFI Bindings
// ============================================================================

/// Global WebSocket connection for progress updates
static WEBSOCKET_CLIENT: OnceLock<std::sync::Arc<tokio::sync::Mutex<Option<crate::comfyui::websocket::ComfyUIWebSocket>>>> = OnceLock::new();

fn get_websocket_client() -> &'static std::sync::Arc<tokio::sync::Mutex<Option<crate::comfyui::websocket::ComfyUIWebSocket>>> {
    WEBSOCKET_CLIENT.get_or_init(|| std::sync::Arc::new(tokio::sync::Mutex::new(None)))
}

/// Progress update information for FFI
#[repr(C)]
pub struct PoolProgressUpdate {
    pub node: *mut c_char,
    pub value: f32,
    pub max: f32,
}

/// Execution update information for FFI
#[repr(C)]
pub struct PoolExecutionUpdate {
    pub prompt_id: *mut c_char,
    pub status: *mut c_char,
    pub progress: f32,
    pub message: *mut c_char,
}

/// Connect to ComfyUI WebSocket for real-time updates.
///
/// # Returns
/// JSON string with connection status.
#[no_mangle]
pub extern "C" fn pool_websocket_connect() -> *mut c_char {
    let config = if let Ok(cfg) = get_comfyui_config().lock() {
        cfg.clone()
    } else {
        let error = r#"{"success":false,"error":"Failed to get config"}"#;
        return CString::new(error).unwrap().into_raw();
    };

    let runtime = get_runtime();
    let result = runtime.block_on(async {
        let ws = crate::comfyui::websocket::ComfyUIWebSocket::new(&config.server_url);
        match ws.connect().await {
            Ok(_) => {
                let mut client = get_websocket_client().lock().await;
                *client = Some(ws);
                r#"{"success":true,"message":"WebSocket connected"}"#.to_string()
            }
            Err(e) => {
                format!(r#"{{"success":false,"error":"{}"}}"#, e)
            }
        }
    });

    CString::new(result).unwrap().into_raw()
}

/// Disconnect WebSocket.
#[no_mangle]
pub extern "C" fn pool_websocket_disconnect() {
    let runtime = get_runtime();
    runtime.block_on(async {
        let mut client = get_websocket_client().lock().await;
        *client = None;
    });
}

/// Check if WebSocket is connected.
///
/// # Returns
/// JSON string with connection status.
#[no_mangle]
pub extern "C" fn pool_websocket_is_connected() -> *mut c_char {
    let runtime = get_runtime();
    let connected = runtime.block_on(async {
        if let Some(ws) = get_websocket_client().lock().await.as_ref() {
            ws.is_connected().await
        } else {
            false
        }
    });

    let result = format!(r#"{{"connected":{}}}"#, connected);
    CString::new(result).unwrap().into_raw()
}

/// Try to receive a progress update (non-blocking).
///
/// # Returns
/// JSON string with progress update or null if no update available.
#[no_mangle]
pub extern "C" fn pool_progress_try_recv() -> *mut c_char {
    let runtime = get_runtime();
    let result = runtime.block_on(async {
        if let Some(ws) = get_websocket_client().lock().await.as_ref() {
            let mut receiver = ws.subscribe_progress();
            // Non-blocking try_recv
            match receiver.try_recv() {
                Ok(update) => {
                    match serde_json::to_string(&update) {
                        Ok(json) => json,
                        Err(_) => r#"{"error":"failed to serialize"}"#.to_string()
                    }
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    r#"{"available":false}"#.to_string()
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                    r#"{"error":"channel closed"}"#.to_string()
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                    r#"{"error":"lagged"}"#.to_string()
                }
            }
        } else {
            r#"{"error":"not connected"}"#.to_string()
        }
    });

    CString::new(result).unwrap().into_raw()
}

/// Try to receive an execution update (non-blocking).
///
/// # Returns
/// JSON string with execution update or null if no update available.
#[no_mangle]
pub extern "C" fn pool_execution_try_recv() -> *mut c_char {
    let runtime = get_runtime();
    let result = runtime.block_on(async {
        if let Some(ws) = get_websocket_client().lock().await.as_ref() {
            let mut receiver = ws.subscribe_execution();
            match receiver.try_recv() {
                Ok(update) => {
                    match serde_json::to_string(&update) {
                        Ok(json) => json,
                        Err(_) => r#"{"error":"failed to serialize"}"#.to_string()
                    }
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    r#"{"available":false}"#.to_string()
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                    r#"{"error":"channel closed"}"#.to_string()
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                    r#"{"error":"lagged"}"#.to_string()
                }
            }
        } else {
            r#"{"error":"not connected"}"#.to_string()
        }
    });

    CString::new(result).unwrap().into_raw()
}

/// Execute a workflow with real-time progress updates.
///
/// # Safety
/// The `workflow_json` parameter must be a valid null-terminated C string.
///
/// # Returns
/// JSON string with execution ID for tracking progress.
#[no_mangle]
pub extern "C" fn pool_workflow_execute_async(workflow_json: *const c_char) -> *mut c_char {
    if workflow_json.is_null() {
        let error = r#"{"success":false,"error":"workflow_json is null"}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let json_str = match unsafe { CStr::from_ptr(workflow_json) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"success":false,"error":"invalid UTF-8 in workflow_json"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let workflow: crate::models::Workflow = match serde_json::from_str(json_str) {
        Ok(w) => w,
        Err(e) => {
            let error = format!(r#"{{"success":false,"error":"Failed to parse workflow: {}"}}"#, e);
            return CString::new(error).unwrap().into_raw();
        }
    };

    // Get ComfyUI config
    let comfyui_config = if let Ok(cfg) = get_comfyui_config().lock() {
        cfg.clone()
    } else {
        crate::models::ComfyUIConfig::default()
    };

    let runtime = get_runtime();
    let result = runtime.block_on(async {
        let executor = crate::engine::WorkflowExecutor::with_comfyui(&workflow, comfyui_config);

        // Try to connect to ComfyUI
        if let Err(e) = executor.connect_comfyui().await {
            return format!(r#"{{"success":false,"error":"Failed to connect to ComfyUI: {}"}}"#, e);
        }

        // Start execution in background
        let workflow_id = workflow.id.clone();

        // For async execution, we spawn a task and return immediately
        let _ = tokio::spawn(async move {
            let _ = executor.execute().await;
        });

        format!(r#"{{"success":true,"execution_id":"{}"}}"#, workflow_id)
    });

    CString::new(result).unwrap().into_raw()
}

/// Get the output path for generated images.
///
/// # Returns
/// JSON string with output directory path.
#[no_mangle]
pub extern "C" fn pool_get_output_path() -> *mut c_char {
    // Default output path
    let output_path = std::env::var("POOL_OUTPUT_PATH")
        .unwrap_or_else(|_| "./output".to_string());

    let result = format!(r#"{{"path":"{}"}}"#, output_path);
    CString::new(result).unwrap().into_raw()
}

/// Get list of generated files for a prompt ID.
///
/// # Returns
/// JSON string with array of file paths.
#[no_mangle]
pub extern "C" fn pool_get_generated_files(prompt_id: *const c_char) -> *mut c_char {
    if prompt_id.is_null() {
        let error = r#"{"files":[]}"#;
        return CString::new(error).unwrap().into_raw();
    }

    let prompt_id_str = match unsafe { CStr::from_ptr(prompt_id) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            let error = r#"{"error":"invalid UTF-8"}"#;
            return CString::new(error).unwrap().into_raw();
        }
    };

    let config = if let Ok(cfg) = get_comfyui_config().lock() {
        cfg.clone()
    } else {
        crate::models::ComfyUIConfig::default()
    };

    let runtime = get_runtime();
    let result = runtime.block_on(async {
        let client = crate::comfyui::ComfyUIClient::new(&config.server_url);

        match client.get_history(prompt_id_str).await {
            Ok(history) => {
                let mut files: Vec<String> = Vec::new();

                if let Some(outputs) = history.get("outputs").and_then(|o| o.as_object()) {
                    for (_, output) in outputs {
                        if let Some(images) = output.get("images").and_then(|i| i.as_array()) {
                            for image in images {
                                if let (Some(filename), Some(subfolder), Some(img_type)) = (
                                    image.get("filename").and_then(|f| f.as_str()),
                                    image.get("subfolder").and_then(|s| s.as_str()),
                                    image.get("type").and_then(|t| t.as_str()),
                                ) {
                                    files.push(format!("{}/{}/{}", img_type, subfolder, filename));
                                }
                            }
                        }
                    }
                }

                match serde_json::to_string(&files) {
                    Ok(json) => format!(r#"{{"files":{}}}"#, json),
                    Err(_) => r#"{"files":[]}"#.to_string()
                }
            }
            Err(e) => {
                format!(r#"{{"error":"{}","files":[]}}"#, e)
            }
        }
    });

    CString::new(result).unwrap().into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_pool_version() {
        let version = pool_version();
        let version_str = unsafe { CStr::from_ptr(version) }.to_str().unwrap();
        assert!(!version_str.is_empty());
        pool_string_free(version);
    }

    #[test]
    fn test_pool_project_create() {
        let name = CString::new("Test Project").unwrap();
        let result = pool_project_create(name.as_ptr());
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(json.contains("Test Project"));
        assert!(json.contains("id"));
        pool_string_free(result);
    }

    #[test]
    fn test_pool_comfyui_set_config() {
        let url = CString::new("http://127.0.0.1:8188").unwrap();
        let result = pool_comfyui_set_config(url.as_ptr(), 30, true, 3);
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(json.contains("success"));
        pool_string_free(result);
    }

    #[test]
    fn test_pool_comfyui_get_config() {
        let result = pool_comfyui_get_config();
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(json.contains("server_url"));
        pool_string_free(result);
    }

    #[test]
    fn test_pool_comfyui_get_templates() {
        let result = pool_comfyui_get_templates();
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(json.contains("Text-to-Image"));
        pool_string_free(result);
    }

    #[test]
    fn test_pool_workflow_get_node_types() {
        let result = pool_workflow_get_node_types();
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(json.contains("ComfyUI"));
        pool_string_free(result);
    }

    #[test]
    fn test_pool_workflow_create_sample() {
        let result = pool_workflow_create_sample();
        let json = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(json.contains("nodes"));
        assert!(json.contains("connections"));
        pool_string_free(result);
    }
}
