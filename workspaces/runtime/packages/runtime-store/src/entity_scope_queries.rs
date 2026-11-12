//! Fixed canonical change queries for scoped publication readers.

use crate::agent_tables::AgentsStorageRow;
use crate::change_schema::Changes;
use crate::entity_publication::WorkHistoryKind;
use crate::peer_update_settings::{Canvas, canvas_read};
use crate::planning_work::{TaskRow, WorkRow};
use crate::queue_schema::ExecutionRunsRow;
use crate::work_history::{WorkDecisionsStorageRow, WorkReleasesStorageRow, WorkResultsStorageRow};
use studio_db_api::DbQuery;

/// Storage boundary for scoped read-time publication candidates.
pub struct EntityScopeQueries;

/// Change identity, sequence, deleted selector and nullable raw work-history projections.
pub type WorkHistoryChangeRow = (
    String,
    i64,
    i64,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
);

// Owner: EntityScopeQueries. Unit: durable sequence numbers in a narrow catch-up interval.
const SEQ_INDEX_RANGE_MAX: i64 = 1024;

// Preserve label-first direct reads while seeking the sequence interval in index order.
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64,i64),
    tables(changes=Changes),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID),
    sql="SELECT {id},{seq},{deleted} FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) WHERE {label}=?1 AND {seq}>?2 AND {seq}<=?3 AND ({deleted}=0 OR {seq}>?4) AND (?5 IS NULL OR {id}=?5) ORDER BY {seq},{rowid} LIMIT ?6")]
struct DirectChanges<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    id: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

// An explicitly selected task and its JSON owner are unique identity probes;
// a label/range scan here would traverse every task and agent during catch-up.
#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64,i64),
    tables(changes=Changes,tasks=TaskRow),indexes(changes=Changes::INDEX_CHANGES_DURABLE_IDENTITY),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID,task_id=TaskRow::ID,task_record=TaskRow::RECORD),
    sql="SELECT candidate,MAX(number),MAX(gone) FROM (
 SELECT {id} AS candidate,{seq} AS number,{deleted} AS gone FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) WHERE {label}=?1 AND {seq}>?2 AND {seq}<=?3 AND ({deleted}=0 OR {seq}>?4) AND {id}=?5
 UNION ALL SELECT task.{task_id},owner.{seq},0 FROM {tasks} task JOIN (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) owner ON owner.{label}='runtime_agents' AND owner.{id}=json_extract(task.{task_record},'$.agent')
 WHERE task.{task_id}=?5 AND owner.{seq}>?2 AND owner.{seq}<=?3
 ) GROUP BY candidate ORDER BY MAX(number) LIMIT ?6")]
struct TaskChanges<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    id: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64),
    tables(changes=Changes,agents=AgentsStorageRow),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID,agent_id=AgentsStorageRow::ID,root=AgentsStorageRow::ROOT_ID),
    sql="SELECT {id},MAX({seq}) AS last FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable})
 WHERE {label} IN ('runtime_agents',?1) AND {seq}>?2 AND {seq}<=?3 AND ({deleted}=0 OR {seq}>?4) AND (?5 IS NULL OR {id}=?5)
 AND (?7 IS NULL OR {id}=?7 OR {id} IN (SELECT {agent_id} FROM {agents} WHERE {root}=?7))
 GROUP BY {id} ORDER BY last LIMIT ?6")]
struct AgentChanges<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    id: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
    #[db(column=Changes::DURABLE_ID)]
    root: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64),
    tables(changes=Changes,agents=AgentsStorageRow),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID,agent_id=AgentsStorageRow::ID,root=AgentsStorageRow::ROOT_ID),
    sql="SELECT {id},MAX({seq}) AS last FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable})
 WHERE {label} IN ('runtime_agents',?1) AND {seq}>?2 AND {seq}<=?3 AND ({deleted}=0 OR {seq}>?4) AND (?5 IS NULL OR {id}=?5)
 AND (?7 IS NULL OR {id}=?7 OR {id} IN (SELECT {agent_id} FROM {agents} WHERE {root}=?7))
 GROUP BY {id} ORDER BY last LIMIT ?6")]
struct NarrowAgentChanges<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    id: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
    #[db(column=Changes::DURABLE_ID)]
    root: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64),
    tables(changes=Changes),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID),
    sql="SELECT {id},MAX({seq}) FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) WHERE {label} IN ('runtime_agents','runtime_agent_volatile') AND {seq}>?1 AND {seq}<=?2 AND ({deleted}=0 OR {seq}>?3) AND (?4 IS NULL OR {id}=?4) GROUP BY {id} ORDER BY MAX({seq}) LIMIT ?5")]
struct TailChanges<'a> {
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    id: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=one,native,result=Option<i64>,
    tables(changes=Changes),indexes(changes=Changes::INDEX_CHANGES_DURABLE_IDENTITY),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID),
    sql="SELECT MAX({seq}) FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) WHERE {label} IN ('runtime_agents',?1) AND {id}=?2")]
struct AgentSequence<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::DURABLE_ID)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=optional,native,result=i64,
    tables(changes=Changes),indexes(changes=Changes::INDEX_CHANGES_DURABLE_IDENTITY),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID),
    sql="SELECT {seq} FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) WHERE {label}=?1 AND {id}=?2 AND {deleted}=0")]
