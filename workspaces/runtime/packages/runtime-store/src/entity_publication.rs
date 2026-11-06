//! Default client publication of agent and request rows.
//!
//! The renderer reads chats and cards only from their `sync_entities` rows, and
//! a pull does not rebuild a complete projection. So every writer transaction
//! publishes the `runtime_agents` and `approvals` rows it changed, in the
//! same transaction and before it commits, unless the transaction already
//! published that entity after its last change. The projection itself belongs
//! to runtime-services, which installs it with
//! [`crate::RuntimeStore::install_entity_publisher`].
//!
//! The writer's update hook journals each changed row. Entity writers journal
//! each publication. A partial projection write that cannot recompute derived
//! view fields journals a stale mark instead, which asks for a full
//! publication. Volatile agent fields (`agent_volatile`) are outside this
//! rule: their writers leave `runtime_agents` alone on purpose.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::StoreResult;

/// The projection that publishes the rows a writer transaction changed.
pub trait EntityPublisher: Send + Sync {
    /// Publish the client views of `pending` inside `transaction`, before it commits.
    fn publish(&self, transaction: &Transaction<'_>, pending: &PendingEntities) -> StoreResult<()>;
}

/// Agent and request ids whose client views are older than their stored rows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PendingEntities {
    pub agents: BTreeSet<String>,
    /// Team leads whose members changed. Their views depend on the members only
    /// through the team counts, so the publisher marks them only when those change.
    pub team_roots: BTreeSet<String>,
    pub receipt_events: BTreeSet<String>,
    pub receipt_items: BTreeSet<String>,
    pub tasks: BTreeSet<String>,
    pub work: BTreeSet<String>,
    /// Source rows of the derived Studio error list that changed.
    pub studio_errors: StudioErrorSources,
    pub queues: BTreeSet<String>,
    pub terminals: BTreeSet<String>,
}

impl PendingEntities {
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
            && self.team_roots.is_empty()
            && self.receipt_events.is_empty()
            && self.receipt_items.is_empty()
            && self.tasks.is_empty()
            && self.work.is_empty()
            && self.studio_errors.is_empty()
            && self.queues.is_empty()
            && self.terminals.is_empty()
    }
}

/// Canonical work history fields and their scoped entity collections.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum WorkHistoryKind {
    Result,
    Decision,
    Release,
}
impl WorkHistoryKind {
    pub const ALL: [Self; 3] = [Self::Result, Self::Decision, Self::Release];
    pub const fn field(self) -> &'static str {
        match self {
            Self::Result => "results",
            Self::Decision => "decisions",
            Self::Release => "releases",
        }
    }
    pub const fn collection(self) -> &'static str {
        match self {
            Self::Result => "workResult",
            Self::Decision => "workDecision",
            Self::Release => "workRelease",
        }
    }
}

/// Ids of the changed rows that the Studio error list reads. The list owner
/// (`studio_errors` in runtime-services) decides which of them can change it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StudioErrorSources {
    pub agents: BTreeSet<String>,
    pub monitors: BTreeSet<String>,
    pub events: BTreeSet<String>,
    /// `runtime_error_incidents` ids, which include dismissals.
    pub incidents: BTreeSet<String>,
    /// An agent, monitor or event row was deleted; its id is gone.
    pub rows_removed: bool,
    /// A diagnostic row was deleted; its id is gone.
    pub incidents_removed: bool,
}

impl StudioErrorSources {
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
            && self.monitors.is_empty()
            && self.events.is_empty()
            && self.incidents.is_empty()
            && !self.rows_removed
            && !self.incidents_removed
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Source {
    Agent,
    Request,
    Event,
    EventMeta,
    Item,
    Task,
    Work,
    QueueAsset,
    Terminal,
    /// Read only by the Studio error list.
    Monitor,
    /// Read only by the Studio error list (dismissals).
    Incident,
    /// A chat row: its agent changed (the chat row owns the agent status).
    Chat,
    /// A turn row: its agent changed (an unfinished turn is the in-flight flag).
    Turn,
}

impl Source {
    fn of_table(table: &str) -> Option<Self> {
        match table {
            "runtime_agents" => Some(Self::Agent),
            "approvals" => Some(Self::Request),
            "runtime_events" => Some(Self::Event),
            "runtime_event_meta" => Some(Self::EventMeta),
            "runtime_items" => Some(Self::Item),
            "runtime_tasks" => Some(Self::Task),
            "runtime_work" => Some(Self::Work),
            "runtime_assets" => Some(Self::QueueAsset),
            "user_terminals" => Some(Self::Terminal),
            "runtime_monitors" => Some(Self::Monitor),
            "runtime_error_incidents" => Some(Self::Incident),
            "chats" => Some(Self::Chat),
            "turns" => Some(Self::Turn),
            _ => None,
        }
    }

