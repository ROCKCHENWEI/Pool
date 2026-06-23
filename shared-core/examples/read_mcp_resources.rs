use anyhow::Result;
use pool_core::{build_default_content_burst_plan, McpServer, RuntimeRepository};

fn main() -> Result<()> {
    let repository = RuntimeRepository::in_memory()?;
    repository.migrate()?;
    let plan = build_default_content_burst_plan("demo", "Pool MCP resource smoke");
    repository.persist_plan(&plan)?;
    let snapshot = repository.snapshot(Some("demo"))?;
    let server = McpServer::from_snapshot(snapshot);

    println!("resources={}", server.list_resources().len());
    for uri in [
        "pool://status",
        "pool://tasks",
        "pool://workflow",
        "pool://runtime-graph",
        "pool://software-actions",
        "pool://desktop-recognition",
        "pool://agent-sessions",
        "pool://snapshot",
    ] {
        let payload = server.read_resource(uri)?;
        println!("{uri} bytes={}", payload.len());
    }

    Ok(())
}
