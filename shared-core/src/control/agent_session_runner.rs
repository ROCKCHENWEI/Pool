use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::db::RuntimeRepository;
use crate::models::{AgentSession, RuntimeEvent, RuntimeEventLevel, RuntimeTask, TaskStatus};

use super::{AgentCliCommand, HermesCommand};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub enum AgentSessionKind {
    Hermes,
    AgentCli,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSessionRunReport {
    pub session_id: String,
    pub task_id: String,
    pub project_slug: String,
    pub kind: AgentSessionKind,
    pub status: TaskStatus,
    pub transcript_path: String,
    pub token_budget: Option<u64>,
    pub token_used: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<AgentSessionExecutionReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSessionExecutionReport {
    pub channel: AgentSessionExecutionChannel,
    pub attempted: bool,
    pub allowed: bool,
    pub ok: bool,
    pub argv: Vec<String>,
    pub endpoint: Option<String>,
    pub working_dir: Option<String>,
    pub exit_code: Option<i32>,
    pub status_code: Option<u16>,
    pub stdout: String,
    pub stderr: String,
    pub response_body: Option<String>,
    pub error: Option<String>,
}

pub type AgentCliExecutionReport = AgentSessionExecutionReport;

#[derive(Debug, Clone, Serialize)]
pub enum AgentSessionExecutionChannel {
    HermesHttp,
    AgentCli,
}

#[derive(Debug, Clone)]
pub struct AgentCliExecutionOptions {
    pub allowed_commands: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub max_output_bytes: usize,
    pub timeout_ms: u64,
}

impl Default for AgentCliExecutionOptions {
    fn default() -> Self {
        Self {
            allowed_commands: vec!["pool-cli".to_string()],
            working_dir: None,
            max_output_bytes: 16 * 1024,
            timeout_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HermesExecutionOptions {
    pub auth_token: Option<String>,
    pub max_response_bytes: usize,
    pub timeout_ms: u64,
}

impl Default for HermesExecutionOptions {
    fn default() -> Self {
        Self {
            auth_token: None,
            max_response_bytes: 16 * 1024,
            timeout_ms: 30_000,
        }
    }
}

pub struct AgentSessionRunner<'a> {
    repository: &'a RuntimeRepository,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentSessionExecutionRequest {
    execute_requested: bool,
    channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_commands: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_response_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentSessionTranscript {
    session_id: String,
    kind: String,
    project_slug: String,
    #[serde(default)]
    token_used: u64,
    token_budget: Option<u64>,
    command: Value,
    execution_request: Option<AgentSessionExecutionRequest>,
}

impl<'a> AgentSessionRunner<'a> {
    pub fn new(repository: &'a RuntimeRepository) -> Self {
        Self { repository }
    }

    pub fn stage_hermes_command(
        &self,
        command: HermesCommand,
        control_dir: impl AsRef<Path>,
    ) -> Result<AgentSessionRunReport> {
        let token_used = estimate_tokens(&[
            command.endpoint.as_str(),
            command.project_slug.as_str(),
            command.instruction.as_str(),
        ]) + (command.allowed_tools.len() as u64 * 32);
        let session =
            AgentSession::new(command.project_slug.clone(), command.allowed_tools.clone())
                .with_token_used(token_used);
        let status = if command.requires_confirmation {
            TaskStatus::WaitingApproval
        } else {
            TaskStatus::Ready
        };
        let transcript_path = transcript_path(
            control_dir.as_ref(),
            &command.project_slug,
            "hermes",
            &session.id,
        );
        write_transcript(
            &transcript_path,
            &json!({
                "session_id": session.id,
                "kind": "hermes",
                "project_slug": command.project_slug,
                "status": status,
                "token_used": token_used,
                "command": command,
            }),
        )?;
        let session = session.with_transcript_path(transcript_path.to_string_lossy().to_string());
        let mut task = RuntimeTask::new(session.project_slug.clone(), "Hermes embedded control");
        task.provider_id = Some("hermes".to_string());
        task.status = status.clone();
        task.cost_estimate_tokens = token_used;
        task.requires_approval = status == TaskStatus::WaitingApproval;
        task.request_metadata_path = session.transcript_path.clone();

        self.repository.insert_task(&task)?;
        self.repository.insert_agent_session(&session)?;
        self.repository.insert_event(&RuntimeEvent::new(
            session.project_slug.clone(),
            if status == TaskStatus::WaitingApproval {
                RuntimeEventLevel::Warn
            } else {
                RuntimeEventLevel::Info
            },
            format!(
                "Hermes command staged: session={} status={:?}",
                session.id, status
            ),
        ))?;

        Ok(AgentSessionRunReport {
            session_id: session.id,
            task_id: task.id,
            project_slug: session.project_slug,
            kind: AgentSessionKind::Hermes,
            status,
            transcript_path: transcript_path.to_string_lossy().to_string(),
            token_budget: session.token_budget,
            token_used,
            execution: None,
        })
    }

    pub fn stage_cli_command(
        &self,
        project_slug: impl Into<String>,
        command: AgentCliCommand,
        control_dir: impl AsRef<Path>,
    ) -> Result<AgentSessionRunReport> {
        let project_slug = project_slug.into();
        let token_used = estimate_tokens(&[
            project_slug.as_str(),
            command.title.as_str(),
            command.command.as_str(),
        ]) + (command.tools.len() as u64 * 32);
        let session = AgentSession::new(project_slug.clone(), command.tools.clone())
            .with_token_budget(command.token_budget)
            .with_token_used(token_used);
        let status = if let Some(budget) = session.token_budget {
            if token_used > budget {
                TaskStatus::WaitingApproval
            } else {
                TaskStatus::Ready
            }
        } else {
            TaskStatus::Ready
        };
        let transcript_path = transcript_path(
            control_dir.as_ref(),
            &project_slug,
            "agent-cli",
            &session.id,
        );
        write_transcript(
            &transcript_path,
            &json!({
                "session_id": session.id,
                "kind": "agent_cli",
                "project_slug": project_slug,
                "status": status,
                "token_used": token_used,
                "token_budget": session.token_budget,
                "command": command,
            }),
        )?;
        let session = session.with_transcript_path(transcript_path.to_string_lossy().to_string());
        let mut task = RuntimeTask::new(
            project_slug.clone(),
            format!("Agent CLI: {}", command.title),
        );
        task.provider_id = Some("agent-cli".to_string());
        task.status = status.clone();
        task.cost_estimate_tokens = token_used;
        task.requires_approval = status == TaskStatus::WaitingApproval;
        task.request_metadata_path = session.transcript_path.clone();

        self.repository.insert_task(&task)?;
        self.repository.insert_agent_session(&session)?;
        self.repository.insert_event(&RuntimeEvent::new(
            session.project_slug.clone(),
            if status == TaskStatus::WaitingApproval {
                RuntimeEventLevel::Warn
            } else {
                RuntimeEventLevel::Info
            },
            format!(
                "Agent CLI command staged: session={} status={:?}",
                session.id, status
            ),
        ))?;

        Ok(AgentSessionRunReport {
            session_id: session.id,
            task_id: task.id,
            project_slug: session.project_slug,
            kind: AgentSessionKind::AgentCli,
            status,
            transcript_path: transcript_path.to_string_lossy().to_string(),
            token_budget: session.token_budget,
            token_used,
            execution: None,
        })
    }

    pub fn run_hermes_command(
        &self,
        command: HermesCommand,
        control_dir: impl AsRef<Path>,
        options: HermesExecutionOptions,
    ) -> Result<AgentSessionRunReport> {
        let mut report = self.stage_hermes_command(command.clone(), control_dir)?;
        if report.status == TaskStatus::WaitingApproval {
            write_transcript(
                Path::new(&report.transcript_path),
                &json!({
                    "session_id": report.session_id,
                    "kind": "hermes",
                    "project_slug": report.project_slug,
                    "status": report.status,
                    "token_used": report.token_used,
                    "command": command,
                    "execution_request": hermes_execution_request(&options),
                }),
            )?;
            return Ok(report);
        }

        self.repository
            .update_task_status(&report.task_id, TaskStatus::Running)?;
        self.repository.insert_event(&RuntimeEvent::new(
            command.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!(
                "Hermes HTTP execution started: session={}",
                report.session_id
            ),
        ))?;

        let execution = execute_hermes_http(&command, &options);
        let status = if execution.ok {
            TaskStatus::Succeeded
        } else {
            TaskStatus::Failed
        };
        self.repository
            .update_task_status(&report.task_id, status.clone())?;
        self.repository.insert_event(&RuntimeEvent::new(
            command.project_slug.clone(),
            if status == TaskStatus::Succeeded {
                RuntimeEventLevel::Ok
            } else {
                RuntimeEventLevel::Error
            },
            format!(
                "Hermes HTTP execution finished: session={} status={:?}",
                report.session_id, status
            ),
        ))?;

        write_transcript(
            Path::new(&report.transcript_path),
            &json!({
                "session_id": report.session_id,
                "kind": "hermes",
                "project_slug": report.project_slug,
                "status": status,
                "token_used": report.token_used,
                "command": command,
                "execution": execution,
            }),
        )?;

        report.status = status;
        report.execution = Some(execution);
        Ok(report)
    }

    pub fn run_cli_command(
        &self,
        project_slug: impl Into<String>,
        command: AgentCliCommand,
        control_dir: impl AsRef<Path>,
        options: AgentCliExecutionOptions,
    ) -> Result<AgentSessionRunReport> {
        let project_slug = project_slug.into();
        let mut report =
            self.stage_cli_command(project_slug.clone(), command.clone(), control_dir)?;
        if report.status == TaskStatus::WaitingApproval {
            write_transcript(
                Path::new(&report.transcript_path),
                &json!({
                    "session_id": report.session_id,
                    "kind": "agent_cli",
                    "project_slug": report.project_slug,
                    "status": report.status,
                    "token_used": report.token_used,
                    "token_budget": report.token_budget,
                    "command": command,
                    "execution_request": agent_cli_execution_request(&options),
                }),
            )?;
            return Ok(report);
        }

        self.repository
            .update_task_status(&report.task_id, TaskStatus::Running)?;
        self.repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("Agent CLI execution started: session={}", report.session_id),
        ))?;

        let execution = execute_agent_cli(&command.command, &options);
        let status =
            if execution.allowed && execution.error.is_none() && execution.exit_code == Some(0) {
                TaskStatus::Succeeded
            } else {
                TaskStatus::Failed
            };
        self.repository
            .update_task_status(&report.task_id, status.clone())?;
        self.repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            if status == TaskStatus::Succeeded {
                RuntimeEventLevel::Ok
            } else {
                RuntimeEventLevel::Error
            },
            format!(
                "Agent CLI execution finished: session={} status={:?}",
                report.session_id, status
            ),
        ))?;

        write_transcript(
            Path::new(&report.transcript_path),
            &json!({
                "session_id": report.session_id,
                "kind": "agent_cli",
                "project_slug": report.project_slug,
                "status": status,
                "token_used": report.token_used,
                "token_budget": report.token_budget,
                "command": command,
                "execution": execution,
            }),
        )?;

        report.status = status;
        report.execution = Some(execution);
        Ok(report)
    }

    pub fn resume_transcript_execution(
        &self,
        task_id: &str,
        transcript_path: impl AsRef<Path>,
        hermes_options: HermesExecutionOptions,
        resume_reason: &str,
    ) -> Result<Option<AgentSessionRunReport>> {
        let transcript_path = transcript_path.as_ref();
        let transcript: AgentSessionTranscript = serde_json::from_str(
            &fs::read_to_string(transcript_path)
                .with_context(|| format!("read agent transcript {}", transcript_path.display()))?,
        )
        .with_context(|| format!("parse agent transcript {}", transcript_path.display()))?;
        let Some(execution_request) = transcript.execution_request else {
            return Ok(None);
        };
        if !execution_request.execute_requested {
            return Ok(None);
        }

        match transcript.kind.as_str() {
            "hermes" => {
                let mut command: HermesCommand =
                    serde_json::from_value(transcript.command).context("parse Hermes command")?;
                command.requires_confirmation = false;
                let options = HermesExecutionOptions {
                    auth_token: hermes_options.auth_token,
                    max_response_bytes: execution_request
                        .max_response_bytes
                        .unwrap_or(hermes_options.max_response_bytes),
                    timeout_ms: execution_request
                        .timeout_ms
                        .unwrap_or(hermes_options.timeout_ms),
                };
                self.resume_hermes_command(
                    task_id,
                    transcript.session_id,
                    command,
                    transcript_path,
                    transcript.token_used,
                    options,
                    resume_reason,
                )
                .map(Some)
            }
            "agent_cli" => {
                let command: AgentCliCommand = serde_json::from_value(transcript.command)
                    .context("parse Agent CLI command")?;
                let defaults = AgentCliExecutionOptions::default();
                let options = AgentCliExecutionOptions {
                    allowed_commands: execution_request
                        .allowed_commands
                        .unwrap_or_else(|| defaults.allowed_commands.clone()),
                    working_dir: execution_request
                        .working_dir
                        .map(PathBuf::from)
                        .or_else(|| defaults.working_dir.clone()),
                    max_output_bytes: execution_request
                        .max_output_bytes
                        .unwrap_or(defaults.max_output_bytes),
                    timeout_ms: execution_request.timeout_ms.unwrap_or(defaults.timeout_ms),
                };
                self.resume_cli_command(
                    task_id,
                    transcript.session_id,
                    transcript.project_slug,
                    command,
                    transcript_path,
                    transcript.token_used,
                    transcript.token_budget,
                    options,
                    resume_reason,
                )
                .map(Some)
            }
            _ => Ok(None),
        }
    }

    fn resume_hermes_command(
        &self,
        task_id: &str,
        session_id: String,
        command: HermesCommand,
        transcript_path: &Path,
        token_used: u64,
        options: HermesExecutionOptions,
        resume_reason: &str,
    ) -> Result<AgentSessionRunReport> {
        self.repository
            .update_task_status(task_id, TaskStatus::Running)?;
        self.repository.insert_event(&RuntimeEvent::new(
            command.project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("Hermes HTTP execution resumed: session={session_id}"),
        ))?;
        write_transcript(
            transcript_path,
            &json!({
                "session_id": session_id,
                "kind": "hermes",
                "project_slug": command.project_slug,
                "status": TaskStatus::Running,
                "token_used": token_used,
                "command": command,
                "execution_request": hermes_execution_request(&options),
                "resume_reason": resume_reason,
            }),
        )?;

        let execution = execute_hermes_http(&command, &options);
        let status = if execution.ok {
            TaskStatus::Succeeded
        } else {
            TaskStatus::Failed
        };
        self.repository
            .update_task_status(task_id, status.clone())?;
        self.repository.insert_event(&RuntimeEvent::new(
            command.project_slug.clone(),
            if status == TaskStatus::Succeeded {
                RuntimeEventLevel::Ok
            } else {
                RuntimeEventLevel::Error
            },
            format!("Hermes HTTP execution finished: session={session_id} status={status:?}"),
        ))?;
        write_transcript(
            transcript_path,
            &json!({
                "session_id": session_id,
                "kind": "hermes",
                "project_slug": command.project_slug,
                "status": status,
                "token_used": token_used,
                "command": command,
                "execution_request": hermes_execution_request(&options),
                "resume_reason": resume_reason,
                "execution": execution,
            }),
        )?;

        Ok(AgentSessionRunReport {
            session_id,
            task_id: task_id.to_string(),
            project_slug: command.project_slug,
            kind: AgentSessionKind::Hermes,
            status,
            transcript_path: transcript_path.to_string_lossy().to_string(),
            token_budget: None,
            token_used,
            execution: Some(execution),
        })
    }

    fn resume_cli_command(
        &self,
        task_id: &str,
        session_id: String,
        project_slug: String,
        command: AgentCliCommand,
        transcript_path: &Path,
        token_used: u64,
        token_budget: Option<u64>,
        options: AgentCliExecutionOptions,
        resume_reason: &str,
    ) -> Result<AgentSessionRunReport> {
        self.repository
            .update_task_status(task_id, TaskStatus::Running)?;
        self.repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            RuntimeEventLevel::Info,
            format!("Agent CLI execution resumed: session={session_id}"),
        ))?;
        write_transcript(
            transcript_path,
            &json!({
                "session_id": session_id,
                "kind": "agent_cli",
                "project_slug": project_slug,
                "status": TaskStatus::Running,
                "token_used": token_used,
                "token_budget": token_budget,
                "command": command,
                "execution_request": agent_cli_execution_request(&options),
                "resume_reason": resume_reason,
            }),
        )?;

        let execution = execute_agent_cli(&command.command, &options);
        let status =
            if execution.allowed && execution.error.is_none() && execution.exit_code == Some(0) {
                TaskStatus::Succeeded
            } else {
                TaskStatus::Failed
            };
        self.repository
            .update_task_status(task_id, status.clone())?;
        self.repository.insert_event(&RuntimeEvent::new(
            project_slug.clone(),
            if status == TaskStatus::Succeeded {
                RuntimeEventLevel::Ok
            } else {
                RuntimeEventLevel::Error
            },
            format!("Agent CLI execution finished: session={session_id} status={status:?}"),
        ))?;
        write_transcript(
            transcript_path,
            &json!({
                "session_id": session_id,
                "kind": "agent_cli",
                "project_slug": project_slug,
                "status": status,
                "token_used": token_used,
                "token_budget": token_budget,
                "command": command,
                "execution_request": agent_cli_execution_request(&options),
                "resume_reason": resume_reason,
                "execution": execution,
            }),
        )?;

        Ok(AgentSessionRunReport {
            session_id,
            task_id: task_id.to_string(),
            project_slug,
            kind: AgentSessionKind::AgentCli,
            status,
            transcript_path: transcript_path.to_string_lossy().to_string(),
            token_budget,
            token_used,
            execution: Some(execution),
        })
    }
}

