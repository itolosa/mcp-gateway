const SEPARATOR: &str = "__";

pub fn encode(server: &str, tool: &str) -> String {
    format!("{server}{SEPARATOR}{tool}")
}

pub fn decode(name: &str) -> Option<(&str, &str)> {
    name.split_once(SEPARATOR)
}

pub fn update_json_field(json: &str, key: &str, value: &str) -> String {
    let Ok(mut obj) = serde_json::from_str::<serde_json::Value>(json) else {
        return json.to_string();
    };
    if let Some(map) = obj.as_object_mut() {
        map.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
    serde_json::to_string(&obj).unwrap_or_else(|_| json.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn prefix_combines_server_and_tool() {
        assert_eq!(encode("fs", "read_file"), "fs__read_file");
    }

    #[test]
    fn prefix_preserves_underscores_in_tool_name() {
        assert_eq!(encode("server", "my_tool_name"), "server__my_tool_name");
    }

    #[test]
    fn prefix_handles_hyphens() {
        assert_eq!(encode("my-server", "list-files"), "my-server__list-files");
    }

    #[test]
    fn split_valid_prefixed_name() {
        let result = decode("fs__read_file");
        assert_eq!(result, Some(("fs", "read_file")));
    }

    #[test]
    fn split_returns_none_for_unprefixed_name() {
        assert_eq!(decode("plain_tool"), None);
    }

    #[test]
    fn split_returns_none_for_single_underscore() {
        assert_eq!(decode("server_tool"), None);
    }

    #[test]
    fn split_handles_multiple_double_underscores() {
        let result = decode("server__tool__extra");
        assert_eq!(result, Some(("server", "tool__extra")));
    }

    #[test]
    fn split_returns_none_for_empty_string() {
        assert_eq!(decode(""), None);
    }

    #[test]
    fn split_handles_separator_at_start() {
        let result = decode("__tool");
        assert_eq!(result, Some(("", "tool")));
    }

    #[test]
    fn split_handles_separator_at_end() {
        let result = decode("server__");
        assert_eq!(result, Some(("server", "")));
    }

    #[test]
    fn roundtrip_prefix_then_split() {
        let prefixed = encode("myserver", "mytool");
        let (server, tool) = decode(&prefixed).unwrap();
        assert_eq!(server, "myserver");
        assert_eq!(tool, "mytool");
    }

    #[test]
    fn update_json_field_updates_existing_field() {
        let json = r#"{"name":"old","description":"test"}"#;
        let result = update_json_field(json, "name", "new");
        assert!(result.contains(r#""name":"new""#));
        assert!(result.contains(r#""description":"test""#));
    }

    #[test]
    fn update_json_field_adds_missing_field() {
        let json = r#"{"description":"test"}"#;
        let result = update_json_field(json, "name", "added");
        assert!(result.contains(r#""name":"added""#));
    }

    #[test]
    fn update_json_field_returns_original_for_invalid_json() {
        let json = "not json";
        let result = update_json_field(json, "name", "value");
        assert_eq!(result, "not json");
    }
}