    fn of_collection(collection: &str) -> Option<Self> {
        match collection {
            "agent" => Some(Self::Agent),
            "task" => Some(Self::Task),
            "work" => Some(Self::Work),
            _ => None,
        }
    }

    fn table(self) -> &'static str {
        match self {
            Self::Agent => "runtime_agents",
            Self::Request => "approvals",
            Self::Event => "runtime_events",
            Self::EventMeta => "runtime_event_meta",
            Self::Item => "runtime_items",
            Self::Task => "runtime_tasks",
            Self::Work => "runtime_work",
            Self::QueueAsset => "runtime_assets",
            Self::Terminal => "user_terminals",
            Self::Monitor => "runtime_monitors",
            Self::Incident => "runtime_error_incidents",
            Self::Chat => "chats",
            Self::Turn => "turns",
        }
    }
}

#[derive(Debug)]
enum Event {
    /// The update hook saw a write of this row.
    Row(Source, i64),
    /// An entity writer published this entity.
    Published(Source, String),
    /// A partial projection write needs a full publication.
    Stale(Source, String),
    TeamRoot(String),
}

thread_local! {
    /// The journal of the writer transaction that runs on this thread, if any.
    /// The writer connection runs all its transactions on one thread.
    static JOURNAL: RefCell<Option<Vec<Event>>> = const { RefCell::new(None) };
}

fn push(event: Event) {
    JOURNAL.with(|journal| {
        if let Some(events) = journal.borrow_mut().as_mut() {
            events.push(event);
        }
    });
}

/// Capture the old team root before an indexed write can move or remove it.
pub(crate) fn agent_team_root(connection: &Connection, id: &str) -> StoreResult<Option<String>> {
    if !active() {
        return Ok(None);
    }
    Ok(connection
        .prepare_cached("SELECT root_id FROM runtime_agents WHERE id=?1")?
        .query_row([id], |row| row.get(0))
        .optional()?
        .flatten())
}

/// Enqueue roots only after the indexed row has actually changed.
pub(crate) fn mark_agent_team_roots(id: &str, old_root: Option<&str>, new_root: Option<&str>) {
    for root in old_root.into_iter().chain(new_root) {
        if root != id {
            push(Event::TeamRoot(root.to_owned()));
        }
    }
}

/// Start the journal of a writer transaction.
pub(crate) fn begin() {
    JOURNAL.with(|journal| *journal.borrow_mut() = Some(Vec::new()));
}

/// The events of one writer transaction, in write order.
pub(crate) struct Journal(Vec<Event>);

/// End the journal of the current writer transaction and return its events.
pub(crate) fn finish() -> Journal {
    Journal(JOURNAL.with(|journal| journal.borrow_mut().take().unwrap_or_default()))
}

/// Whether the current writer transaction publishes its changed rows before commit.
pub(crate) fn active() -> bool {
    JOURNAL.with(|journal| journal.borrow().is_some())
}

/// Journal a row write that the writer update hook reports.
pub(crate) fn observe_row(table: &str, row_id: i64) {
    if let Some(source) = Source::of_table(table) {
        push(Event::Row(source, row_id));
    }
}

/// Journal that an entity writer published `collection/id`.
pub(crate) fn mark_published(collection: &str, id: &str) {
    if let Some(source) = Source::of_collection(collection) {
        push(Event::Published(source, id.to_owned()));
    }
}

/// Journal that `collection/id` needs a full publication before commit.
pub fn mark_stale(collection: &str, id: &str) {
    if let Some(source) = Source::of_collection(collection) {
        push(Event::Stale(source, id.to_owned()));
    }
}