fn transcript_path(root: &Path, project_slug: &str, kind: &str, session_id: &str) -> PathBuf {
    root.join(project_slug)
        .join(format!("{kind}-{session_id}-transcript.json"))
}

fn write_transcript(path: &Path, payload: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create agent transcript dir {}", parent.display()))?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(payload).context("serialize agent transcript")?,
    )
    .with_context(|| format!("write agent transcript {}", path.display()))
}

fn hermes_execution_request(options: &HermesExecutionOptions) -> AgentSessionExecutionRequest {
    AgentSessionExecutionRequest {
        execute_requested: true,
        channel: "hermes_http".to_string(),
        allowed_commands: None,
        working_dir: None,
        max_output_bytes: None,
        max_response_bytes: Some(options.max_response_bytes),
        timeout_ms: Some(options.timeout_ms),
    }
}

fn agent_cli_execution_request(options: &AgentCliExecutionOptions) -> AgentSessionExecutionRequest {
    AgentSessionExecutionRequest {
        execute_requested: true,
        channel: "agent_cli".to_string(),
        allowed_commands: Some(options.allowed_commands.clone()),
        working_dir: working_dir_text(options),
        max_output_bytes: Some(options.max_output_bytes),
        max_response_bytes: None,
        timeout_ms: Some(options.timeout_ms),
    }
}