struct DirectSequence<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::DURABLE_ID)]
    id: Option<&'a str>,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64,i64),
    tables(changes=Changes,members=Changes),
    indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE,members=Changes::INDEX_CHANGES_DURABLE_IDENTITY),
    predicates(durable=changes,membership=members),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID),
    sql="SELECT x.member,MAX(x.number) AS last,w.{deleted} FROM (
 SELECT {id} AS member,{seq} AS number FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) WHERE {label}=?1 AND {seq}>?3 AND {seq}<=?4 AND ({deleted}=0 OR {seq}>?5)
 UNION ALL SELECT d.{id},d.{seq} FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) d WHERE d.{label}=?2 AND d.{deleted}=0 AND d.{seq}>?3 AND d.{seq}<=?4
 AND EXISTS(SELECT 1 FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {members} WHERE {membership}) w WHERE w.{label}=?1 AND w.{id}=d.{id} AND w.{deleted}=0)
 ) x CROSS JOIN (SELECT {id},{seq},{deleted},{label},{rowid} FROM {members} WHERE {membership}) w ON w.{label}=?1 AND w.{id}=x.member
 WHERE ?6 IS NULL OR x.member=?6 GROUP BY x.member ORDER BY last LIMIT ?7")]
struct WindowChanges<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SOURCE_LABEL)]
    table: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    id: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64,i64),
    tables(changes=Changes,works=WorkRow),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID,work_id=WorkRow::ID,work_record=WorkRow::RECORD),
    sql="SELECT d.{id},d.{seq},d.{deleted} FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) d LEFT JOIN {works} w ON w.{work_id}=d.{id}
 WHERE d.{label}='runtime_work' AND d.{seq}>?1 AND d.{seq}<=?2 AND (d.{deleted}=0 OR d.{seq}>?3)
 AND (?4 IS NULL OR d.{deleted}=1 OR json_extract(w.{work_record},'$.rootId')=?4)
 ORDER BY d.{seq},d.{rowid} LIMIT ?5")]
struct WorkChanges<'a> {
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    root: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64,i64,Option<String>,Option<String>,Option<i64>,Option<String>),
    tables(changes=Changes,children=WorkResultsStorageRow,works=WorkRow),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID,child_id=WorkResultsStorageRow::ID,record=WorkResultsStorageRow::RECORD,work=WorkResultsStorageRow::WORK,idx=WorkResultsStorageRow::IDX,work_id=WorkRow::ID,work_record=WorkRow::RECORD),
    sql="SELECT d.{id},d.{seq},d.{deleted},c.{record},c.{work},c.{idx},json_extract(w.{work_record},'$.rootId')
 FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) d LEFT JOIN {children} c ON c.{child_id}=d.{id} LEFT JOIN {works} w ON w.{work_id}=c.{work}
 WHERE d.{label}=?1 AND d.{seq}>?2 AND d.{seq}<=?3 AND (d.{deleted}=0 OR d.{seq}>?4)
 AND (?5 IS NULL OR d.{deleted}=1 OR json_extract(w.{work_record},'$.rootId')=?5)
 ORDER BY d.{seq},d.{rowid} LIMIT ?6")]
struct Results<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    root: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64,i64,Option<String>,Option<String>,Option<i64>,Option<String>),
    tables(changes=Changes,children=WorkDecisionsStorageRow,works=WorkRow),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID,child_id=WorkDecisionsStorageRow::ID,record=WorkDecisionsStorageRow::RECORD,work=WorkDecisionsStorageRow::WORK,idx=WorkDecisionsStorageRow::IDX,work_id=WorkRow::ID,work_record=WorkRow::RECORD),
    sql="SELECT d.{id},d.{seq},d.{deleted},c.{record},c.{work},c.{idx},json_extract(w.{work_record},'$.rootId')
 FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) d LEFT JOIN {children} c ON c.{child_id}=d.{id} LEFT JOIN {works} w ON w.{work_id}=c.{work}
 WHERE d.{label}=?1 AND d.{seq}>?2 AND d.{seq}<=?3 AND (d.{deleted}=0 OR d.{seq}>?4)
 AND (?5 IS NULL OR d.{deleted}=1 OR json_extract(w.{work_record},'$.rootId')=?5)
 ORDER BY d.{seq},d.{rowid} LIMIT ?6")]
struct Decisions<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    root: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64,i64,Option<String>,Option<String>,Option<i64>,Option<String>),
    tables(changes=Changes,children=WorkReleasesStorageRow,works=WorkRow),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID,child_id=WorkReleasesStorageRow::ID,record=WorkReleasesStorageRow::RECORD,work=WorkReleasesStorageRow::WORK,idx=WorkReleasesStorageRow::IDX,work_id=WorkRow::ID,work_record=WorkRow::RECORD),
    sql="SELECT d.{id},d.{seq},d.{deleted},c.{record},c.{work},c.{idx},json_extract(w.{work_record},'$.rootId')
 FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) d LEFT JOIN {children} c ON c.{child_id}=d.{id} LEFT JOIN {works} w ON w.{work_id}=c.{work}
 WHERE d.{label}=?1 AND d.{seq}>?2 AND d.{seq}<=?3 AND (d.{deleted}=0 OR d.{seq}>?4)
 AND (?5 IS NULL OR d.{deleted}=1 OR json_extract(w.{work_record},'$.rootId')=?5)
 ORDER BY d.{seq},d.{rowid} LIMIT ?6")]
struct Releases<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    deletes_after: Option<i64>,
    #[db(column=Changes::DURABLE_ID)]
    root: Option<&'a str>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64),
    tables(changes=Changes),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID),
    sql="SELECT {id},{seq} FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) WHERE {label}=?1 AND {seq}>?2 AND {seq}<=?3 ORDER BY {seq},{rowid} LIMIT ?4")]
