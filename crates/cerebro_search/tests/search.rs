use cerebro_search::{SqliteToolSearch, format_hits};
use cerebro_store::Store;
use cerebro_types::{
    Catalog, Detail, SearchOpts, ServerRecord, ServerStatus, ToolRecord, ToolSearch,
};
use serde_json::json;

fn server(name: &str) -> ServerRecord {
    ServerRecord {
        name: name.into(),
        ts_namespace: name.into(),
        status: ServerStatus::Ok,
        last_error: None,
        protocol_version: None,
        instructions: None,
        catalog_hash: None,
    }
}

fn tool(server: &str, name: &str, ts: &str, desc: &str, ann: serde_json::Value) -> ToolRecord {
    ToolRecord {
        server: server.into(),
        name: name.into(),
        ts_name: ts.into(),
        title: None,
        description: desc.into(),
        input_schema: json!({"type": "object"}),
        output_schema: None,
        annotations: ann,
        ts_signature: Some(format!("{ts}(input: {{ }}): Promise<unknown>")),
        approx_tokens: 10,
        schema_hash: format!("h-{server}-{name}"),
        taint_labels: vec![],
    }
}

fn fixture() -> SqliteToolSearch {
    let store = Store::open_in_memory().unwrap();
    store
        .replace_server_tools(
            &server("github"),
            &[
                tool(
                    "github",
                    "create_issue",
                    "createIssue",
                    "Create an issue in a repository. Needs a title.",
                    json!({}),
                ),
                tool(
                    "github",
                    "list_issues",
                    "listIssues",
                    "List issues of a repository",
                    json!({"readOnlyHint": true}),
                ),
                tool(
                    "github",
                    "delete_repo",
                    "deleteRepo",
                    "Delete a repository",
                    json!({"destructiveHint": true}),
                ),
                tool(
                    "github",
                    "get_pull_request",
                    "getPullRequest",
                    "Fetch a pull request",
                    json!({"readOnlyHint": true}),
                ),
            ],
        )
        .unwrap();
    store
        .replace_server_tools(
            &server("fs"),
            &[
                tool(
                    "fs",
                    "read_file",
                    "readFile",
                    "Read a file from disk",
                    json!({"readOnlyHint": true}),
                ),
                tool(
                    "fs",
                    "list_issues_log",
                    "listIssuesLog",
                    "List the issue log files",
                    json!({"readOnlyHint": true}),
                ),
            ],
        )
        .unwrap();
    SqliteToolSearch::new(store)
}

fn names(hits: &[cerebro_types::ToolHit]) -> Vec<String> {
    hits.iter().map(|h| h.tool.qualified_name()).collect()
}

#[test]
fn create_issue_ranks_before_list_issues() {
    let s = fixture();
    let hits = s.search("create issue", &SearchOpts::default()).unwrap();
    let n = names(&hits);
    assert_eq!(n[0], "github.create_issue", "{n:?}");
    let pos = |x: &str| n.iter().position(|v| v == x);
    assert!(pos("github.create_issue") < pos("github.list_issues"));
}

#[test]
fn camel_case_query_matches_snake_case_tool() {
    let s = fixture();
    let n = names(&s.search("createIssue", &SearchOpts::default()).unwrap());
    assert_eq!(n[0], "github.create_issue");
}

#[test]
fn typo_is_found_via_trigram() {
    let s = fixture();
    let n = names(&s.search("isue", &SearchOpts::default()).unwrap());
    assert!(n.contains(&"github.create_issue".to_string()), "{n:?}");
    // un MATCH literal de "isue" no casaría: la vía es solo trigram
    let n = names(&s.search("repositry", &SearchOpts::default()).unwrap());
    assert!(n.contains(&"github.delete_repo".to_string()), "{n:?}");
}

