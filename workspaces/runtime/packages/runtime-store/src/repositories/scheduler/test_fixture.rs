use crate::records::{AgentId, AgentRecord, AgentRepository, AgentStatus};
use rusqlite::Connection;
use studio_json::json;

pub(super) fn database() -> Connection {
    let mut connection = Connection::open_in_memory().expect("memory database opens");
    crate::receipt_schema::create(&connection).expect("receipt schema");
    connection
        .execute_batch(
            "CREATE TABLE runtime_orchestration_settings(id TEXT PRIMARY KEY,record TEXT);",
        )
        .expect("settings");
    connection
            .execute_batch(
                &crate::agent_columns::fixture_sql("CREATE TABLE runtime_execution_runs(id TEXT PRIMARY KEY,agent TEXT,account TEXT,epoch INTEGER,thread TEXT,turn TEXT,created REAL,record TEXT);
                 CREATE TABLE runtime_execution_attempts(id TEXT PRIMARY KEY,run TEXT,record TEXT);
                 CREATE TABLE runtime_execution_inputs(run TEXT,event TEXT,PRIMARY KEY(run,event));
                 CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT NOT NULL);
                 CREATE TABLE runtime_events(
                   id TEXT PRIMARY KEY,agent TEXT,kind TEXT,text TEXT,status TEXT,
                   created REAL,epoch INTEGER,turn_id TEXT,error TEXT);
                 CREATE TABLE runtime_event_meta(id TEXT PRIMARY KEY,record TEXT NOT NULL);
                 CREATE TABLE sync_entities(collection TEXT,id TEXT,seq INTEGER,hash TEXT,payload TEXT,deleted INTEGER,
                   PRIMARY KEY(collection,id));
                 CREATE TABLE sync_documents(seq INTEGER);
                 CREATE TABLE durable_rows(tbl TEXT,id TEXT,seq INTEGER,deleted INTEGER,at INTEGER,PRIMARY KEY(tbl,id)); CREATE TABLE sync_generation(id INTEGER PRIMARY KEY,value INTEGER NOT NULL); INSERT INTO sync_generation VALUES(1,0);"),
            )
            .expect("scheduler tables create");
    connection
        .execute_batch(crate::agent_volatile::agent_volatile_table_sql())
        .expect("volatile agent table creates");
    crate::test_fixtures::canvas::install_canonical(&connection)
        .expect("canonical scheduler storage");
    AgentRepository::put(
        &connection,
        &AgentRecord {
            in_flight: Some(false),
            auto_wake: Some(true),
            id: AgentId::from("agent"),
            root_id: AgentId::from("agent"),
            epoch: 4,
            status: AgentStatus::Queued,
            provider: studio_runtime_schema::queue_command::Field::Value(
                studio_runtime_schema::ProviderKind::Codex,
            ),
            ..AgentRecord::test_defaults()
        },
    )
    .expect("agent fixture writes");
    let transaction = connection.transaction().expect("projection transaction");
    crate::SyncDocumentsRepository::put_entity(
        &transaction,
        "agent",
        "agent",
        json!({"id":"agent", "status":"queued", "overview":{
            "result":"", "resultTruncated":false, "resultTurnId":null
        }}),
        false,
    )
    .expect("agent sync projection fixture writes");
    transaction.commit().expect("projection commits");
    for (id, created) in [("later", 2.0), ("first", 1.0), ("tie", 1.0)] {
        connection
            .execute(
                "INSERT INTO runtime_events VALUES(?1,'agent','user',?1,'pending',?2,4,NULL,NULL)",
                rusqlite::params![id, created],
            )
            .expect("event fixture writes");
    }
    connection
}