struct ChangedIds<'a> {
    #[db(column=Changes::SOURCE_LABEL)]
    source: Option<&'a str>,
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

#[derive(DbQuery)]
#[db_query(database=Canvas,mode=read,cardinality=many,native,result=(String,i64),
    tables(changes=Changes,runs=ExecutionRunsRow),indexes(changes=Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE),predicates(durable=changes),
    columns(id=Changes::DURABLE_ID,seq=Changes::SEQUENCE,deleted=Changes::DELETED,label=Changes::SOURCE_LABEL,rowid=Changes::SOURCE_ROWID,agent=ExecutionRunsRow::AGENT,run_id=ExecutionRunsRow::ID),
    sql="SELECT r.{agent},d.{seq} FROM (SELECT {id},{seq},{deleted},{label},{rowid} FROM {changes} WHERE {durable}) d JOIN {runs} r ON r.{run_id}=d.{id} WHERE d.{label}='runtime_execution_runs' AND d.{deleted}=0 AND d.{seq}>?1 AND d.{seq}<=?2 ORDER BY d.{seq},d.{rowid} LIMIT ?3")]
struct ChangedRuns {
    #[db(column=Changes::SEQUENCE)]
    after: Option<i64>,
    #[db(column=Changes::SEQUENCE)]
    through: Option<i64>,
    #[db(column=Changes::SOURCE_ROWID)]
    fetch: i64,
}

