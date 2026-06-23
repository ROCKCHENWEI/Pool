use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedName {
    pub index: u32,
    pub slug: String,
    pub scope: Option<String>,
    pub extension: String,
    pub hidden: bool,
    pub name: String,
}

pub fn parse_indexed_name(file_name: &str) -> Option<IndexedName> {
    if let Some(stripped) = file_name.strip_prefix('.') {
        let request = stripped.strip_suffix("-request.json")?;
        let (index, rest) = request.split_once('-')?;
        let (slug, scope) = match rest.split_once("__") {
            Some((slug, scope)) => (slug.to_string(), Some(scope.to_string())),
            None => (rest.to_string(), None),
        };
        return Some(IndexedName {
            index: index.parse().ok()?,
            slug,
            scope,
            extension: ".json".to_string(),
            hidden: true,
            name: file_name.to_string(),
        });
    }

    let (index, rest) = file_name.split_once('-')?;
    let dot = rest.rfind('.')?;
    Some(IndexedName {
        index: index.parse().ok()?,
        slug: rest[..dot].to_string(),
        scope: None,
        extension: rest[dot..].to_ascii_lowercase(),
        hidden: false,
        name: file_name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_visible_indexed_assets() {
        let parsed = parse_indexed_name("12-world-full_res.spz").unwrap();
        assert_eq!(parsed.index, 12);
        assert_eq!(parsed.slug, "world-full_res");
        assert_eq!(parsed.extension, ".spz");
        assert!(!parsed.hidden);
    }

    #[test]
    fn parses_hidden_request_metadata() {
        let parsed = parse_indexed_name(".3-chair__model-request.json").unwrap();
        assert_eq!(parsed.index, 3);
        assert_eq!(parsed.slug, "chair");
        assert_eq!(parsed.scope.as_deref(), Some("model"));
        assert!(parsed.hidden);
    }

    #[test]
    fn parses_image_blaster_world_request_metadata() {
        let parsed = parse_indexed_name(".1-world-request.json").unwrap();
        assert_eq!(parsed.index, 1);
        assert_eq!(parsed.slug, "world");
        assert_eq!(parsed.scope, None);
        assert_eq!(parsed.extension, ".json");
        assert!(parsed.hidden);
    }
}