fn estimate_tokens(parts: &[&str]) -> u64 {
    let chars: usize = parts.iter().map(|part| part.chars().count()).sum();
    ((chars as u64) / 4).max(1)
}

fn execute_hermes_http(
    command: &HermesCommand,
    options: &HermesExecutionOptions,
) -> AgentSessionExecutionReport {
    let endpoint = command.endpoint.clone();
    if endpoint.trim().is_empty() {
        return AgentSessionExecutionReport {
            channel: AgentSessionExecutionChannel::HermesHttp,
            attempted: false,
            allowed: false,
            ok: false,
            argv: Vec::new(),
            endpoint: Some(endpoint),
            working_dir: None,
            exit_code: None,
            status_code: None,
            stdout: String::new(),
            stderr: String::new(),
            response_body: None,
            error: Some("Hermes endpoint is empty".to_string()),
        };
    }

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return AgentSessionExecutionReport {
                channel: AgentSessionExecutionChannel::HermesHttp,
                attempted: false,
                allowed: true,
                ok: false,
                argv: Vec::new(),
                endpoint: Some(endpoint),
                working_dir: None,
                exit_code: None,
                status_code: None,
                stdout: String::new(),
                stderr: String::new(),
                response_body: None,
                error: Some(format!("failed to create Hermes HTTP runtime: {error}")),
            };
        }
    };

    runtime.block_on(async {
        let client = match Client::builder()
            .timeout(Duration::from_millis(options.timeout_ms.max(1)))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                return AgentSessionExecutionReport {
                    channel: AgentSessionExecutionChannel::HermesHttp,
                    attempted: false,
                    allowed: true,
                    ok: false,
                    argv: Vec::new(),
                    endpoint: Some(endpoint),
                    working_dir: None,
                    exit_code: None,
                    status_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    response_body: None,
                    error: Some(format!("failed to create Hermes HTTP client: {error}")),
                };
            }
        };
        let mut request = client.post(&command.endpoint).json(&json!({
            "project_slug": command.project_slug,
            "instruction": command.instruction,
            "allowed_tools": command.allowed_tools,
            "requires_confirmation": command.requires_confirmation,
        }));
        if let Some(token) = options
            .auth_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
        {
            request = request.bearer_auth(token);
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status();
                match response.text().await {
                    Ok(body) => AgentSessionExecutionReport {
                        channel: AgentSessionExecutionChannel::HermesHttp,
                        attempted: true,
                        allowed: true,
                        ok: status.is_success(),
                        argv: Vec::new(),
                        endpoint: Some(command.endpoint.clone()),
                        working_dir: None,
                        exit_code: None,
                        status_code: Some(status.as_u16()),
                        stdout: String::new(),
                        stderr: String::new(),
                        response_body: Some(truncate_text(&body, options.max_response_bytes)),
                        error: if status.is_success() {
                            None
                        } else {
                            Some(format!("Hermes HTTP returned status {}", status.as_u16()))
                        },
                    },
                    Err(error) => AgentSessionExecutionReport {
                        channel: AgentSessionExecutionChannel::HermesHttp,
                        attempted: true,
                        allowed: true,
                        ok: false,
                        argv: Vec::new(),
                        endpoint: Some(command.endpoint.clone()),
                        working_dir: None,
                        exit_code: None,
                        status_code: Some(status.as_u16()),
                        stdout: String::new(),
                        stderr: String::new(),
                        response_body: None,
                        error: Some(format!("failed to read Hermes HTTP response: {error}")),
                    },
                }
            }
            Err(error) => AgentSessionExecutionReport {
                channel: AgentSessionExecutionChannel::HermesHttp,
                attempted: true,
                allowed: true,
                ok: false,
                argv: Vec::new(),
                endpoint: Some(command.endpoint.clone()),
                working_dir: None,
                exit_code: None,
                status_code: error.status().map(|status| status.as_u16()),
                stdout: String::new(),
                stderr: String::new(),
                response_body: None,
                error: Some(format!("Hermes HTTP request failed: {error:?}")),
            },
        }
    })
}

