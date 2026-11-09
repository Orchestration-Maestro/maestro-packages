//! The dependency-backed duration example, discovered as a package by a real SDK session.
use crate::executable::support::{Root, session};
use maestro_app::AgentSession;
use serde_json::{Value, json};
use std::rc::Rc;

/// Execute the actual installed tool callback, outside every session and tool guard.
async fn execute(session: &AgentSession, name: &str, args: Value) -> Value {
    let execute = {
        let state = session.state().read().unwrap();
        let tools = state.tools().read().unwrap();
        let tool = tools
            .iter()
            .find(|tool| tool.read().unwrap().definition.name == name)
            .unwrap();
        tool.read().unwrap().execute.clone()
    };
    serde_json::to_value(execute("sdk-call".into(), args, None, None).await.unwrap()).unwrap()
}

/// A dependency-backed author workload executes after package discovery, not a test-host import.
#[test]
fn sdk_executes_a_packaged_dependency_backed_author_workload() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = Root::new();
        let package = root.path("project/duration-package");
        std::fs::create_dir(&package).unwrap();
        std::fs::copy(std::env::var_os("MAESTRO_WITH_DEPS_COMPONENT").unwrap(), package.join("duration.wasm")).unwrap();
        std::fs::write(package.join("package.json"), r#"{"name":"duration-package","maestro":{"extensions":["duration.wasm"]}}"#).unwrap();
        let session = session::component(&root, local.clone(), package.display().to_string(), None, crate::executable::support::widgets()).await;
        assert_eq!(execute(&session, "parse_duration", json!({"duration":"2 days"})).await["content"][0]["text"], "2 days = 172800000 milliseconds");
        assert_eq!(execute(&session, "parse_duration", json!({"duration":"1h"})).await["content"][0]["text"], "1h = 3600000 milliseconds");
        session.dispose();
    });
}
