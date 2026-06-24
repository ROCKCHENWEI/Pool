use anyhow::{Context, Result};
use pool_core::{ProviderGatewayWorker, ProviderGatewayWorkerOptions};
use std::net::TcpListener;

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let bind_addr = arg_value(&args, "--bind=").unwrap_or("127.0.0.1:8788");
    let upstream = arg_value(&args, "--upstream=")
        .map(str::to_string)
        .or_else(|| std::env::var("POOL_PROVIDER_GATEWAY_UPSTREAM").ok())
        .unwrap_or_else(|| "http://127.0.0.1:8787".to_string());
    let max_requests = arg_value(&args, "--max-requests=")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let api_key = arg_value(&args, "--api-key=")
        .map(str::to_string)
        .or_else(|| {
            arg_value(&args, "--api-key-env=").and_then(|env_key| std::env::var(env_key).ok())
        });

    let listener = TcpListener::bind(bind_addr)
        .with_context(|| format!("bind provider gateway worker {bind_addr}"))?;
    let local_addr = listener
        .local_addr()
        .context("read provider gateway worker local addr")?;
    let mut options = ProviderGatewayWorkerOptions::new(format!("http://{local_addr}"))
        .with_default_upstream_endpoint(upstream.clone());
    if let Some(api_key) = api_key {
        options = options.with_api_key(api_key);
    }
    let mut worker = ProviderGatewayWorker::new(options);

    println!("Pool provider gateway worker listening on http://{local_addr}");
    println!("  upstream {upstream}");
    println!("  GET  /health");
    println!("  POST /v1/media/jobs");
    println!("  GET  /v1/media/jobs/<job-id>");
    println!("  POST /v1/3dgs/jobs");
    println!("  POST /v1/3dgs/triposplat/jobs");
    println!("  POST /v1/3dgs/sam-3d/jobs");
    println!("  POST /v1/3dgs/spark/jobs");
    println!("  POST /v1/3dgs/qunhe/jobs");
    println!("  GET  /v1/3dgs/.../jobs/<job-id>");

    worker.serve_listener(listener, max_requests)?;
    Ok(())
}

fn arg_value<'a>(args: &'a [String], prefix: &str) -> Option<&'a str> {
    args.iter().find_map(|arg| arg.strip_prefix(prefix))
}
