use super::circuit_breaker::CircuitBreaker;
use crate::database::ApiEntry;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageEndpoint {
    Generations,
    Edits,
    Variations,
}

impl ImageEndpoint {
    pub fn path(self) -> &'static str {
        match self {
            Self::Generations => "generations",
            Self::Edits => "edits",
            Self::Variations => "variations",
        }
    }
}

pub fn resolve_images_entries(
    requested_model: &str,
    entries: &[ApiEntry],
    breakers: &HashMap<String, CircuitBreaker>,
) -> Vec<ApiEntry> {
    let now = chrono::Utc::now().timestamp();
    let mut resolved: Vec<ApiEntry> = entries
        .iter()
        .filter(|entry| entry.enabled && entry.model == requested_model)
        .filter(|entry| entry.cooldown_until.is_none_or(|until| until <= now))
        .filter(|entry| {
            breakers
                .get(&entry.id)
                .is_none_or(CircuitBreaker::is_available)
        })
        .cloned()
        .collect();
    resolved.sort_by_key(|entry| entry.sort_index);
    resolved
}

pub fn build_images_url(base_url: &str, endpoint: ImageEndpoint) -> String {
    let mut url = url::Url::parse(base_url).expect("channel base URL must be valid");
    let path = url.path().trim_end_matches('/');
    let mut segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if let Some(index) = segments.iter().position(|segment| *segment == "v1") {
        segments.truncate(index + 1);
    } else {
        segments.push("v1");
    }
    segments.extend(["images", endpoint.path()]);
    url.set_path(&format!("/{}", segments.join("/")));
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, model: &str, enabled: bool, sort_index: i32) -> ApiEntry {
        ApiEntry {
            id: id.to_string(),
            channel_id: format!("channel-{id}"),
            model: model.to_string(),
            display_name: "Image Alias".to_string(),
            sort_index,
            enabled,
            cooldown_until: None,
            circuit_state: "closed".to_string(),
            created_at: 0,
            updated_at: 0,
            channel_name: Some(format!("channel-{id}")),
            channel_api_type: Some("openai".to_string()),
            owned_by: None,
            response_ms: None,
            provider_logo: None,
            release_date: None,
            model_meta_zh: None,
            model_meta_en: None,
            group_name: Some("image-group".to_string()),
            score: 0.0,
        }
    }

    #[test]
    fn images_route_only_exact_enabled_same_model_entries_by_sort_index() {
        let mut cooled = entry("cooled", "image-model", true, 0);
        cooled.cooldown_until = Some(chrono::Utc::now().timestamp() + 60);
        let entries = vec![
            entry("second", "image-model", true, 20),
            entry("disabled", "image-model", false, 1),
            entry("other", "other-image", true, 2),
            entry("first", "image-model", true, 10),
            cooled,
        ];

        let resolved = resolve_images_entries("image-model", &entries, &HashMap::new());
        assert_eq!(
            resolved.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["first", "second"]
        );
    }

    #[test]
    fn images_route_does_not_match_alias_group_auto_or_different_case() {
        let entries = vec![entry("one", "image-model", true, 0)];
        for requested in ["Image Alias", "image-group", "auto", "IMAGE-MODEL", ""] {
            assert!(
                resolve_images_entries(requested, &entries, &HashMap::new()).is_empty(),
                "unexpected match for {requested}"
            );
        }
    }

    #[test]
    fn images_url_supports_all_endpoints_and_common_base_forms() {
        let cases = [
            (
                "https://api.openai.com",
                "https://api.openai.com/v1/images/generations",
            ),
            (
                "https://api.openai.com/",
                "https://api.openai.com/v1/images/generations",
            ),
            (
                "https://example.com/v1",
                "https://example.com/v1/images/generations",
            ),
            (
                "https://example.com/api/v1",
                "https://example.com/api/v1/images/generations",
            ),
            (
                "https://example.com/v1/chat/completions",
                "https://example.com/v1/images/generations",
            ),
            (
                "https://example.com/v1/responses?x=1",
                "https://example.com/v1/images/generations?x=1",
            ),
            (
                "https://example.com/v1/images/edits",
                "https://example.com/v1/images/generations",
            ),
        ];
        for (base, expected) in cases {
            assert_eq!(build_images_url(base, ImageEndpoint::Generations), expected);
        }
        assert_eq!(
            build_images_url("https://api.openai.com/v1", ImageEndpoint::Edits),
            "https://api.openai.com/v1/images/edits"
        );
        assert_eq!(
            build_images_url("https://api.openai.com/v1", ImageEndpoint::Variations),
            "https://api.openai.com/v1/images/variations"
        );
    }
}
