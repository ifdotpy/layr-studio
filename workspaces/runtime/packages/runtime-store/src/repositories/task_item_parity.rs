//! How far `runtime_tasks` repeats the transcript items (#630 item 75). The target design
//! makes a command or tool call one Item with its status and output; this check measures
//! the copies before that decision. It only reads.
//!
//! Measured on 2026-11-06 with `examples/task_item_parity.rs` (live canvas, read only): 1,189,992 tasks, each with its item.
//! The task status equals the item `toolStatus` for 1,185,891 (99.66%). Most differences
//! are a task that recovery settled as `lost` or `interrupted` while its item still says
//! `running` (3,735). Of 760,696 command items with inline JSON text, 760,661 have the
//! task's exit code; 74,616 truncated items keep their text elsewhere and are not compared.

use rusqlite::Connection;

use crate::StoreResult;

/// One group of tasks whose status differs from the status of their item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusDifference {
    pub kind: Option<String>,
    pub task_status: Option<String>,
    pub item_status: Option<String>,
    pub count: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskItemParity {
    pub tasks: i64,
    pub with_item: i64,
    pub status_equal: i64,
    pub status_differences: Vec<StatusDifference>,
    /// Command items whose text is the inline JSON of the provider item.
    pub exit_compared: i64,
    pub exit_equal: i64,
}

/// Compare every task with the item of the same id.
pub fn measure(connection: &Connection) -> StoreResult<TaskItemParity> {
    let (tasks, with_item) = connection.query_row(
        "SELECT count(*), count(i.id) FROM runtime_tasks t LEFT JOIN runtime_items i ON i.id=t.id",
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let mut parity = TaskItemParity {
        tasks,
        with_item,
        ..TaskItemParity::default()
    };
    let mut statement = connection.prepare(
        "SELECT json_extract(t.record,'$.kind'), json_extract(t.record,'$.status'),
                json_extract(i.record,'$.toolStatus'), count(*)
         FROM runtime_tasks t JOIN runtime_items i ON i.id=t.id
         GROUP BY 1,2,3 ORDER BY 4 DESC, 1, 2, 3",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(StatusDifference {
            kind: row.get(0)?,
            task_status: row.get(1)?,
            item_status: row.get(2)?,
            count: row.get(3)?,
        })
    })?;
    for row in rows {
        let row = row?;
        if row.task_status.is_some() && row.task_status == row.item_status {
            parity.status_equal += row.count;
        } else {
            parity.status_differences.push(row);
        }
    }
    (parity.exit_compared, parity.exit_equal) = connection.query_row(
        "SELECT count(*), COALESCE(sum(json_extract(json_extract(i.record,'$.text'),'$.exitCode')
                                       IS json_extract(t.record,'$.exitCode')),0)
         FROM runtime_tasks t JOIN runtime_items i ON i.id=t.id
         WHERE json_extract(t.record,'$.kind')='command'
           AND json_valid(json_extract(i.record,'$.text'))",
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    Ok(parity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_and_different_rows_are_counted() {
        let db = Connection::open_in_memory().expect("db");
        db.execute_batch(
            r#"CREATE TABLE runtime_tasks(id TEXT PRIMARY KEY, record TEXT NOT NULL);
               CREATE TABLE runtime_items(id TEXT PRIMARY KEY, agent TEXT, record TEXT NOT NULL, created REAL);
               INSERT INTO runtime_tasks VALUES
                 ('a','{"kind":"command","status":"completed","exitCode":0}'),
                 ('b','{"kind":"command","status":"lost"}'),
                 ('c','{"kind":"tool","status":"completed"}'),
                 ('d','{"kind":"tool","status":"completed"}');
               INSERT INTO runtime_items VALUES
                 ('a','x','{"toolStatus":"completed","text":"{\"exitCode\":0}"}',1),
                 ('b','x','{"toolStatus":"running","text":"{\"exitCode\":1}"}',1),
                 ('c','x','{"toolStatus":"completed","text":"done"}',1);"#,
        )
        .expect("fixture");
        let parity = measure(&db).expect("measure");
        assert_eq!(
            (parity.tasks, parity.with_item, parity.status_equal),
            (4, 3, 2)
        );
        assert_eq!(
            parity.status_differences,
            [StatusDifference {
                kind: Some("command".into()),
                task_status: Some("lost".into()),
                item_status: Some("running".into()),
                count: 1,
            }]
        );
        assert_eq!((parity.exit_compared, parity.exit_equal), (2, 1));
    }
}
