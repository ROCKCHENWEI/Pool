use anyhow::{Context, Result};
use pool_core::ProviderSdkWorkerTemplate;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let bind_addr = arg_value(&args, "--bind=").unwrap_or("127.0.0.1:8798");
    let output_root = arg_value(&args, "--output-root=")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/provider-sdk-worker-template"));
    let max_requests = arg_value(&args, "--max-requests=")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let once = args.iter().any(|arg| arg == "once" || arg == "--once");

    fs::create_dir_all(&output_root)
        .with_context(|| format!("create SDK worker output root {}", output_root.display()))?;

    if once {
        let mut worker =
            ProviderSdkWorkerTemplate::new("http://127.0.0.1:8798", output_root.clone());
        for (route, status, bytes) in worker.self_check()? {
            println!("{route} status={status} bytes={bytes}");
        }
        println!("output_root={}", output_root.display());
        return Ok(());
    }

    let listener = TcpListener::bind(bind_addr)
        .with_context(|| format!("bind provider SDK worker template {bind_addr}"))?;
    let local_addr = listener
        .local_addr()
        .context("read provider SDK worker local addr")?;
    let mut worker =
        ProviderSdkWorkerTemplate::new(format!("http://{local_addr}"), output_root.clone());

    println!("Pool provider SDK worker template listening on http://{local_addr}");
    println!("  output_root {}", output_root.display());
    println!("  GET  /health");
    println!("  POST /v1/media/jobs");
    println!("  GET  /v1/media/jobs/<job-id>");
    println!("  POST /v1/3dgs/jobs");
    println!("  POST /v1/3dgs/triposplat/jobs");
    println!("  GET  /v1/3dgs/.../jobs/<job-id>");
    println!("  GET  /outputs/<job-id>/<file>");

    worker.serve_listener(listener, max_requests)?;
    Ok(())
}

fn arg_value<'a>(args: &'a [String], prefix: &str) -> Option<&'a str> {
    args.iter().find_map(|arg| arg.strip_prefix(prefix))
}