impl EntityScopeQueries {
    pub fn direct_changes(
        connection: &rusqlite::Connection,
        source: &str,
        after: i64,
        through: i64,
        deletes_after: i64,
        id: Option<&str>,
        fetch: i64,
    ) -> rusqlite::Result<Vec<(String, i64, i64)>> {
        let args = DirectChanges {
            source: Some(source),
            after: Some(after),
            through: Some(through),
            deletes_after: Some(deletes_after),
            id,
            fetch,
        };
        canvas_read(connection).collect(&DirectChanges::QUERY, &args)
    }
    pub fn task_changes(
        connection: &rusqlite::Connection,
        after: i64,
        through: i64,
        deletes_after: i64,
        id: Option<&str>,
        fetch: i64,
    ) -> rusqlite::Result<Vec<(String, i64, i64)>> {
        canvas_read(connection).collect(
            &TaskChanges::QUERY,
            &TaskChanges {
                source: Some("runtime_tasks"),
                after: Some(after),
                through: Some(through),
                deletes_after: Some(deletes_after),
                id,
                fetch,
            },
        )
    }
    pub fn agent_changes(
        connection: &rusqlite::Connection,
        after: i64,
        through: i64,
        deletes_after: i64,
        id: Option<&str>,
        fetch: i64,
        root: Option<&str>,
    ) -> rusqlite::Result<Vec<(String, i64)>> {
        if through.saturating_sub(after) <= SEQ_INDEX_RANGE_MAX {
            return canvas_read(connection).collect(
                &NarrowAgentChanges::QUERY,
                &NarrowAgentChanges {
                    source: Some(crate::durable_seq::AGENT_VIEW_MARK),
                    after: Some(after),
                    through: Some(through),
                    deletes_after: Some(deletes_after),
                    id,
                    fetch,
                    root,
                },
            );
        }
        canvas_read(connection).collect(
            &AgentChanges::QUERY,
            &AgentChanges {
                source: Some(crate::durable_seq::AGENT_VIEW_MARK),
                after: Some(after),
                through: Some(through),
                deletes_after: Some(deletes_after),
                id,
                fetch,
                root,
            },
        )
    }
    pub fn tail_changes(
        connection: &rusqlite::Connection,
        after: i64,
        through: i64,
        deletes_after: i64,
        id: Option<&str>,
        fetch: i64,
    ) -> rusqlite::Result<Vec<(String, i64)>> {
        canvas_read(connection).collect(
            &TailChanges::QUERY,
            &TailChanges {
                after: Some(after),
                through: Some(through),
                deletes_after: Some(deletes_after),
                id,
                fetch,
            },
        )
    }
    pub fn agent_sequence(
        connection: &rusqlite::Connection,
        id: &str,
    ) -> rusqlite::Result<Option<i64>> {
        canvas_read(connection).named(
            &AgentSequence::QUERY,
            &AgentSequence {
                source: Some(crate::durable_seq::AGENT_VIEW_MARK),
                id: Some(id),
            },
        )
    }
    pub fn direct_sequence(
        connection: &rusqlite::Connection,
        source: &str,
        id: &str,
    ) -> rusqlite::Result<Option<i64>> {
        canvas_read(connection).named(
            &DirectSequence::QUERY,
            &DirectSequence {
                source: Some(source),
                id: Some(id),
            },
        )
    }
    pub fn window_changes(
        connection: &rusqlite::Connection,
        window: &str,
        table: &str,
        sequence_range: (i64, i64),
        deletes_after: i64,
        id: Option<&str>,
        fetch: i64,
    ) -> rusqlite::Result<Vec<(String, i64, i64)>> {
        let (after, through) = sequence_range;
        canvas_read(connection).collect(
            &WindowChanges::QUERY,
            &WindowChanges {
                source: Some(window),
                table: Some(table),
                after: Some(after),
                through: Some(through),
                deletes_after: Some(deletes_after),
                id,
                fetch,
            },
        )
    }
    pub fn work_changes(
        connection: &rusqlite::Connection,
        after: i64,
        through: i64,
        deletes_after: i64,
        root: Option<&str>,
        fetch: i64,
    ) -> rusqlite::Result<Vec<(String, i64, i64)>> {
        canvas_read(connection).collect(
            &WorkChanges::QUERY,
            &WorkChanges {
                after: Some(after),
                through: Some(through),
                deletes_after: Some(deletes_after),
                root,
                fetch,
            },
        )
    }
    pub fn history_changes(
        connection: &rusqlite::Connection,
        kind: WorkHistoryKind,
        after: i64,
        through: i64,
        deletes_after: i64,
        root: Option<&str>,
        fetch: i64,
    ) -> rusqlite::Result<Vec<WorkHistoryChangeRow>> {
        let source = Some(crate::work_history::table_of(kind));
        match kind {
            WorkHistoryKind::Result => canvas_read(connection).collect(
                &Results::QUERY,
                &Results {
                    source,
                    after: Some(after),
                    through: Some(through),
                    deletes_after: Some(deletes_after),
                    root,
                    fetch,
                },
            ),
            WorkHistoryKind::Decision => canvas_read(connection).collect(
                &Decisions::QUERY,
                &Decisions {
                    source,
                    after: Some(after),
                    through: Some(through),
                    deletes_after: Some(deletes_after),
                    root,
                    fetch,
                },
            ),
            WorkHistoryKind::Release => canvas_read(connection).collect(
                &Releases::QUERY,
                &Releases {
                    source,
                    after: Some(after),
                    through: Some(through),
                    deletes_after: Some(deletes_after),
                    root,
                    fetch,
                },
            ),
        }
    }
    pub fn changed_ids(
        connection: &rusqlite::Connection,
        source: &str,
        after: i64,
        through: i64,
        fetch: i64,
    ) -> rusqlite::Result<Vec<(String, i64)>> {
        canvas_read(connection).collect(
            &ChangedIds::QUERY,
            &ChangedIds {
                source: Some(source),
                after: Some(after),
                through: Some(through),
                fetch,
            },
        )
    }
    pub fn changed_runs(
        connection: &rusqlite::Connection,
        after: i64,
        through: i64,
        fetch: i64,
    ) -> rusqlite::Result<Vec<(String, i64)>> {
        canvas_read(connection).collect(
            &ChangedRuns::QUERY,
            &ChangedRuns {
                after: Some(after),
                through: Some(through),
                fetch,
            },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::change_schema::{DurableRows, DurableSource};
    use crate::metadata_values::StoredBoolean;
    use crate::peer_update_settings::canvas_tx;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use studio_db_api::{Table, migration::encode_key};

    fn database() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for descriptor in [
            Changes::descriptor(),
            AgentsStorageRow::descriptor(),
            crate::agent_tables::AgentVolatileStorageRow::descriptor(),
            WorkRow::descriptor(),
            TaskRow::descriptor(),
            ExecutionRunsRow::descriptor(),
            WorkResultsStorageRow::descriptor(),
            WorkDecisionsStorageRow::descriptor(),
            WorkReleasesStorageRow::descriptor(),
        ] {
            descriptor.install_if_absent(&db).unwrap();
        }
        db
    }
    fn change(
        tx: &rusqlite::Transaction<'_>,
        source: &str,
        id: &str,
        seq: i64,
        deleted: i64,
        rowid: i64,
    ) {
        canvas_tx(tx)
            .partition::<DurableRows>()
            .put(&DurableRows {
                identity: encode_key(&(source.to_owned(), id.to_owned())),
                source_rowid: rowid,
                source_label: DurableSource::from_stored(source.to_owned()),
                durable_id: id.into(),
                sequence: seq,
                deleted: StoredBoolean::from_stored(deleted),
                deleted_at: 0,
                scope: None,
            })
            .unwrap();
    }
    fn agent(tx: &rusqlite::Transaction<'_>, id: &str, root: &str) {
        let raw = format!(
            r#"{{"id":"{id}","provider":"codex","rootId":"{root}","epoch":0,"status":"unknown","accountKey":"default"}}"#
        );
        let row: crate::AgentRecord = studio_json::from_str(&raw).unwrap();
        canvas_tx(tx)
            .upsert(&AgentsStorageRow {
                hold_status: studio_runtime_schema::persistence::HoldStatus::Clear,
                parent_id: None,
                cwd: None,
                account_key: String::new(),
                id: id.into(),
                root_id: None,
                deleted_at: None,
                record: crate::json_record_codec::RawJsonRecord::from_record(tx, &row).unwrap(),
            })
            .unwrap();
    }
    fn work(tx: &rusqlite::Transaction<'_>, id: &str, root: Option<&str>) {
        let mut raw = format!(r#"{{"id":"{id}","status":"ready""#);
        if let Some(root) = root {
            raw.push_str(&format!(r#", "rootId":"{root}""#));
        }
        raw.push('}');
        let record = crate::WorkItemRecord::decode(id, &raw).unwrap();
        canvas_tx(tx)
            .upsert(&WorkRow::from_record(tx, &record).unwrap())
            .unwrap();
    }
    #[test]
    fn label_sequence_queries_bound_skewed_changes() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        // One frequent label and many unrelated labels, all before the catch-up interval.
        for n in 1..=4000 {
            change(&tx, "runtime_agents", &format!("hot-{n}"), n, 0, n);
        }
        for n in 4001..=16000 {
            change(&tx, &format!("cold-{}", n % 128), &n.to_string(), n, 0, n);
        }
        for (n, label) in [
            "runtime_agents",
            "runtime_agent_volatile",
            "runtime_work",
            "runtime_work_results",
            "runtime_work_decisions",
            "runtime_work_releases",
            "runtime_execution_runs",
            "runtime_tasks",
            "runtime_plans",
            crate::durable_seq::TASK_WINDOW,
        ]
        .into_iter()
        .enumerate()
        {
            let seq = 30000 + n as i64;
            change(&tx, label, "candidate", seq, 0, seq);
        }
        agent(&tx, "hot-1", "root");
        let raw = r#"{"id":"candidate","itemId":"candidate","turnId":"turn","type":"commandExecution","name":"command","agent":"hot-1","kind":"command","status":"completed","created":1}"#;
        canvas_tx(&tx)
            .upsert(&TaskRow {
                id: "candidate".into(),
                record: crate::json_record_codec::RawJsonRecord::from_stored(
                    &tx,
                    "candidate",
                    raw.into(),
                )
                .unwrap(),
            })
            .unwrap();
        canvas_tx(&tx)
            .upsert(&ExecutionRunsRow {
                id: "candidate".into(),
                agent: "hot-1".into(),
                account: "default".into(),
                epoch: 1,
                thread: None,
                turn: None,
                created: 1.0,
                record: "malformed unrelated payload".to_owned().into(),
                status: None,
            })
            .unwrap();
        // An owner-only interval proves the JSON owner branch contributes a candidate.
        assert_eq!(
            EntityScopeQueries::task_changes(&tx, 0, 2, 0, Some("candidate"), 10).unwrap(),
            vec![("candidate".into(), 1, 0)]
        );
        let composite_index = studio_db_api::query_index::<Canvas, Changes>(
            &Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE,
        );
        let identity_index =
            studio_db_api::query_index::<Canvas, Changes>(&Changes::INDEX_CHANGES_DURABLE_IDENTITY);
        for after in [0, 29999] {
            assert_eq!(
                EntityScopeQueries::task_changes(&tx, after, 30020, 0, Some("candidate"), 10)
                    .unwrap(),
                vec![("candidate".into(), 30007, 0)]
            );
            assert_eq!(
                EntityScopeQueries::changed_runs(&tx, after, 30020, 10).unwrap(),
                vec![("hot-1".into(), 30006)]
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "DirectChanges",
                composite_index,
                &DirectChanges::QUERY,
                &DirectChanges {
                    source: Some("runtime_plans"),
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    id: None,
                    fetch: 10,
                },
                None,
                5000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "TaskChanges",
                identity_index,
                &TaskChanges::QUERY,
                &TaskChanges {
                    source: Some("runtime_tasks"),
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    id: Some("candidate"),
                    fetch: 10,
                },
                Some("Task owner union groups candidates and orders their maximum sequence"),
                1000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "AgentChanges",
                composite_index,
                &AgentChanges::QUERY,
                &AgentChanges {
                    source: Some(crate::durable_seq::AGENT_VIEW_MARK),
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    id: None,
                    fetch: 10,
                    root: None,
                },
                Some("Two agent labels must group identities and order their maximum sequence"),
                200000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "NarrowAgentChanges",
                composite_index,
                &NarrowAgentChanges::QUERY,
                &NarrowAgentChanges {
                    source: Some(crate::durable_seq::AGENT_VIEW_MARK),
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    id: None,
                    fetch: 10,
                    root: None,
                },
                Some("Two agent labels must group identities and order their maximum sequence"),
                200000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "TailChanges",
                composite_index,
                &TailChanges::QUERY,
                &TailChanges {
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    id: None,
                    fetch: 10,
                },
                Some("Tail combines two labels by identity and orders their maximum sequence"),
                200000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "WindowChanges",
                composite_index,
                &WindowChanges::QUERY,
                &WindowChanges {
                    source: Some(crate::durable_seq::TASK_WINDOW),
                    table: Some("runtime_tasks"),
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    id: None,
                    fetch: 10,
                },
                Some("Window membership union groups identities and orders their maximum sequence"),
                5000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "WorkChanges",
                composite_index,
                &WorkChanges::QUERY,
                &WorkChanges {
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    root: None,
                    fetch: 10,
                },
                None,
                5000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "Results",
                composite_index,
                &Results::QUERY,
                &Results {
                    source: Some("runtime_work_results"),
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    root: None,
                    fetch: 10,
                },
                None,
                5000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "Decisions",
                composite_index,
                &Decisions::QUERY,
                &Decisions {
                    source: Some("runtime_work_decisions"),
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    root: None,
                    fetch: 10,
                },
                None,
                5000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "Releases",
                composite_index,
                &Releases::QUERY,
                &Releases {
                    source: Some("runtime_work_releases"),
                    after: Some(after),
                    through: Some(30020),
                    deletes_after: Some(0),
                    root: None,
                    fetch: 10,
                },
                None,
                5000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "ChangedIds",
                composite_index,
                &ChangedIds::QUERY,
                &ChangedIds {
                    source: Some("runtime_agents"),
                    after: Some(after),
                    through: Some(30020),
                    fetch: 10,
                },
                None,
                1000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "ChangedRuns",
                composite_index,
                &ChangedRuns::QUERY,
                &ChangedRuns {
                    after: Some(after),
                    through: Some(30020),
                    fetch: 10,
                },
                None,
                5000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "crate::clock_schema::queries::Tombstones",
                composite_index,
                &crate::clock_schema::queries::Tombstones::QUERY,
                &crate::clock_schema::queries::Tombstones {
                    table: "runtime_plans",
                    after,
                    high: 30020,
                },
                None,
                5000,
            );
            crate::query_plan_test_support::assert_label_range(
                &tx,
                "crate::durable_seq::queries::LiveKeys",
                composite_index,
                &crate::durable_seq::queries::LiveKeys::QUERY,
                &crate::durable_seq::queries::LiveKeys {
                    label: Some("runtime_plans"),
                    after: Some(after),
                    high: Some(30020),
                },
                None,
                5000,
            );
        }
    }
    #[test]
    fn entity_scope_target_direct_candidates_preserve_deleted_selectors_bounds_and_source_ties() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        for (id, seq, deleted, rowid) in [
            ("z", 5, 0, -9),
            ("a", 5, 2, 20),
            ("gone", 6, 1, 30),
            ("before", 2, 0, 40),
            ("after", 10, 0, 50),
        ] {
            change(&tx, "runtime_plans", id, seq, deleted, rowid);
        }
        change(&tx, "unrelated", "wrong", 5, 0, 60);
        assert_eq!(
            EntityScopeQueries::direct_changes(&tx, "runtime_plans", 2, 6, 5, None, 10).unwrap(),
            vec![("z".into(), 5, 0), ("gone".into(), 6, 1)]
        );
        assert_eq!(
            EntityScopeQueries::direct_changes(&tx, "runtime_plans", 2, 6, 0, None, 2).unwrap(),
            vec![("z".into(), 5, 0), ("a".into(), 5, 2)]
        );
        assert_eq!(
            EntityScopeQueries::direct_sequence(&tx, "runtime_plans", "a").unwrap(),
            None
        );
        assert_eq!(
            EntityScopeQueries::direct_sequence(&tx, "runtime_plans", "z").unwrap(),
            Some(5)
        );
        assert_eq!(
            EntityScopeQueries::direct_sequence(&tx, "runtime_plans", "missing").unwrap(),
            None
        );
        assert_eq!(
            EntityScopeQueries::changed_ids(&tx, "runtime_plans", 2, 6, 10).unwrap(),
            vec![("z".into(), 5), ("a".into(), 5), ("gone".into(), 6)]
        );
        tx.rollback().unwrap();
        assert!(
            EntityScopeQueries::changed_ids(&db, "runtime_plans", 0, 20, 10)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn entity_scope_target_agent_grouping_scopes_tombstones_and_range_boundaries() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        agent(&tx, "a", "root");
        agent(&tx, "b", "root");
        agent(&tx, "c", "other");
        change(&tx, "runtime_agents", "a", 300001, 0, 1);
        change(&tx, crate::durable_seq::AGENT_VIEW_MARK, "a", 300002, 0, 2);
        change(&tx, "runtime_agents", "b", 300003, 1, 3);
        change(&tx, "runtime_agents", "c", 300004, 2, 4);
        change(&tx, "runtime_agent_volatile", "a", 300005, 0, 5);
        for (after, through) in [
            (300000, 300010),
            (298986, 300010),
            (298985, 300010),
            (0, 300010),
        ] {
            assert_eq!(
                EntityScopeQueries::agent_changes(&tx, after, through, 0, None, 10, Some("root"))
                    .unwrap(),
                vec![("a".into(), 300002), ("b".into(), 300003)]
            );
            assert_eq!(
                EntityScopeQueries::agent_changes(&tx, after, through, i64::MAX, None, 10, None)
                    .unwrap(),
                vec![("a".into(), 300002)]
            );
        }
        assert_eq!(
            EntityScopeQueries::agent_changes(&tx, 300000, 300010, 0, Some("a"), 10, None).unwrap(),
            vec![("a".into(), 300002)]
        );
        assert_eq!(
            EntityScopeQueries::agent_changes(&tx, 300000, 300010, 0, None, 1, None).unwrap(),
            vec![("a".into(), 300002)]
        );
        assert_eq!(
            EntityScopeQueries::agent_sequence(&tx, "a").unwrap(),
            Some(300002)
        );
        assert_eq!(
            EntityScopeQueries::agent_sequence(&tx, "missing").unwrap(),
            None
        );
        assert_eq!(
            EntityScopeQueries::tail_changes(&tx, 300000, 300010, 0, Some("a"), 10).unwrap(),
            vec![("a".into(), 300005)]
        );
        assert!(
            EntityScopeQueries::agent_changes(&tx, 10, 9, 0, None, 10, None)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn entity_scope_target_window_uses_membership_and_exact_source_deleted_zero() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        let window = crate::durable_seq::TASK_WINDOW;
        for (label, id, seq, deleted, rowid) in [
            (window, "live", 2, 0, 1),
            ("runtime_tasks", "live", 7, 0, 2),
            (window, "left", 8, 2, 3),
            ("runtime_tasks", "left", 9, 0, 4),
            ("runtime_tasks", "outsider", 10, 0, 5),
            (window, "odd", 3, 0, 6),
            ("runtime_tasks", "odd", 9, 2, 7),
        ] {
            change(&tx, label, id, seq, deleted, rowid);
        }
        assert_eq!(
            EntityScopeQueries::window_changes(&tx, window, "runtime_tasks", (4, 10), 0, None, 10)
                .unwrap(),
            vec![("live".into(), 7, 0), ("left".into(), 8, 2)]
        );
        assert_eq!(
            EntityScopeQueries::window_changes(
                &tx,
                window,
                "runtime_tasks",
                (4, 10),
                i64::MAX,
                None,
                10
            )
            .unwrap(),
            vec![("live".into(), 7, 0)]
        );
        assert!(
            EntityScopeQueries::window_changes(
                &tx,
                window,
                "runtime_tasks",
                (4, 10),
                0,
                Some("outsider"),
                10
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn entity_scope_target_work_keeps_exact_deleted_one_scope_escape() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        work(&tx, "match", Some("root"));
        work(&tx, "null", None);
        for (id, seq, deleted, rowid) in [
            ("match", 1, 0, 1),
            ("null", 2, 0, 2),
            ("gone", 3, 1, 3),
            ("odd", 4, 2, 4),
        ] {
            change(&tx, "runtime_work", id, seq, deleted, rowid);
        }
        assert_eq!(
            EntityScopeQueries::work_changes(&tx, 0, 10, 0, Some("root"), 10).unwrap(),
            vec![("match".into(), 1, 0), ("gone".into(), 3, 1)]
        );
        assert_eq!(
            EntityScopeQueries::work_changes(&tx, 0, 10, i64::MAX, None, 10).unwrap(),
            vec![("match".into(), 1, 0), ("null".into(), 2, 0)]
        );
    }

    #[derive(DbQuery)]
    #[db_query(database=Canvas,mode=write,cardinality=none,result=(),tables(rows=WorkResultsStorageRow),
        columns(id=WorkResultsStorageRow::ID,work=WorkResultsStorageRow::WORK,idx=WorkResultsStorageRow::IDX,record=WorkResultsStorageRow::RECORD),
        sql="INSERT INTO {rows}({id},{work},{idx},{record}) VALUES(?1,?2,?3,?4)")]
    struct SeedResult<'a> {
        #[db(column=WorkResultsStorageRow::ID)]
        id: &'a str,
        #[db(column=WorkResultsStorageRow::WORK)]
        work: &'a str,
        #[db(column=WorkResultsStorageRow::IDX)]
        idx: i64,
        #[db(column=WorkResultsStorageRow::RECORD)]
        record: crate::json_record_codec::ExactJson<crate::work_history::WorkResultRecord>,
    }
    #[test]
    fn entity_scope_target_history_preserves_raw_tokens_nullable_join_and_tombstones() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        work(&tx, "parent", Some("root"));
        let raw = " { \"number\":2.00, \"escaped\":\"\\u0061\", \"key\":1, \"key\":2 } ";
        canvas_tx(&tx)
            .named(
                &SeedResult::QUERY,
                &SeedResult {
                    id: "entry",
                    work: "parent",
                    idx: 7,
                    record: crate::json_record_codec::ExactJson::from_stored(raw.into()).unwrap(),
                },
            )
            .unwrap();
        for (id, seq, deleted, rowid) in
            [("entry", 1, 0, 1), ("missing", 2, 0, 2), ("gone", 3, 2, 3)]
        {
            change(&tx, "runtime_work_results", id, seq, deleted, rowid);
        }
        let rows =
            EntityScopeQueries::history_changes(&tx, WorkHistoryKind::Result, 0, 10, 0, None, 10)
                .unwrap();
        assert_eq!(
            rows,
            vec![
                (
                    "entry".into(),
                    1,
                    0,
                    Some(raw.into()),
                    Some("parent".into()),
                    Some(7),
                    Some("root".into())
                ),
                ("missing".into(), 2, 0, None, None, None, None),
                ("gone".into(), 3, 2, None, None, None, None)
            ]
        );
        assert_eq!(
            EntityScopeQueries::history_changes(
                &tx,
                WorkHistoryKind::Result,
                0,
                10,
                0,
                Some("root"),
                10
            )
            .unwrap()
            .len(),
            1
        );
        for kind in [WorkHistoryKind::Decision, WorkHistoryKind::Release] {
            change(
                &tx,
                crate::work_history::table_of(kind),
                "gone",
                4,
                1,
                4 + kind as i64,
            );
            let rows =
                EntityScopeQueries::history_changes(&tx, kind, 0, 10, 0, Some("root"), 10).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].2, 1);
        }
    }