/// The rows of `events` whose last change comes after their last publication.
pub(crate) fn pending(
    transaction: &Transaction<'_>,
    Journal(events): Journal,
) -> StoreResult<PendingEntities> {
    // A chat or turn row write is a change of its agent: a status-only write leaves the
    // agent record unchanged and writes only these rows (#630 item 63).
    let events = agent_events(transaction, events)?;
    let mut ids: HashMap<(Source, i64), Option<String>> = HashMap::new();
    for event in &events {
        if let Event::Row(source, row_id) = event
            && !ids.contains_key(&(*source, *row_id))
        {
            // A deleted row has no id left; its writer owns the entity removal.
            let id = transaction
                .prepare_cached(&match source {
                    Source::QueueAsset => {
                        "SELECT json_extract(record,'$.agent') FROM runtime_assets WHERE rowid=?1"
                            .to_owned()
                    }
                    Source::Agent
                    | Source::Request
                    | Source::Event
                    | Source::EventMeta
                    | Source::Item
                    | Source::Task
                    | Source::Work
                    | Source::Terminal
                    | Source::Monitor
                    | Source::Incident
                    | Source::Chat
                    | Source::Turn => {
                        format!("SELECT id FROM {} WHERE rowid=?1", source.table())
                    }
                })?
                .query_row([row_id], |row| row.get::<_, String>(0))
                .optional()?;
            ids.insert((*source, *row_id), id);
        }
    }
    let mut pending = PendingEntities::default();
    for ((source, _), id) in &ids {
        if *source == Source::Agent {
            if let Some(id) = id {
                let root: Option<String> = transaction
                    .prepare_cached(
                        "SELECT root_id FROM runtime_agents WHERE id=?1 AND root_id<>id",
                    )?
                    .query_row([id], |row| row.get(0))
                    .optional()?;
                if let Some(root) = root {
                    pending.team_roots.insert(root);
                }
            } else {
                let mut statement = transaction.prepare_cached(
                    "SELECT id FROM runtime_agents WHERE root_id=id AND deleted_at IS NULL",
                )?;
                for row in statement.query_map([], |row| row.get::<_, String>(0))? {
                    pending.team_roots.insert(row?);
                }
            }
        }
        if matches!(
            source,
            Source::Agent | Source::Event | Source::EventMeta | Source::QueueAsset
        ) {
            let owner = match (source, id) {
                (Source::Event | Source::EventMeta, Some(id)) => transaction
                    .prepare_cached("SELECT agent FROM runtime_events WHERE id=?1")?
                    .query_row([id], |row| row.get::<_, String>(0))
                    .optional()?,
                (_, id) => id.clone(),
            };
            if let Some(id) = owner {
                pending.queues.insert(id);
            } else {
                // Deleted source rows no longer carry their owner. Recheck only
                // existing queue views; this path never scans event history.
                let mut statement = transaction.prepare_cached(
                    "SELECT id FROM sync_entities WHERE collection='queue' AND deleted=0",
                )?;
                for id in statement.query_map([], |row| row.get::<_, String>(0))? {
                    pending.queues.insert(id?);
                }
            }
        }
    }
    for ((source, _), id) in &ids {
        if *source == Source::Terminal {
            if let Some(id) = id {
                pending.terminals.insert(id.clone());
            } else {
                let mut statement = transaction.prepare_cached(
                    "SELECT id FROM sync_entities WHERE collection='terminal' AND deleted=0",
                )?;
                for id in statement.query_map([], |row| row.get::<_, String>(0))? {
                    pending.terminals.insert(id?);
                }
            }
        }
    }
    for ((source, _), id) in &ids {
        let studio_errors = &mut pending.studio_errors;
        match (source, id) {
            (Source::Agent, Some(id)) => studio_errors.agents.insert(id.clone()),
            (Source::Monitor, Some(id)) => studio_errors.monitors.insert(id.clone()),
            (Source::Event, Some(id)) => studio_errors.events.insert(id.clone()),
            (Source::Incident, Some(id)) => studio_errors.incidents.insert(id.clone()),
            (Source::Agent | Source::Monitor | Source::Event, None) => {
                studio_errors.rows_removed = true;
                false
            }
            (Source::Incident, None) => {
                studio_errors.incidents_removed = true;
                false
            }
            _ => false,
        };
    }
    // A changed approval changes the attention fields of its agent's summary. The
    // approval documents themselves are built at read time.
    for ((source, _), id) in &ids {
        if *source != Source::Request {
            continue;
        }
        if let Some(id) = id {
            if let Some(agent) = transaction
                .prepare_cached("SELECT agent FROM approvals WHERE id=?1")?
                .query_row([id], |row| row.get::<_, Option<String>>(0))
                .optional()?
                .flatten()
            {
                pending.agents.insert(agent);
            }
        } else {
            let mut statement = transaction
                .prepare_cached("SELECT id FROM runtime_agents WHERE deleted_at IS NULL")?;
            for row in statement.query_map([], |row| row.get::<_, String>(0))? {
                pending.agents.insert(row?);
            }
        }
    }
    let mut changed: BTreeMap<(Source, String), usize> = BTreeMap::new();
    let mut published: HashMap<(Source, String), usize> = HashMap::new();
    for (index, event) in events.into_iter().enumerate() {
        match event {
            Event::Row(source, row_id) => {
                if let Some(Some(id)) = ids.get(&(source, row_id)) {
                    changed.insert((source, id.clone()), index);
                }
            }
            Event::Stale(source, id) => {
                changed.insert((source, id), index);
            }
            Event::Published(source, id) => {
                published.insert((source, id), index);
            }
            Event::TeamRoot(id) => {
                pending.team_roots.insert(id);
            }
        }
    }
    for ((source, id), index) in changed {
        if published
            .get(&(source, id.clone()))
            .is_some_and(|published| *published > index)
        {
            continue;
        }
        match source {
            Source::Agent => pending.agents.insert(id),
            Source::Event | Source::EventMeta => pending.receipt_events.insert(id),
            Source::Item => pending.receipt_items.insert(id),
            Source::Task => pending.tasks.insert(id),
            Source::Work => pending.work.insert(id),
            Source::Request
            | Source::QueueAsset
            | Source::Terminal
            | Source::Monitor
            | Source::Incident
            | Source::Chat
            | Source::Turn => false,
        };
    }
    Ok(pending)
}

