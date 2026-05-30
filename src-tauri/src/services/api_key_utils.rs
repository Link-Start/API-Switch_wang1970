pub fn primary_api_key(api_key: &str) -> &str {
    api_key
        .lines()
        .map(str::trim)
        .find(|key| !key.is_empty())
        .unwrap_or_else(|| api_key.trim())
}

#[cfg(test)]
mod tests {
    use super::primary_api_key;

    #[test]
    fn primary_api_key_returns_single_key() {
        assert_eq!(primary_api_key("sk-one"), "sk-one");
    }

    #[test]
    fn primary_api_key_returns_first_non_empty_trimmed_line() {
        assert_eq!(primary_api_key("\n  sk-one  \nsk-two"), "sk-one");
    }

    #[test]
    fn primary_api_key_supports_windows_newlines() {
        assert_eq!(primary_api_key("\r\n  sk-one\r\nsk-two"), "sk-one");
    }

    #[test]
    fn primary_api_key_returns_empty_for_blank_input() {
        assert_eq!(primary_api_key("\n  \n\t"), "");
    }
}
