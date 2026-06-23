use anyhow::Result;
use pool_core::{
    build_default_content_burst_plan, materialize_project_envelope, RuntimeHttpConfig,
    RuntimeHttpServer, RuntimeRepository,
};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let db_path = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/runtime-http-smoke/pool-runtime.sqlite"));
    let once = args.iter().any(|arg| arg == "once" || arg == "--once");
    let bind_addr = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--bind="))
        .unwrap_or("127.0.0.1:4788");
    let registry_path = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--registry="))
        .map(PathBuf::from);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let repository = RuntimeRepository::open(&db_path)?;
    repository.migrate()?;
    if repository.stats()?.projects == 0 {
        let plan = build_default_content_burst_plan("demo", "Pool runtime HTTP smoke");
        repository.persist_plan(&plan)?;
        if let Some(parent) = db_path.parent() {
            materialize_project_envelope(parent, &plan)?;
        }
    }
    drop(repository);

    let server = RuntimeHttpServer::new(
        RuntimeHttpConfig::new(&db_path)
            .with_project_slug("demo")
            .with_bind_addr(bind_addr),
    );

    if let Some(registry_path) = registry_path {
        server.write_runtime_registry(&registry_path)?;
        println!(
            "Pool runtime registry written to {}",
            registry_path.display()
        );
    }

    if once {
        for path in [
            "/api/discovery",
            "/api/runtime-registry",
            "/api/health",
            "/api/events/ws",
            "/api/resources",
            "/api/prompts",
            "/api/prompts?name=pool_software_handoff&project_slug=demo&adapter_id=blender&action_kind=ExecuteCli",
            "/api/provider-contracts?provider_id=triposplat",
            "/api/provider-gateway-worker",
            "/api/software-contracts?adapter_id=unreal",
            "/api/desktop-recognition/contract",
            "/api/agent-sessions/ws?session_id=example",
            "/api/mcp?uri=pool%3A%2F%2Ftasks",
            "/api/runtime-graph",
            "/api/runtime-execution-plan",
            "/api/node-context",
            "/api/mcp?uri=pool%3A%2F%2Fruntime-graph",
            "/api/mcp?uri=pool%3A%2F%2Fruntime-execution-plan",
            "/api/mcp?uri=pool%3A%2F%2Fnode-context",
            "/api/mcp?uri=pool%3A%2F%2Fprovider-contracts%2Fmidjourney",
            "/api/mcp?uri=pool%3A%2F%2Fprovider-gateway-worker",
            "/api/mcp?uri=pool%3A%2F%2Fsoftware-contracts%2Funreal",
            "/api/mcp?uri=pool%3A%2F%2Fdesktop-recognition-contract",
            "/api/mcp?uri=pool%3A%2F%2Fdesktop-recognition",
            "/api/api-keys",
            "/api/snapshot",
        ] {
            let response = server.handle_path(path)?;
            println!(
                "{} status={} bytes={}",
                path,
                response.status_code,
                response.body.len()
            );
        }
        let response = server.handle_request_with_body(
            "POST",
            "/api/runtime-execution-plan/run-next",
            r#"{"project_slug":"demo"}"#,
        )?;
        println!(
            "{} status={} bytes={}",
            "/api/runtime-execution-plan/run-next",
            response.status_code,
            response.body.len()
        );
        return Ok(());
    }

    println!("Pool runtime HTTP server listening on http://{}", bind_addr);
    println!("  GET /api/discovery");
    println!("  GET /api/runtime-registry");
    println!("  GET /.well-known/pool-runtime.json");
    println!("  GET /api/health");
    println!("  GET /api/snapshot");
    println!("  GET /api/events/stream");
    println!("  GET /api/events/ws");
    println!("  GET /api/resources");
    println!("  GET /api/prompts");
    println!("  GET /api/prompts?name=pool_software_handoff&adapter_id=blender");
    println!("  GET /api/provider-contracts?provider_id=<provider-id>");
    println!("  GET /api/software-contracts?adapter_id=<adapter-id>");
    println!("  GET /api/desktop-recognition/contract");
    println!("  GET /api/mcp?uri=pool://tasks");
    println!("  GET /api/runtime-graph");
    println!("  GET /api/runtime-execution-plan");
    println!("  POST /api/runtime-execution-plan/run-next");
    println!("  GET /api/node-context?node_id=<node-id>");
    println!("  GET /api/mcp?uri=pool://runtime-graph");
    println!("  GET /api/mcp?uri=pool://runtime-execution-plan");
    println!("  GET /api/mcp?uri=pool://node-context/<node-id>");
    println!("  GET /api/mcp?uri=pool://provider-contracts/<provider-id>");
    println!("  GET /api/mcp?uri=pool://software-contracts/<adapter-id>");
    println!("  GET /api/mcp?uri=pool://desktop-recognition-contract");
    println!("  GET /api/mcp?uri=pool://desktop-recognition");
    println!("  GET /api/api-keys");
    println!("  POST /api/tasks");
    println!("  POST /api/api-keys");
    println!("  POST /api/workflow-runs");
    println!("  POST /api/provider-runs");
    println!("  POST /api/output-packages");
    println!("  POST /api/agent-sessions");
    println!("  GET /api/agent-sessions/ws?session_id=<agent-session-id>");
    println!("  POST /api/tasks/approve?task_id=<task-id>");
    println!("  POST /api/software-actions");
    println!("  GET /api/desktop-recognition/requests");
    println!("  POST /api/desktop-recognition/run-next");
    println!("  POST /api/desktop-recognition/results");
    server.serve_blocking()
}
