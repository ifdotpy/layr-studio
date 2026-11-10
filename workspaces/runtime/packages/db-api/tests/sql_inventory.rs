//! These calls must stay visible to the mandatory compiler SQL ratchet.
#[test]
fn compiler_detects_sql_function_alias() {
    let conn = rusqlite::Connection::open_in_memory().expect("fixture");
    let run = rusqlite::Connection::execute;
    run(
        &conn,
        "CREATE TABLE alias_witness(id INTEGER)",
        rusqlite::params![],
    )
    .expect("alias executes");
}

#[test]
fn compiler_detects_sql_in_macro_argument() {
    macro_rules! invoke {
        ($call:expr) => {
            $call
        };
    }
    let conn = rusqlite::Connection::open_in_memory().expect("fixture");
    invoke!(conn.execute(
        "CREATE TABLE macro_witness(id INTEGER)",
        rusqlite::params![]
    ))
    .expect("macro executes");
}
