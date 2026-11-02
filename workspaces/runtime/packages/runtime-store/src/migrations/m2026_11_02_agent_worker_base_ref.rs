use crate::StoreResult;
use rusqlite::Transaction;

pub(super) const OPTIONAL_FIELDS: &[&str] = &[
    "worktree",
    "workerBaseRef",
    "workspaceModeExplicit",
    "workspaceInGit",
    "quickCreate",
    "reviewDefaults",
    "worktreeWarning",
    "workerBaseCommit",
    "workerBaseBehindMain",
    "workerBaseMainRef",
    "quickCreateRequest",
];

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    let paths: Vec<_> = OPTIONAL_FIELDS
        .iter()
        .copied()
        .chain(["reviewDefaults.effort"])
        .collect();
    let checks = paths.iter().map(|path| {
        if *path == "reviewDefaults" {
            "(json_type(record, '$.reviewDefaults') = 'null' OR (json_type(record, '$.reviewDefaults') = 'object' AND NOT EXISTS (SELECT 1 FROM json_each(record, '$.reviewDefaults'))))".to_owned()
        } else {
            format!("json_type(record, '$.{path}') = 'null'")
        }
    });
    let removals = checks
        .clone()
        .zip(paths.iter())
        .map(|(check, path)| format!("CASE WHEN {check} THEN '$.{path}' ELSE '$[#-1]' END"));
    tx.execute_batch(&format!(
        "UPDATE runtime_agents SET record = json_remove(record, {}) WHERE json_valid(record) AND ({})",
        removals.collect::<Vec<_>>().join(", "), checks.collect::<Vec<_>>().join(" OR ")
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn removes_only_null_worker_base_refs() {
        let mut db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(r#"CREATE TABLE runtime_agents(id TEXT PRIMARY KEY,record TEXT);
            INSERT INTO runtime_agents VALUES ('null', '{"worktree":null,"workerBaseRef":null,"workspaceModeExplicit":null,"workspaceInGit":null,"quickCreate":null,"worktreeWarning":null,"workerBaseCommit":null,"workerBaseBehindMain":null,"workerBaseMainRef":null,"quickCreateRequest":null,"large":1e40,"reviewDefaults":{"model":null,"effort":null}}'), ('missing', '{}'), ('value', '{"workerBaseRef":"main","reviewDefaults":{"model":"m","effort":"high"}}'), ('one', '{"workerBaseRef":"main","reviewDefaults":{"model":"m","effort":null}}'), ('other', '{"workerBaseRef":null,"reviewDefaults":{"model":"m","effort":"high"}}'), ('review', '{"reviewDefaults":null}'), ('empty', '{"reviewDefaults":{}}');"#).unwrap();
        let tx = db.transaction().unwrap();
        super::apply(&tx).unwrap();
        let read = |id: &str| {
            tx.query_row("SELECT record FROM runtime_agents WHERE id=?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
        };
        assert_eq!(
            read("null"),
            r#"{"large":1e40,"reviewDefaults":{"model":null}}"#
        );
        assert_eq!(read("missing"), "{}");
        assert_eq!(
            read("value"),
            r#"{"workerBaseRef":"main","reviewDefaults":{"model":"m","effort":"high"}}"#
        );
        assert_eq!(
            read("one"),
            r#"{"workerBaseRef":"main","reviewDefaults":{"model":"m"}}"#
        );
        assert_eq!(
            read("other"),
            r#"{"reviewDefaults":{"model":"m","effort":"high"}}"#
        );
        assert_eq!(read("review"), "{}");
        assert_eq!(read("empty"), "{}");
    }
}
