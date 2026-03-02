use std::collections::BTreeSet;

use serde_json::Map;

fn is_valid_placeholder_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn extract_placeholders(args: &[String]) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for arg in args {
        let mut rest = arg.as_str();
        while let Some(start) = rest.find("{{") {
            let after_open = &rest[start + 2..];
            if let Some(end) = after_open.find("}}") {
                let name = &after_open[..end];
                if is_valid_placeholder_name(name) {
                    result.insert(name.to_string());
                }
                rest = &after_open[end + 2..];
            } else {
                break;
            }
        }
    }
    result
}

pub fn render_args(
    template_args: &[String],
    params: &Map<String, serde_json::Value>,
) -> Result<Vec<String>, String> {
    let mut rendered = Vec::with_capacity(template_args.len());
    for arg in template_args {
        let mut result = String::new();
        let mut rest = arg.as_str();
        while let Some(start) = rest.find("{{") {
            result.push_str(&rest[..start]);
            let after_open = &rest[start + 2..];
            if let Some(end) = after_open.find("}}") {
                let name = &after_open[..end];
                if is_valid_placeholder_name(name) {
                    let value = params
                        .get(name)
                        .ok_or_else(|| format!("missing required argument: {name}"))?;
                    match value.as_str() {
                        Some(s) => result.push_str(s),
                        None => result.push_str(&value.to_string()),
                    }
                } else {
                    result.push_str("{{");
                    result.push_str(name);
                    result.push_str("}}");
                }
                rest = &after_open[end + 2..];
            } else {
                result.push_str(&rest[start..]);
                rest = "";
                break;
            }
        }
        result.push_str(rest);
        rendered.push(result);
    }
    Ok(rendered)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    fn params(pairs: &[(&str, &str)]) -> Map<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect()
    }

    // --- extract_placeholders tests ---

    #[test]
    fn extract_no_placeholders() {
        assert!(extract_placeholders(&args(&["ls", "-la"])).is_empty());
    }

    #[test]
    fn extract_single_placeholder() {
        let result = extract_placeholders(&args(&["--repo", "{{repo}}"]));
        assert_eq!(result, BTreeSet::from(["repo".to_string()]));
    }

    #[test]
    fn extract_multiple_placeholders() {
        let result = extract_placeholders(&args(&["{{owner}}/{{repo}}"]));
        assert_eq!(
            result,
            BTreeSet::from(["owner".to_string(), "repo".to_string()])
        );
    }

    #[test]
    fn extract_deduplicates() {
        let result = extract_placeholders(&args(&["{{name}}", "{{name}}"]));
        assert_eq!(result.len(), 1);
        assert!(result.contains("name"));
    }

    #[test]
    fn extract_unclosed_brace_ignored() {
        assert!(extract_placeholders(&args(&["{{unclosed"])).is_empty());
    }

    #[test]
    fn extract_empty_args() {
        assert!(extract_placeholders(&[]).is_empty());
    }

    #[test]
    fn extract_dotted_name_not_placeholder() {
        // Docker-style {{.Names}} is NOT a valid identifier placeholder
        assert!(extract_placeholders(&args(&["{{.Names}}"])).is_empty());
    }

    #[test]
    fn extract_underscore_placeholder() {
        let result = extract_placeholders(&args(&["{{_private}}"]));
        assert_eq!(result, BTreeSet::from(["_private".to_string()]));
    }

    #[test]
    fn extract_single_char_placeholder() {
        let result = extract_placeholders(&args(&["{{x}}"]));
        assert_eq!(result, BTreeSet::from(["x".to_string()]));
    }

    // --- render_args tests ---

    #[test]
    fn render_no_placeholders() {
        let result = render_args(&args(&["ls", "-la"]), &params(&[])).unwrap();
        assert_eq!(result, vec!["ls", "-la"]);
    }

    #[test]
    fn render_single_substitution() {
        let result = render_args(
            &args(&["--repo", "{{repo}}"]),
            &params(&[("repo", "myrepo")]),
        )
        .unwrap();
        assert_eq!(result, vec!["--repo", "myrepo"]);
    }

    #[test]
    fn render_embedded_substitution() {
        let result = render_args(
            &args(&["{{owner}}/{{repo}}"]),
            &params(&[("owner", "me"), ("repo", "proj")]),
        )
        .unwrap();
        assert_eq!(result, vec!["me/proj"]);
    }

    #[test]
    fn render_missing_param_returns_error() {
        let result = render_args(&args(&["{{missing}}"]), &params(&[]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing"));
    }

    #[test]
    fn render_non_string_value_uses_to_string() {
        let mut map = Map::new();
        map.insert("count".to_string(), json!(42));
        let result = render_args(&args(&["--count", "{{count}}"]), &map).unwrap();
        assert_eq!(result, vec!["--count", "42"]);
    }

    #[test]
    fn render_multiple_in_one_arg() {
        let result =
            render_args(&args(&["{{a}}-{{b}}"]), &params(&[("a", "x"), ("b", "y")])).unwrap();
        assert_eq!(result, vec!["x-y"]);
    }

    #[test]
    fn render_unclosed_brace_preserved() {
        let result = render_args(&args(&["{{unclosed"]), &params(&[])).unwrap();
        assert_eq!(result, vec!["{{unclosed"]);
    }

    #[test]
    fn render_non_identifier_preserved() {
        // Docker-style {{.Names}} is preserved literally
        let result = render_args(&args(&["{{.Names}}"]), &params(&[])).unwrap();
        assert_eq!(result, vec!["{{.Names}}"]);
    }

    // --- is_valid_placeholder_name tests ---

    #[test]
    fn valid_placeholder_names() {
        assert!(is_valid_placeholder_name("repo"));
        assert!(is_valid_placeholder_name("_private"));
        assert!(is_valid_placeholder_name("name123"));
        assert!(is_valid_placeholder_name("A"));
    }

    #[test]
    fn invalid_placeholder_names() {
        assert!(!is_valid_placeholder_name(""));
        assert!(!is_valid_placeholder_name(".Names"));
        assert!(!is_valid_placeholder_name("123abc"));
        assert!(!is_valid_placeholder_name("a-b"));
    }
}
