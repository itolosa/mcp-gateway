const SEPARATOR: &str = "__";

pub fn prefix_tool_name(server: &str, tool: &str) -> String {
    format!("{server}{SEPARATOR}{tool}")
}

pub fn split_prefixed_name(name: &str) -> Option<(&str, &str)> {
    name.split_once(SEPARATOR)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn prefix_combines_server_and_tool() {
        assert_eq!(prefix_tool_name("fs", "read_file"), "fs__read_file");
    }

    #[test]
    fn prefix_preserves_underscores_in_tool_name() {
        assert_eq!(
            prefix_tool_name("server", "my_tool_name"),
            "server__my_tool_name"
        );
    }

    #[test]
    fn prefix_handles_hyphens() {
        assert_eq!(
            prefix_tool_name("my-server", "list-files"),
            "my-server__list-files"
        );
    }

    #[test]
    fn split_valid_prefixed_name() {
        let result = split_prefixed_name("fs__read_file");
        assert_eq!(result, Some(("fs", "read_file")));
    }

    #[test]
    fn split_returns_none_for_unprefixed_name() {
        assert_eq!(split_prefixed_name("plain_tool"), None);
    }

    #[test]
    fn split_returns_none_for_single_underscore() {
        assert_eq!(split_prefixed_name("server_tool"), None);
    }

    #[test]
    fn split_handles_multiple_double_underscores() {
        let result = split_prefixed_name("server__tool__extra");
        assert_eq!(result, Some(("server", "tool__extra")));
    }

    #[test]
    fn split_returns_none_for_empty_string() {
        assert_eq!(split_prefixed_name(""), None);
    }

    #[test]
    fn split_handles_separator_at_start() {
        let result = split_prefixed_name("__tool");
        assert_eq!(result, Some(("", "tool")));
    }

    #[test]
    fn split_handles_separator_at_end() {
        let result = split_prefixed_name("server__");
        assert_eq!(result, Some(("server", "")));
    }

    #[test]
    fn roundtrip_prefix_then_split() {
        let prefixed = prefix_tool_name("myserver", "mytool");
        let (server, tool) = split_prefixed_name(&prefixed).unwrap();
        assert_eq!(server, "myserver");
        assert_eq!(tool, "mytool");
    }
}
