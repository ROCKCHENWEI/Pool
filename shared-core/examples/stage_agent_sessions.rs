use anyhow::Result;
use pool_core::{AgentCliCommand, AgentSessionRunner, HermesCommand, RuntimeRepository};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/agent-session-runner"));
    std::fs::create_dir_all(&output_dir)?;

    let db_path = output_dir.join("pool-runtime.sqlite");
    let control_dir = output_dir.join("worlds/demo/output/control");
    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;

    let runner = AgentSessionRunner::new(&repository);
    let hermes = runner.stage_hermes_command(
        HermesCommand {
            endpoint: "http://127.0.0.1:3900".to_string(),
            project_slug: "demo".to_string(),
            instruction: "inspect current Unreal assembly plan and suggest import order"
                .to_string(),
            allowed_tools: vec![
                "mcp".to_string(),
                "unreal".to_string(),
                "sqlite".to_string(),
            ],
            requires_confirmation: false,
        },
        &control_dir,
    )?;
    let cli = runner.stage_cli_command(
        "demo",
        AgentCliCommand {
            id: "pool-node-context".to_string(),
            title: "Inspect runtime node context".to_string(),
            command: "pool-cli --project demo node-context".to_string(),
            tools: vec!["sqlite".to_string(), "filesystem".to_string()],
            token_budget: Some(4_000),
        },
        &control_dir,
    )?;

    println!("db={}", db_path.display());
    println!("hermes_status={:?}", hermes.status);
    println!("hermes_transcript={}", hermes.transcript_path);
    println!("cli_status={:?}", cli.status);
    println!("cli_transcript={}", cli.transcript_path);
    println!(
        "stats=tasks:{},agent_sessions:{},events:{}",
        repository.table_count("tasks")?,
        repository.table_count("agent_sessions")?,
        repository.table_count("workflow_events")?
    );

    Ok(())
}