fn execute_agent_cli(
    command: &str,
    options: &AgentCliExecutionOptions,
) -> AgentSessionExecutionReport {
    let argv = match parse_command_line(command) {
        Ok(argv) => argv,
        Err(error) => {
            return AgentSessionExecutionReport {
                channel: AgentSessionExecutionChannel::AgentCli,
                attempted: false,
                allowed: false,
                ok: false,
                argv: Vec::new(),
                endpoint: None,
                working_dir: working_dir_text(options),
                exit_code: None,
                status_code: None,
                stdout: String::new(),
                stderr: String::new(),
                response_body: None,
                error: Some(error),
            };
        }
    };
    if argv.is_empty() {
        return AgentSessionExecutionReport {
            channel: AgentSessionExecutionChannel::AgentCli,
            attempted: false,
            allowed: false,
            ok: false,
            argv,
            endpoint: None,
            working_dir: working_dir_text(options),
            exit_code: None,
            status_code: None,
            stdout: String::new(),
            stderr: String::new(),
            response_body: None,
            error: Some("Agent CLI command is empty".to_string()),
        };
    }
    if !command_allowed(&argv[0], &options.allowed_commands) {
        let name = command_name(&argv[0]);
        return AgentSessionExecutionReport {
            channel: AgentSessionExecutionChannel::AgentCli,
            attempted: false,
            allowed: false,
            ok: false,
            argv,
            endpoint: None,
            working_dir: working_dir_text(options),
            exit_code: None,
            status_code: None,
            stdout: String::new(),
            stderr: String::new(),
            response_body: None,
            error: Some(format!("command is not in Agent CLI allowlist: {name}")),
        };
    }

    let mut process = Command::new(&argv[0]);
    process.args(&argv[1..]);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());
    if let Some(working_dir) = &options.working_dir {
        process.current_dir(working_dir);
    }

    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            return AgentSessionExecutionReport {
                channel: AgentSessionExecutionChannel::AgentCli,
                attempted: true,
                allowed: true,
                ok: false,
                argv,
                endpoint: None,
                working_dir: working_dir_text(options),
                exit_code: None,
                status_code: None,
                stdout: String::new(),
                stderr: String::new(),
                response_body: None,
                error: Some(format!("failed to spawn Agent CLI command: {error}")),
            };
        }
    };
    let started = Instant::now();
    let timeout = Duration::from_millis(options.timeout_ms.max(1));
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return match child.wait_with_output() {
                    Ok(output) => AgentSessionExecutionReport {
                        channel: AgentSessionExecutionChannel::AgentCli,
                        attempted: true,
                        allowed: true,
                        ok: output.status.success(),
                        argv,
                        endpoint: None,
                        working_dir: working_dir_text(options),
                        exit_code: output.status.code(),
                        status_code: None,
                        stdout: truncate_utf8(&output.stdout, options.max_output_bytes),
                        stderr: truncate_utf8(&output.stderr, options.max_output_bytes),
                        response_body: None,
                        error: None,
                    },
                    Err(error) => AgentSessionExecutionReport {
                        channel: AgentSessionExecutionChannel::AgentCli,
                        attempted: true,
                        allowed: true,
                        ok: false,
                        argv,
                        endpoint: None,
                        working_dir: working_dir_text(options),
                        exit_code: None,
                        status_code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        response_body: None,
                        error: Some(format!("failed to collect Agent CLI output: {error}")),
                    },
                };
            }
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let output = child.wait_with_output();
                    return match output {
                        Ok(output) => AgentSessionExecutionReport {
                            channel: AgentSessionExecutionChannel::AgentCli,
                            attempted: true,
                            allowed: true,
                            ok: false,
                            argv,
                            endpoint: None,
                            working_dir: working_dir_text(options),
                            exit_code: output.status.code(),
                            status_code: None,
                            stdout: truncate_utf8(&output.stdout, options.max_output_bytes),
                            stderr: truncate_utf8(&output.stderr, options.max_output_bytes),
                            response_body: None,
                            error: Some(format!(
                                "Agent CLI command timed out after {} ms",
                                options.timeout_ms
                            )),
                        },
                        Err(error) => AgentSessionExecutionReport {
                            channel: AgentSessionExecutionChannel::AgentCli,
                            attempted: true,
                            allowed: true,
                            ok: false,
                            argv,
                            endpoint: None,
                            working_dir: working_dir_text(options),
                            exit_code: None,
                            status_code: None,
                            stdout: String::new(),
                            stderr: String::new(),
                            response_body: None,
                            error: Some(format!(
                                "Agent CLI command timed out and output collection failed: {error}"
                            )),
                        },
                    };
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                let _ = child.kill();
                return AgentSessionExecutionReport {
                    channel: AgentSessionExecutionChannel::AgentCli,
                    attempted: true,
                    allowed: true,
                    ok: false,
                    argv,
                    endpoint: None,
                    working_dir: working_dir_text(options),
                    exit_code: None,
                    status_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    response_body: None,
                    error: Some(format!("failed to wait for Agent CLI command: {error}")),
                };
            }
        }
    }
}