    #[test]
    fn entity_scope_target_task_owner_union_and_run_fanout_keep_raw_projection_policy() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        let raw = r#"{"id":"task","itemId":"task","turnId":"turn","type":"commandExecution","name":"command","agent":"owner","kind":"command","status":"completed","created":1}"#;
        canvas_tx(&tx)
            .upsert(&TaskRow {
                id: "task".into(),
                record: crate::json_record_codec::RawJsonRecord::from_stored(
                    &tx,
                    "task",
                    raw.into(),
                )
                .unwrap(),
            })
            .unwrap();
        change(&tx, "runtime_tasks", "task", 2, 2, 1);
        change(&tx, "runtime_agents", "owner", 5, 1, 2);
        assert_eq!(
            EntityScopeQueries::task_changes(&tx, 0, 10, 0, Some("task"), 10).unwrap(),
            vec![("task".into(), 5, 2)]
        );
        assert_eq!(
            EntityScopeQueries::task_changes(&tx, 0, 10, i64::MAX, Some("task"), 10).unwrap(),
            vec![("task".into(), 5, 0)]
        );
        assert!(
            EntityScopeQueries::task_changes(&tx, 0, 10, 0, None, 10)
                .unwrap()
                .is_empty()
        );
        canvas_tx(&tx)
            .upsert(&ExecutionRunsRow {
                id: "run".into(),
                agent: "owner".into(),
                account: "default".into(),
                epoch: 1,
                thread: None,
                turn: None,
                created: 1.0,
                record: "malformed unrelated payload".to_owned().into(),
                status: None,
            })
            .unwrap();
        change(&tx, "runtime_execution_runs", "run", 6, 0, 3);
        assert_eq!(
            EntityScopeQueries::changed_runs(&tx, 0, 10, 10).unwrap(),
            vec![("owner".into(), 6)]
        );
        change(&tx, "runtime_execution_runs", "run", 7, 2, 3);
        assert!(
            EntityScopeQueries::changed_runs(&tx, 0, 10, 10)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn entity_scope_target_hot_queries_use_owned_indexes_and_ignore_unrelated_history() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        change(&tx, "runtime_agents", "live", 300001, 0, 1);
        change(&tx, crate::durable_seq::TASK_WINDOW, "member", 300002, 0, 2);
        change(&tx, "runtime_plans", "plan", 300003, 0, 3);
        let params = NarrowAgentChanges {
            source: Some(crate::durable_seq::AGENT_VIEW_MARK),
            after: Some(300000),
            through: Some(300010),
            deletes_after: Some(0),
            id: None,
            fetch: 10,
            root: None,
        };
        let plan =
            crate::query_plan_test_support::explain(&tx, NarrowAgentChanges::QUERY.sql(), &params)
                .unwrap();
        eprintln!("entity scope narrow agent EQP {plan:?}");
        assert!(
            plan.iter().any(
                |s| s.contains(studio_db_api::query_index::<Canvas, Changes>(
                    &Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE
                ))
            ),
            "{plan:?}"
        );
        let scoped = WindowChanges {
            source: Some(crate::durable_seq::TASK_WINDOW),
            table: Some("runtime_tasks"),
            after: Some(300000),
            through: Some(300010),
            deletes_after: Some(0),
            id: None,
            fetch: 10,
        };
        let plan =
            crate::query_plan_test_support::explain(&tx, WindowChanges::QUERY.sql(), &scoped)
                .unwrap();
        eprintln!("entity scope window EQP {plan:?}");
        assert!(
            plan.iter().any(
                |s| s.contains(studio_db_api::query_index::<Canvas, Changes>(
                    &Changes::INDEX_CHANGES_DURABLE_IDENTITY
                ))
            ),
            "{plan:?}"
        );
        let direct = DirectChanges {
            source: Some("runtime_plans"),
            after: Some(300000),
            through: Some(300010),
            deletes_after: Some(0),
            id: None,
            fetch: 10,
        };
        let plan =
            crate::query_plan_test_support::explain(&tx, DirectChanges::QUERY.sql(), &direct)
                .unwrap();
        eprintln!("entity scope direct EQP {plan:?}");
        assert!(
            plan.iter().any(
                |s| s.contains(studio_db_api::query_index::<Canvas, Changes>(
                    &Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE
                ))
            ),
            "{plan:?}"
        );
        let broad = AgentChanges {
            source: Some(crate::durable_seq::AGENT_VIEW_MARK),
            after: Some(0),
            through: Some(300010),
            deletes_after: Some(0),
            id: None,
            fetch: 10,
            root: None,
        };
        let plan = crate::query_plan_test_support::explain(&tx, AgentChanges::QUERY.sql(), &broad)
            .unwrap();
        eprintln!("entity scope broad agent EQP {plan:?}");
        assert!(
            plan.iter().any(
                |s| s.contains(studio_db_api::query_index::<Canvas, Changes>(
                    &Changes::INDEX_CHANGES_DURABLE_LABEL_SEQUENCE
                ))
            ),
            "{plan:?}"
        );
        let measure = || {
            let steps = Arc::new(AtomicUsize::new(0));
            let counter = steps.clone();
            tx.progress_handler(
                1,
                Some(move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                    false
                }),
            )
            .unwrap();
            assert_eq!(
                EntityScopeQueries::agent_changes(&tx, 300000, 300010, 0, None, 10, None)
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                EntityScopeQueries::agent_changes(&tx, 0, 300010, 0, None, 10, None)
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                EntityScopeQueries::window_changes(
                    &tx,
                    crate::durable_seq::TASK_WINDOW,
                    "runtime_tasks",
                    (300000, 300010),
                    0,
                    None,
                    10
                )
                .unwrap()
                .len(),
                1
            );
            assert_eq!(
                EntityScopeQueries::direct_changes(
                    &tx,
                    "runtime_plans",
                    300000,
                    300010,
                    0,
                    None,
                    10
                )
                .unwrap()
                .len(),
                1
            );
            tx.progress_handler(0, None::<fn() -> bool>).unwrap();
            steps.load(Ordering::Relaxed)
        };
        let before = measure();
        for n in 10..2010 {
            change(&tx, "unrelated", &n.to_string(), n, 1, n);
            change(
                &tx,
                crate::durable_seq::TASK_WINDOW,
                &n.to_string(),
                n,
                1,
                n + 10000,
            );
            change(&tx, "runtime_tasks", &n.to_string(), n, 1, n + 20000);
            change(&tx, "runtime_plans", &n.to_string(), n, 1, n + 30000);
        }
        let after = measure();
        eprintln!("entity scope canonical hot calls VM steps {before}->{after}");
        assert!(
            after <= before + 100,
            "unrelated history: {before}->{after}"
        );
        assert!(after < 3000, "flat bound: {after}");
        for n in 2010..4010 {
            change(&tx, "runtime_agents", &n.to_string(), n, 0, n);
        }
        let steps = Arc::new(AtomicUsize::new(0));
        let counter = steps.clone();
        tx.progress_handler(
            1,
            Some(move || {
                counter.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
        .unwrap();
        assert_eq!(
            EntityScopeQueries::agent_changes(&tx, 300000, 300010, 0, None, 10, None).unwrap(),
            vec![("live".into(), 300001)]
        );
        tx.progress_handler(0, None::<fn() -> bool>).unwrap();
        assert!(
            steps.load(Ordering::Relaxed) < 1000,
            "narrow agent history must seek the interval"
        );
    }
}
