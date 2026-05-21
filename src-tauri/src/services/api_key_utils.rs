pub fn primary_api_key(api_key: &str) -> &str {
    api_key
        .lines()
        .map(str::trim)
        .find(|key| !key.is_empty())
        .unwrap_or_else(|| api_key.trim())
}