fn parse_command_line(command: &str) -> std::result::Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (None, '"') | (None, '\'') => quote = Some(ch),
            (Some(active), ch) if ch == active => quote = None,
            (None, ch) if ch.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            (_, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push('\\');
                }
            }
            (_, ch) => current.push(ch),
        }
    }
    if let Some(active) = quote {
        return Err(format!("unterminated quote in Agent CLI command: {active}"));
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
}

fn command_allowed(binary: &str, allowed_commands: &[String]) -> bool {
    let name = command_name(binary);
    allowed_commands
        .iter()
        .any(|allowed| allowed == binary || allowed == &name)
}

fn command_name(binary: &str) -> String {
    Path::new(binary)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(binary)
        .to_string()
}

fn truncate_utf8(bytes: &[u8], max_bytes: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    truncate_text(&text, max_bytes)
}

fn truncate_text(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &text[..end])
}

fn working_dir_text(options: &AgentCliExecutionOptions) -> Option<String> {
    options
        .working_dir
        .as_ref()
        .map(|path| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RuntimeRepository;

    #[test]
    fn stages_hermes_command_and_records_session() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let temp = tempfile_dir("hermes");
        let runner = AgentSessionRunner::new(&repository);

        let report = runner
            .stage_hermes_command(
                HermesCommand {
                    endpoint: "http://127.0.0.1:3900".to_string(),
                    project_slug: "demo".to_string(),
                    instruction: "inspect Unreal scene state".to_string(),
                    allowed_tools: vec!["mcp".to_string(), "git".to_string()],
                    requires_confirmation: false,
                },
                &temp,
            )
            .unwrap();

        assert_eq!(report.status, TaskStatus::Ready);
        assert!(Path::new(&report.transcript_path).exists());
        assert_eq!(repository.table_count("agent_sessions").unwrap(), 1);
        assert_eq!(repository.table_count("tasks").unwrap(), 1);
        assert_eq!(repository.table_count("workflow_events").unwrap(), 1);
    }

    #[test]
    fn stages_hermes_command_waiting_for_confirmation() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let temp = tempfile_dir("hermes-confirm");
        let runner = AgentSessionRunner::new(&repository);

        let report = runner
            .stage_hermes_command(
                HermesCommand {
                    endpoint: "http://127.0.0.1:3900".to_string(),
                    project_slug: "demo".to_string(),
                    instruction: "execute expensive scene build".to_string(),
                    allowed_tools: vec!["unreal".to_string()],
                    requires_confirmation: true,
                },
                &temp,
            )
            .unwrap();

        assert_eq!(report.status, TaskStatus::WaitingApproval);
        assert_eq!(repository.table_count("agent_sessions").unwrap(), 1);
    }

    #[test]
    fn stages_agent_cli_command_and_tracks_budget() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let temp = tempfile_dir("agent-cli");
        let runner = AgentSessionRunner::new(&repository);

        let report = runner
            .stage_cli_command(
                "demo",
                AgentCliCommand {
                    id: "node-context".to_string(),
                    title: "Inspect project nodes".to_string(),
                    command: "pool-cli --project demo node-context".to_string(),
                    tools: vec!["git".to_string(), "sqlite".to_string()],
                    token_budget: Some(2_000),
                },
                &temp,
            )
            .unwrap();

        assert_eq!(report.status, TaskStatus::Ready);
        assert_eq!(report.token_budget, Some(2_000));
        assert!(report.token_used > 0);
        assert!(Path::new(&report.transcript_path).exists());
        assert_eq!(repository.table_count("agent_sessions").unwrap(), 1);
    }

    #[test]
    fn cli_command_over_budget_waits_for_approval() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let temp = tempfile_dir("agent-cli-budget");
        let runner = AgentSessionRunner::new(&repository);

        let report = runner
            .stage_cli_command(
                "demo",
                AgentCliCommand {
                    id: "large".to_string(),
                    title: "Large reasoning task".to_string(),
                    command: "pool-cli --project demo snapshot".to_string(),
                    tools: vec!["git".to_string(), "sqlite".to_string(), "mcp".to_string()],
                    token_budget: Some(1),
                },
                &temp,
            )
            .unwrap();

        assert_eq!(report.status, TaskStatus::WaitingApproval);
    }

    #[test]
    fn run_cli_command_executes_allowed_binary() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let temp = tempfile_dir("agent-cli-exec");
        let runner = AgentSessionRunner::new(&repository);

        let report = runner
            .run_cli_command(
                "demo",
                AgentCliCommand {
                    id: "echo".to_string(),
                    title: "Echo smoke".to_string(),
                    command: "/bin/echo pool-agent-ok".to_string(),
                    tools: vec!["cli".to_string()],
                    token_budget: Some(2_000),
                },
                &temp,
                AgentCliExecutionOptions {
                    allowed_commands: vec!["/bin/echo".to_string(), "echo".to_string()],
                    working_dir: None,
                    max_output_bytes: 1_024,
                    timeout_ms: 2_000,
                },
            )
            .unwrap();

        let execution = report.execution.unwrap();
        assert_eq!(report.status, TaskStatus::Succeeded);
        assert!(execution.allowed);
        assert!(execution.attempted);
        assert_eq!(execution.exit_code, Some(0));
        assert!(execution.stdout.contains("pool-agent-ok"));
        assert_eq!(
            repository.task_snapshot(&report.task_id).unwrap().status,
            "Succeeded"
        );
        assert!(std::fs::read_to_string(&report.transcript_path)
            .unwrap()
            .contains("\"execution\""));
    }

    #[test]
    fn run_cli_command_denies_unlisted_binary() {
        let repository = RuntimeRepository::in_memory().unwrap();
        repository.migrate().unwrap();
        let temp = tempfile_dir("agent-cli-deny");
        let runner = AgentSessionRunner::new(&repository);

        let report = runner
            .run_cli_command(
                "demo",
                AgentCliCommand {
                    id: "echo".to_string(),
                    title: "Denied echo".to_string(),
                    command: "/bin/echo should-not-run".to_string(),
                    tools: vec!["cli".to_string()],
                    token_budget: Some(2_000),
                },
                &temp,
                AgentCliExecutionOptions {
                    allowed_commands: vec!["pool-cli".to_string()],
                    working_dir: None,
                    max_output_bytes: 1_024,
                    timeout_ms: 2_000,
                },
            )
            .unwrap();

        let execution = report.execution.unwrap();
        assert_eq!(report.status, TaskStatus::Failed);
        assert!(!execution.allowed);
        assert!(!execution.attempted);
        assert!(execution.stdout.is_empty());
        assert!(execution
            .error
            .as_deref()
            .unwrap()
            .contains("not in Agent CLI allowlist"));
    }

    #[test]
    fn parses_quoted_agent_cli_command_arguments() {
        let argv = parse_command_line(r#"pool-cli node-context "hero shot""#).unwrap();

        assert_eq!(argv, vec!["pool-cli", "node-context", "hero shot"]);
        assert!(parse_command_line(r#"pool-cli "unterminated"#).is_err());
    }

    fn tempfile_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pool-agent-session-runner-{name}-{}",
            uuid::Uuid::new_v4()
        ))
    }
}
