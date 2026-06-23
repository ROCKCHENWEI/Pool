use anyhow::{Context, Result};
use pool_core::ProviderGatewayMock;
use std::net::TcpListener;

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let bind_addr = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--bind="))
        .unwrap_or("127.0.0.1:8787");
    let max_requests = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--max-requests="))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let once = args.iter().any(|arg| arg == "once" || arg == "--once");

    if once {
        let mut gateway = ProviderGatewayMock::new("http://127.0.0.1:8787");
        for (route, status, bytes) in gateway.self_check()? {
            println!("{route} status={status} bytes={bytes}");
        }
        return Ok(());
    }

    let listener =
        TcpListener::bind(bind_addr).with_context(|| format!("bind mock gateway {bind_addr}"))?;
    let local_addr = listener
        .local_addr()
        .context("read mock gateway local addr")?;
    let mut gateway = ProviderGatewayMock::new(format!("http://{local_addr}"));

    println!("Pool provider gateway mock listening on http://{local_addr}");
    println!("  GET  /health");
    println!("  POST /v1/media/jobs");
    println!("  GET  /v1/media/jobs/<job-id>");
    println!("  POST /v1/3dgs/jobs");
    println!("  POST /v1/3dgs/triposplat/jobs");
    println!("  POST /v1/3dgs/sam-3d/jobs");
    println!("  POST /v1/3dgs/spark/jobs");
    println!("  POST /v1/3dgs/qunhe/jobs");
    println!("  GET  /v1/3dgs/.../jobs/<job-id>");
    println!("  GET  /outputs/<family>/<job-id>/<file>");

    gateway.serve_listener(listener, max_requests)?;
    Ok(())
}