#[test]
fn server_filter_and_limit() {
    let s = fixture();
    let opts = SearchOpts {
        server: Some("fs".into()),
        ..Default::default()
    };
    let n = names(&s.search("issues", &opts).unwrap());
    assert_eq!(n, ["fs.list_issues_log"]);
    let opts = SearchOpts {
        limit: 1,
        ..Default::default()
    };
    assert_eq!(s.search("issue", &opts).unwrap().len(), 1);
    let opts = SearchOpts {
        limit: 1000,
        ..Default::default()
    };
    assert!(s.search("", &opts).unwrap().len() <= 25);
}

#[test]
fn empty_and_symbol_only_queries_list_by_name() {
    let s = fixture();
    let n = names(
        &s.search(
            "   ",
            &SearchOpts {
                limit: 3,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert_eq!(
        n,
        ["fs.list_issues_log", "fs.read_file", "github.create_issue"]
    );
    assert_eq!(
        names(
            &s.search(
                "\"*(-",
                &SearchOpts {
                    limit: 2,
                    ..Default::default()
                }
            )
            .unwrap()
        )
        .len(),
        2
    );
}

#[test]
fn hostile_syntax_is_neutralised() {
    let s = fixture();
    for q in [
        "issue OR",
        "\"unbalanced",
        "name:create AND NOT",
        "a* NEAR(b c)",
        "'; DROP TABLE tools;--",
    ] {
        s.search(q, &SearchOpts::default()).unwrap();
    }
}

#[test]
fn unavailable_tools_are_excluded() {
    let s = fixture();
    // el hub marcará available=0 en el futuro; se simula directo
    let store = Store::open_in_memory().unwrap();
    store
        .replace_server_tools(
            &server("x"),
            &[tool("x", "a_tool", "aTool", "d", json!({}))],
        )
        .unwrap();
    store
        .conn()
        .execute("UPDATE hub.tools SET available = 0", [])
        .unwrap();
    let s2 = SqliteToolSearch::new(store);
    assert!(
        s2.search("tool", &SearchOpts::default())
            .unwrap()
            .is_empty()
    );
    assert!(
        !s.search("tool OR file", &SearchOpts::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn format_hits_three_levels() {
    let s = fixture();
    let hits = s
        .search(
            "create issue",
            &SearchOpts {
                server: Some("github".into()),
                limit: 3,
                ..Default::default()
            },
        )
        .unwrap();
    let hits: Vec<_> = hits
        .into_iter()
        .filter(|h| matches!(h.tool.name.as_str(), "create_issue" | "list_issues"))
        .collect();
    assert_eq!(
        format_hits(&hits, Detail::Names),
        "github.createIssue\ngithub.listIssues"
    );
    assert_eq!(
        format_hits(&hits, Detail::Summary),
        "github.createIssue — Create an issue in a repository [write]\n\
         github.listIssues — List issues of a repository [read]"
    );
    assert_eq!(
        format_hits(&hits, Detail::Full),
        "github.createIssue — Create an issue in a repository [write]\n  \
         createIssue(input: { }): Promise<unknown>\n\
         github.listIssues — List issues of a repository [read]\n  \
         listIssues(input: { }): Promise<unknown>"
    );
}

#[test]
fn format_hits_marks_and_truncation() {
    let s = fixture();
    let del = s.search("delete repo", &SearchOpts::default()).unwrap();
    assert!(format_hits(&del[..1], Detail::Summary).ends_with("[destructive]"));

    let mut t = tool(
        "a",
        "b",
        "b",
        &"x".repeat(150),
        json!({"readOnlyHint": true, "destructiveHint": true}),
    );
    let out = format_hits(
        &[cerebro_types::ToolHit {
            tool: t.clone(),
            score: 0.0,
        }],
        Detail::Summary,
    );
    assert_eq!(out, format!("a.b — {}… [read]", "x".repeat(100)));
    t.description = String::new();
    t.ts_signature = None;
    assert_eq!(
        format_hits(
            &[cerebro_types::ToolHit {
                tool: t,
                score: 0.0
            }],
            Detail::Full
        ),
        "a.b [read]"
    );
    assert_eq!(format_hits(&[], Detail::Full), "");
}
