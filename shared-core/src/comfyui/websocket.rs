use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// WebSocket client for ComfyUI real-time communication
pub struct ComfyUIWebSocket {
    url: String,
    connected: Arc<Mutex<bool>>,
    progress_sender: broadcast::Sender<ProgressUpdate>,
    execution_sender: broadcast::Sender<ExecutionUpdate>,
}

/// Progress update from ComfyUI during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// Node ID being processed (if applicable)
    pub node: Option<String>,
    /// Current progress value
    pub value: f32,
    /// Maximum progress value
    pub max: f32,
}

/// Execution status update from ComfyUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionUpdate {
    /// Prompt/task ID
    pub prompt_id: String,
    /// Current execution status
    pub status: ExecutionStatus,
    /// Progress percentage (0.0 - 1.0)
    pub progress: f32,
    /// Optional message (error details, etc.)
    pub message: Option<String>,
}

/// Execution status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// Task is pending in queue
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Success,
    /// Task failed
    Failed,
}

/// Internal message types from ComfyUI WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ComfyUIMessage {
    #[serde(rename = "status")]
    Status { data: StatusData },
    #[serde(rename = "progress")]
    Progress { data: ProgressData },
    #[serde(rename = "executing")]
    Executing { data: ExecutingData },
    #[serde(rename = "executed")]
    Executed { data: ExecutedData },
    #[serde(rename = "execution_error")]
    ExecutionError { data: ExecutionErrorData },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusData {
    status: Option<StatusInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusInfo {
    exec_info: Option<ExecInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecInfo {
    queue_remaining: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgressData {
    value: f32,
    max: f32,
    #[serde(rename = "prompt_id")]
    prompt_id: Option<String>,
    node: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutingData {
    node: Option<String>,
    #[serde(rename = "prompt_id")]
    prompt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutedData {
    #[serde(rename = "prompt_id")]
    prompt_id: String,
    node: String,
    output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionErrorData {
    #[serde(rename = "prompt_id")]
    prompt_id: String,
    node_type: Option<String>,
    node_id: Option<String>,
    exception_message: Option<String>,
}

impl ComfyUIWebSocket {
    /// Create a new WebSocket client for the given ComfyUI base URL
    pub fn new(base_url: &str) -> Self {
        let ws_url = base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        let (progress_tx, _) = broadcast::channel(64);
        let (execution_tx, _) = broadcast::channel(64);

        Self {
            url: format!("{}/ws", ws_url),
            connected: Arc::new(Mutex::new(false)),
            progress_sender: progress_tx,
            execution_sender: execution_tx,
        }
    }

    /// Get the WebSocket URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Check if WebSocket is connected
    pub async fn is_connected(&self) -> bool {
        *self.connected.lock().await
    }

    /// Subscribe to progress updates
    pub fn subscribe_progress(&self) -> broadcast::Receiver<ProgressUpdate> {
        self.progress_sender.subscribe()
    }

    /// Subscribe to execution updates
    pub fn subscribe_execution(&self) -> broadcast::Receiver<ExecutionUpdate> {
        self.execution_sender.subscribe()
    }

    /// Connect to the WebSocket server
    pub async fn connect(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.url)
            .await
            .context("Failed to connect to ComfyUI WebSocket")?;

        *self.connected.lock().await = true;
        tracing::info!("Connected to ComfyUI WebSocket: {}", self.url);

        let connected = self.connected.clone();
        let progress_sender = self.progress_sender.clone();
        let execution_sender = self.execution_sender.clone();

        // Spawn a task to handle incoming messages
        tokio::spawn(async move {
            let (_, mut read) = ws_stream.split();

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = Self::handle_message(
                            &text,
                            &progress_sender,
                            &execution_sender,
                        )
                        .await
                        {
                            tracing::warn!("Failed to handle WebSocket message: {}", e);
                        }
                    }
                    Ok(Message::Ping(data)) => {
                        tracing::trace!("Received ping: {:?}", data);
                    }
                    Ok(Message::Pong(data)) => {
                        tracing::trace!("Received pong: {:?}", data);
                    }
                    Ok(Message::Close(_)) => {
                        tracing::info!("WebSocket connection closed by server");
                        *connected.lock().await = false;
                        break;
                    }
                    Err(e) => {
                        tracing::error!("WebSocket error: {}", e);
                        *connected.lock().await = false;
                        break;
                    }
                    _ => {}
                }
            }

            *connected.lock().await = false;
            tracing::info!("WebSocket listener stopped");
        });

        Ok(())
    }

    /// Handle incoming WebSocket message
    async fn handle_message(
        text: &str,
        progress_sender: &broadcast::Sender<ProgressUpdate>,
        execution_sender: &broadcast::Sender<ExecutionUpdate>,
    ) -> Result<()> {
        let msg: ComfyUIMessage =
            serde_json::from_str(text).context("Failed to parse WebSocket message")?;

        match msg {
            ComfyUIMessage::Progress { data } => {
                let update = ProgressUpdate {
                    node: data.node,
                    value: data.value,
                    max: data.max,
                };
                let _ = progress_sender.send(update);

                // Also send execution update if we have a prompt_id
                if let Some(prompt_id) = data.prompt_id {
                    let exec_update = ExecutionUpdate {
                        prompt_id,
                        status: ExecutionStatus::Running,
                        progress: if data.max > 0.0 {
                            data.value / data.max
                        } else {
                            0.0
                        },
                        message: None,
                    };
                    let _ = execution_sender.send(exec_update);
                }
            }
            ComfyUIMessage::Executing { data } => {
                let update = ExecutionUpdate {
                    prompt_id: data.prompt_id,
                    status: ExecutionStatus::Running,
                    progress: 0.0,
                    message: data.node.map(|n| format!("Executing node: {}", n)),
                };
                let _ = execution_sender.send(update);
            }
            ComfyUIMessage::Executed { data } => {
                let update = ExecutionUpdate {
                    prompt_id: data.prompt_id,
                    status: ExecutionStatus::Success,
                    progress: 1.0,
                    message: Some(format!("Completed node: {}", data.node)),
                };
                let _ = execution_sender.send(update);
            }
            ComfyUIMessage::ExecutionError { data } => {
                let update = ExecutionUpdate {
                    prompt_id: data.prompt_id,
                    status: ExecutionStatus::Failed,
                    progress: 0.0,
                    message: data.exception_message.or(data.node_type.map(|t| {
                        format!("Error in {} node", t)
                    })),
                };
                let _ = execution_sender.send(update);
            }
            ComfyUIMessage::Status { data } => {
                if let Some(status) = data.status {
                    if let Some(exec_info) = status.exec_info {
                        tracing::debug!(
                            "Queue remaining: {:?}",
                            exec_info.queue_remaining
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Wait for a specific prompt to complete
    pub async fn wait_for_completion(&self, prompt_id: &str, timeout_secs: u64) -> Result<bool> {
        let mut receiver = self.subscribe_execution();
        let prompt_id = prompt_id.to_string();

        tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), async {
            loop {
                match receiver.recv().await {
                    Ok(update) => {
                        if update.prompt_id == prompt_id {
                            match update.status {
                                ExecutionStatus::Success => return Ok(true),
                                ExecutionStatus::Failed => {
                                    return Err(anyhow::anyhow!(
                                        "Execution failed: {}",
                                        update.message.unwrap_or_default()
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(anyhow::anyhow!("WebSocket channel closed"));
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Receiver lagged by {} messages", n);
                    }
                }
            }
        })
        .await
        .context("Timeout waiting for completion")?
    }
}

impl Default for ComfyUIWebSocket {
    fn default() -> Self {
        Self::new("http://127.0.0.1:8188")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_url_conversion() {
        let ws = ComfyUIWebSocket::new("http://127.0.0.1:8188");
        assert_eq!(ws.url(), "ws://127.0.0.1:8188/ws");

        let ws = ComfyUIWebSocket::new("https://example.com");
        assert_eq!(ws.url(), "wss://example.com/ws");
    }

    #[test]
    fn test_progress_update_serialization() {
        let update = ProgressUpdate {
            node: Some("3".to_string()),
            value: 10.0,
            max: 30.0,
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"node\":\"3\""));
        assert!(json.contains("\"value\":10.0"));
    }

    #[test]
    fn test_execution_status_serialization() {
        let update = ExecutionUpdate {
            prompt_id: "test-123".to_string(),
            status: ExecutionStatus::Running,
            progress: 0.5,
            message: None,
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"status\":\"Running\""));
    }
}
