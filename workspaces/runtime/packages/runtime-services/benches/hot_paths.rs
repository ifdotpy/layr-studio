use std::hint::black_box;
use std::sync::Arc;

use gungraun::{Dhat, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main};
use rusqlite::Connection;
use studio_json::json;
use studio_runtime_schema::queue_command::Field;
use studio_runtime_services::change_notifications::ChangeNotificationHub;
use studio_runtime_services::resource_notify::{LiveDelta, ResourceNotifications};
use studio_runtime_services::work_write::{
    CreateWork, SubmitWork, WorkActorWrite, WorkWrite, write, write_as_agent,
};
use studio_runtime_store::{AgentRecord, AgentRepository, WorkItemsRepository};

fn database() -> Connection {
    let db = studio_runtime_store::test_canvas();
    for agent in [
        json!({"compactions":0,"accountKey":"default","provider":"codex","id":"lead","rootId":"lead","epoch":1,"status":"running","autoWake":true,"inFlight":true,"agentMode":"multi","concurrency":8}),
        json!({"compactions":0,"accountKey":"default","provider":"codex","id":"worker","rootId":"lead","epoch":2,"status":"running","autoWake":true,"inFlight":true,"agentMode":"multi","concurrency":8,"role":"reviewer","workspaceMode":"layr-native"}),
    ] {
        AgentRepository::put(&db, &AgentRecord::from_value(agent).expect("agent"))
            .expect("insert agent");
    }
    db
}

#[library_benchmark]
#[bench::assigned(database())]
fn create(mut db: Connection) -> Connection {
    let tx = db.transaction().expect("transaction");
    black_box(
        write_as_agent(
            &tx,
            "lead",
            "lead",
            None,
            "create",
            WorkActorWrite::Create(CreateWork {
                result_schema: None,
                gate: Field::Missing,
                actor_id: "lead".into(),
                title: "Assigned task".into(),
                description: "Do the task".into(),
                owner: Some("worker".into()),
                status: None,
                dependencies: vec![],
            }),
        )
        .expect("create"),
    );
    tx.commit().expect("commit");
    db
}

fn submit_database() -> Connection {
    let mut db = database();
    let work = json!({"id":"review","rootId":"lead","owner":"worker","status":"running","title":"review","description":"","created":1.0,"updated":1.0,"version":2,"dependencies":[],"results":[],"decisions":[],"createdBy":"worker"});
    let tx = db.transaction().expect("transaction");
    WorkItemsRepository::put_work(&tx, "review", &work).expect("work");
    tx.commit().expect("commit");
    db
}

#[library_benchmark]
#[bench::review(submit_database())]
fn submit(mut db: Connection) -> Connection {
    let tx = db.transaction().expect("transaction");
    black_box(
        write(
            &tx,
            "worker",
            None,
            "submit",
            WorkWrite::Submit(SubmitWork {
                prepared_gate_workspace: None,
                task_id: "review".into(),
                version: Some(2),
                result: "reviewed".into(),
                checks: "read".into(),
                revision: String::new(),
                files: vec![],
                prepared_files: None,
                state_dir: std::env::temp_dir(),
                layr_evidence: None,
                layr_evidence_request_id: None,
            }),
        )
        .expect("submit"),
    );
    tx.commit().expect("commit");
    db
}

fn agent_fixture() -> (Connection, AgentRecord) {
    let db = database();
    let agent = AgentRepository::get(&db, &"worker".into())
        .expect("read")
        .expect("agent");
    (db, agent)
}

#[library_benchmark]
#[bench::worker(agent_fixture())]
fn agent_update((db, agent): (Connection, AgentRecord)) -> (Connection, AgentRecord) {
    AgentRepository::put(black_box(&db), black_box(&agent)).expect("update");
    (db, agent)
}

fn subscribers() -> (
    ResourceNotifications,
    Vec<studio_runtime_services::resource_notify::LiveReceiver>,
) {
    let notifications = ResourceNotifications::new(Arc::new(ChangeNotificationHub::new()));
    let receivers = (0..32).map(|_| notifications.subscribe_live()).collect();
    (notifications, receivers)
}

#[library_benchmark]
#[bench::subscribers(subscribers())]
fn fan_out(
    (notifications, mut receivers): (
        ResourceNotifications,
        Vec<studio_runtime_services::resource_notify::LiveReceiver>,
    ),
) -> (
    ResourceNotifications,
    Vec<studio_runtime_services::resource_notify::LiveReceiver>,
) {
    notifications.publish_live(LiveDelta {
        agent_id: "worker".into(),
        item_id: "item".into(),
        offset: 0,
        text: "delta text".into(),
    });
    for receiver in &mut receivers {
        black_box(receiver.try_recv().expect("event"));
    }
    (notifications, receivers)
}

library_benchmark_group!(
    name = services,
    benchmarks = [create, submit, agent_update, fan_out]
);
main!(
    config = LibraryBenchmarkConfig::default().tool(Dhat::default()),
    library_benchmark_groups = services
);
