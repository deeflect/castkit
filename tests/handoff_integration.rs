use castkit::cli::{HandoffGetArgs, HandoffInitArgs, HandoffListArgs, HandoffSource};
use castkit::handoff;

#[tokio::test]
async fn handoff_init_list_get_roundtrip() {
    let init = handoff::init_session(HandoffInitArgs {
        target: "echo".to_string(),
        readme: None,
        no_readme: true,
    })
    .await
    .expect("init session");

    assert!(init.session_id.starts_with("sess_"));

    let list = handoff::list_refs(HandoffListArgs {
        session: init.session_id.clone(),
        source: HandoffSource::Help,
        page: 1,
        per_page: 10,
    })
    .expect("list refs");

    assert_eq!(list.source, "help");
    assert!(!list.items.is_empty());

    let first_ref = list.items.first().expect("first ref").ref_id.clone();
    let get = handoff::get_ref(HandoffGetArgs {
        session: init.session_id,
        r#ref: first_ref,
    })
    .expect("get ref");

    assert!(!get.content.is_empty());
}