/// `events` with each chat or turn row write replaced by a write of its agent row. A
/// row whose agent row is gone is dropped; the agent delete owns that removal.
fn agent_events(transaction: &Transaction<'_>, events: Vec<Event>) -> StoreResult<Vec<Event>> {
    let mut agents: HashMap<(Source, i64), Option<i64>> = HashMap::new();
    let mut result = Vec::with_capacity(events.len());
    for event in events {
        let Event::Row(source @ (Source::Chat | Source::Turn), row_id) = event else {
            result.push(event);
            continue;
        };
        let agent = match agents.get(&(source, row_id)) {
            Some(agent) => *agent,
            None => {
                let sql = if source == Source::Chat {
                    "SELECT a.rowid FROM chats c JOIN runtime_agents a ON a.id=c.id WHERE c.rowid=?1"
                } else {
                    "SELECT a.rowid FROM turns t JOIN runtime_agents a ON a.id=t.chat WHERE t.rowid=?1"
                };
                let agent = transaction
                    .prepare_cached(sql)?
                    .query_row([row_id], |row| row.get::<_, i64>(0))
                    .optional()?;
                agents.insert((source, row_id), agent);
                agent
            }
        };
        if let Some(agent) = agent {
            result.push(Event::Row(Source::Agent, agent));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_resolves_changed_agents_and_events() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE runtime_agents(id TEXT,root_id TEXT,deleted_at INTEGER); CREATE TABLE runtime_events(id TEXT,agent TEXT); CREATE TABLE sync_entities(id TEXT,collection TEXT,deleted INTEGER);").unwrap();
        for i in 0..100 {
            db.execute(
                "INSERT INTO runtime_agents VALUES(?1,'root',NULL)",
                [format!("agent-{i}")],
            )
            .unwrap();
            db.execute(
                "INSERT INTO runtime_events VALUES(?1,?2)",
                [format!("event-{i}"), format!("agent-{i}")],
            )
            .unwrap();
        }
        db.execute(
            "INSERT INTO runtime_events VALUES('event-only','event-only-owner')",
            [],
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        let mut events: Vec<_> = (1..=100)
            .flat_map(|id| [Event::Row(Source::Agent, id), Event::Row(Source::Event, id)])
            .collect();
        events.push(Event::Row(Source::Event, 101));
        let result = pending(&tx, Journal(events)).unwrap();
        assert_eq!(result.agents.len(), 100);
        assert_eq!(result.team_roots, BTreeSet::from(["root".to_owned()]));
        let expected_queues: BTreeSet<_> = (0..100)
            .map(|i| format!("agent-{i}"))
            .chain(["event-only-owner".to_owned()])
            .collect();
        assert_eq!(result.queues, expected_queues);
        assert_eq!(result.receipt_events.len(), 101);
    }
}
