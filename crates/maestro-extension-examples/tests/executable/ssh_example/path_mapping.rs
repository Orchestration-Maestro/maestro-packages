//! Local paths under the session directory map onto the remote directory; other paths stay
//! literal.
#![cfg(unix)]
use super::{Ui, call, fixture, loaded, run, start};
use maestro_app::{
    BashExecOptions, BashOperations, BashResult, ExtensionRunner, Local, UserBashEvent,
};
use serde_json::json;
use std::{path::Path, rc::Rc};

/// The user-shell operations the SSH hook supplies for `cwd`.
pub(super) async fn shell(
    runner: &ExtensionRunner<Local>,
    cwd: &Path,
) -> Rc<dyn BashOperations<Local>> {
    let mut event = UserBashEvent {
        command: "pwd".into(),
        cwd: cwd.to_str().unwrap().into(),
        exclude_from_context: false,
    };
    runner
        .emit_user_bash::<Rc<dyn BashOperations<Local>>, BashResult>(&mut event)
        .await
        .unwrap()
        .unwrap()
        .operations
        .unwrap()
}

/// The directory a remote `pwd` reports when the shell is asked to run in `cwd`.
pub(super) async fn remote_pwd(operations: &dyn BashOperations<Local>, cwd: &str) -> String {
    let mut bytes = Vec::new();
    let mut sink = |chunk: &[u8]| bytes.extend_from_slice(chunk);
    let result = operations
        .exec(
            "pwd",
            cwd,
            BashExecOptions {
                on_data: &mut sink,
                signal: None,
                timeout: None,
                env: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.exit_code, Some(0.0));
    String::from_utf8(bytes).unwrap().trim().to_owned()
}

#[test]
fn ssh_maps_session_directory_paths_to_the_remote_directory() {
    if super::isolated("ssh_maps_session_directory_paths_to_the_remote_directory") {
        return;
    }
    run(|executor| async move {
        let root = super::super::support::Root::new();
        let session = loaded(&root, executor).await;
        let runner = session.extension_runner();
        session
            .bind_extensions(maestro_app::ExtensionBindings {
                ui_context: Some(
                    maestro_app::ExtensionUIContext::new(Rc::new(Ui::default())).into(),
                ),
                ..maestro_app::ExtensionBindings::default()
            })
            .await
            .unwrap();
        let local = root.path("project");
        let remote = fixture("remote");
        start(&runner, &format!("user@controlled:{}", remote.display())).await;

        let mapped = local.join("nested/mapped.txt");
        call(
            &runner,
            "write",
            json!({"path": mapped, "content": "mapped"}),
        )
        .await;
        assert_eq!(
            std::fs::read_to_string(remote.join("nested/mapped.txt")).unwrap(),
            "mapped"
        );
        assert!(!mapped.exists());
        assert_eq!(
            call(&runner, "read", json!({"path": mapped})).await[0]["text"],
            "mapped"
        );

        // A directory that only shares the session directory's leading characters is not under it.
        let sibling = root.path("project-other");
        std::fs::create_dir(&sibling).unwrap();
        std::fs::write(sibling.join("note.txt"), "literal").unwrap();
        assert_eq!(
            call(&runner, "read", json!({"path": sibling.join("note.txt")})).await[0]["text"],
            "literal"
        );

        // The session directory itself, and an unnormalised spelling of a directory under it.
        std::fs::create_dir(remote.join("inner")).unwrap();
        let operations = shell(&runner, &local).await;
        assert_eq!(
            remote_pwd(&*operations, local.to_str().unwrap()).await,
            remote.to_str().unwrap()
        );
        let doubled = format!("{}//inner", local.display());
        assert_eq!(
            remote_pwd(&*operations, &doubled).await,
            remote.join("inner").to_str().unwrap()
        );
    });
}
