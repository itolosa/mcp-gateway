use mcp_gateway::hexagon::ports::OperationPolicy;
use mcp_gateway::hexagon::usecases::gateway::create_policy;

// ---------------------------------------------------------------------------
// Allowlist behavior (tested through create_policy with empty denylist)
// ---------------------------------------------------------------------------

#[test]
fn empty_allowlist_allows_all_tools() {
    let policy = create_policy(vec![], vec![]);
    assert!(policy.is_allowed("anything"));
    assert!(policy.is_allowed("another_tool"));
}

#[test]
fn non_empty_allowlist_allows_only_listed_tools() {
    let policy = create_policy(vec!["read".to_string(), "search".to_string()], vec![]);
    assert!(policy.is_allowed("read"));
    assert!(policy.is_allowed("search"));
    assert!(!policy.is_allowed("write"));
    assert!(!policy.is_allowed("delete"));
}

#[test]
fn single_tool_allowlist() {
    let policy = create_policy(vec!["only_this".to_string()], vec![]);
    assert!(policy.is_allowed("only_this"));
    assert!(!policy.is_allowed("not_this"));
}

// ---------------------------------------------------------------------------
// Denylist behavior (tested through create_policy with empty allowlist)
// ---------------------------------------------------------------------------

#[test]
fn empty_denylist_allows_all_tools() {
    let policy = create_policy(vec![], vec![]);
    assert!(policy.is_allowed("anything"));
    assert!(policy.is_allowed("another_tool"));
}

#[test]
fn non_empty_denylist_blocks_listed_tools() {
    let policy = create_policy(vec![], vec!["write".to_string(), "delete".to_string()]);
    assert!(policy.is_allowed("read"));
    assert!(!policy.is_allowed("write"));
    assert!(!policy.is_allowed("delete"));
}

#[test]
fn single_tool_denylist() {
    let policy = create_policy(vec![], vec!["blocked".to_string()]);
    assert!(!policy.is_allowed("blocked"));
    assert!(policy.is_allowed("allowed"));
}

// ---------------------------------------------------------------------------
// Compound policy behavior
// ---------------------------------------------------------------------------

#[test]
fn both_empty_allows_all() {
    let policy = create_policy(vec![], vec![]);
    assert!(policy.is_allowed("anything"));
}

#[test]
fn allowlist_only_filters() {
    let policy = create_policy(vec!["read".to_string()], vec![]);
    assert!(policy.is_allowed("read"));
    assert!(!policy.is_allowed("write"));
}

#[test]
fn denylist_only_filters() {
    let policy = create_policy(vec![], vec!["write".to_string()]);
    assert!(policy.is_allowed("read"));
    assert!(!policy.is_allowed("write"));
}

#[test]
fn denylist_takes_precedence_over_allowlist() {
    let policy = create_policy(
        vec!["read".to_string(), "write".to_string()],
        vec!["write".to_string()],
    );
    assert!(policy.is_allowed("read"));
    assert!(!policy.is_allowed("write"));
}

#[test]
fn denied_tool_blocked_even_if_allowed() {
    let policy = create_policy(vec!["dangerous".to_string()], vec!["dangerous".to_string()]);
    assert!(!policy.is_allowed("dangerous"));
}
