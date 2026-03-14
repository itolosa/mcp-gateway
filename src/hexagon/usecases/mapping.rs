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
