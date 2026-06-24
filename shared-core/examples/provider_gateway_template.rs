use anyhow::{bail, Context, Result};
use pool_core::{
    provider_gateway_template_contract, provider_gateway_template_translation,
    sample_provider_gateway_template_request, ProviderGatewayTemplateFamily,
};
use serde_json::Value;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args
        .iter()
        .any(|arg| arg == "--contract" || arg == "contract")
    {
        println!(
            "{}",
            serde_json::to_string_pretty(&provider_gateway_template_contract())?
        );
        return Ok(());
    }

    let family = args
        .first()
        .map(|value| parse_family(value))
        .transpose()?
        .unwrap_or(ProviderGatewayTemplateFamily::AiMedia);
    let provider_id = args.get(1).map(String::as_str).unwrap_or(match family {
        ProviderGatewayTemplateFamily::AiMedia => "nano-banana-pro",
        ProviderGatewayTemplateFamily::ThreeDgs => "worldlabs-marble",
    });
    let request = if let Some(path) = args.get(2).map(PathBuf::from) {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read gateway template request {}", path.display()))?;
        serde_json::from_str::<Value>(&text)
            .with_context(|| format!("decode gateway template request {}", path.display()))?
    } else {
        sample_provider_gateway_template_request(family, provider_id)
    };

    let translation = provider_gateway_template_translation(family, provider_id, &request)?;
    println!("{}", serde_json::to_string_pretty(&translation)?);
    Ok(())
}

fn parse_family(value: &str) -> Result<ProviderGatewayTemplateFamily> {
    match value {
        "ai-media" | "ai_media" | "media" => Ok(ProviderGatewayTemplateFamily::AiMedia),
        "3dgs" | "three-dgs" | "three_dgs" => Ok(ProviderGatewayTemplateFamily::ThreeDgs),
        other => bail!("unknown provider gateway template family: {other}"),
    }
}
