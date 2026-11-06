//! Durable Canvas chat, edge and message rows.

use rusqlite::{OptionalExtension, Transaction, params};
use studio_json::json;

use crate::StoreResult;
use crate::records::{AgentRecord, Table};
use crate::schema::{SchemaRequirements, TableRequirement};

const GROUPS: &[TableRequirement] = &[TableRequirement {
    name: "groups",
    columns: &["id", "name", "members"],
}];
const EDGES: &[TableRequirement] = &[TableRequirement {
    name: "graph_edges",
    columns: &["id", "source", "target", "kind"],
}];
const MESSAGES: &[TableRequirement] = &[TableRequirement {
    name: "messages",
    columns: &["id", "room", "author", "text", "at", "deliveries"],
}];

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[GROUPS[0], EDGES[0], MESSAGES[0]],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatGroup {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
    pub original_members: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasMessage {
    pub id: String,
    pub room: String,
    pub author: String,
    pub text: String,
    pub at: f64,
    pub deliveries: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalParticipant {
    pub id: String,
    pub author: String,
    pub deliveries: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CoordinationRepository;

impl CoordinationRepository {
    pub fn edge_id(source: &str, target: &str) -> String {
        let encoded = format!(
            "[\"chat\", {}, {}]",
            python_string(source),
            python_string(target)
        );
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(encoded.as_bytes());
        data_encoding::HEXLOWER.encode(&digest[..12])
    }

    pub fn group(connection: &rusqlite::Connection, id: &str) -> StoreResult<Option<ChatGroup>> {
        let row = connection
            .query_row(
                "SELECT id,name,members FROM groups WHERE id=?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(id, name, original_members)| {
            Ok(ChatGroup {
                members: members(connection, &id)?,
                id,
                name,
                original_members,
            })
        })
        .transpose()
    }

    pub fn agents(
        connection: &rusqlite::Connection,
    ) -> StoreResult<Vec<(String, studio_json::Value)>> {
        Ok(Table::<AgentRecord>::new(connection)
            .list()?
            .into_iter()
            .map(|agent| (agent.id().0.clone(), agent.value()))
            .collect())
    }

    pub fn agent_exists(connection: &rusqlite::Connection, id: &str) -> StoreResult<bool> {
        Ok(connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM runtime_agents WHERE id=?1)",
            [id],
            |row| row.get(0),
        )?)
    }

    pub fn federated_room_exists(connection: &rusqlite::Connection, id: &str) -> StoreResult<bool> {
        if !id.starts_with("federated:") {
            return Ok(false);
        }
        let table: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='runtime_federation_rooms')",
            [],
            |row| row.get(0),
        )?;
        if !table {
            return Ok(false);
        }
        Ok(connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM runtime_federation_rooms WHERE id=?1)",
            [id],
            |row| row.get(0),
        )?)
    }

    pub fn messages(
        connection: &rusqlite::Connection,
        room: &str,
    ) -> StoreResult<Vec<CanvasMessage>> {
        let mut statement = connection.prepare_cached("SELECT id,room,author,text,at,deliveries FROM messages WHERE room=?1 ORDER BY at DESC LIMIT 200")?;
        let rows = statement.query_map([room], |row| {
            Ok(CanvasMessage {
                id: row.get(0)?,
                room: row.get(1)?,
                author: row.get(2)?,
                text: row.get(3)?,
                at: row.get(4)?,
                deliveries: row.get(5)?,
            })
        })?;
        let mut values = rows.collect::<Result<Vec<_>, _>>()?;
        values.reverse();
        Ok(values)
    }

    pub fn participants(
        connection: &rusqlite::Connection,
        room: &str,
    ) -> StoreResult<Vec<HistoricalParticipant>> {
        let mut statement =
            connection.prepare_cached("SELECT id,author,deliveries FROM messages WHERE room=?1")?;
        let rows = statement.query_map([room], |row| {
            Ok(HistoricalParticipant {
                id: row.get(0)?,
                author: row.get(1)?,
                deliveries: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn message(
        connection: &rusqlite::Connection,
        id: &str,
    ) -> StoreResult<Option<CanvasMessage>> {
        Ok(connection
            .query_row(
                "SELECT id,room,author,text,at,deliveries FROM messages WHERE id=?1",
                [id],
                |row| {
                    Ok(CanvasMessage {
                        id: row.get(0)?,
                        room: row.get(1)?,
                        author: row.get(2)?,
                        text: row.get(3)?,
                        at: row.get(4)?,
                        deliveries: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn put_group(transaction: &Transaction<'_>, group: &ChatGroup) -> StoreResult<()> {
        transaction.execute(
            "INSERT INTO groups(id,name,members) VALUES(?1,?2,?3)",
            params![group.id, group.name, group.original_members],
        )?;
        Self::mark_chat(transaction, &group.id)
    }

    pub fn put_edge(
        transaction: &Transaction<'_>,
        id: &str,
        source: &str,
        target: &str,
    ) -> StoreResult<()> {
        transaction.execute(
            "INSERT OR IGNORE INTO graph_edges(id,source,target,kind) VALUES(?1,?2,?3,'chat')",
            params![id, source, target],
        )?;
        Self::mark_chat(transaction, target)
    }

    pub fn remove_edge(transaction: &Transaction<'_>, id: &str) -> StoreResult<()> {
        let target: Option<String> = transaction
            .query_row("SELECT target FROM graph_edges WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        transaction.execute("DELETE FROM graph_edges WHERE id=?1", [id])?;
        match target {
            Some(target) => Self::mark_chat(transaction, &target),
            None => Ok(()),
        }
    }

    pub fn put_message(transaction: &Transaction<'_>, message: &CanvasMessage) -> StoreResult<()> {
        transaction.execute(
            "INSERT INTO messages(id,room,author,text,at,deliveries) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                message.id,
                message.room,
                message.author,
                message.text,
                message.at,
                message.deliveries
            ],
        )?;
        Self::mark_chat(transaction, &message.room)
    }

    /// The `chat` document of a chat group at read time; None when it is not a chat.
    pub fn chat_view(
        connection: &rusqlite::Connection,
        id: &str,
    ) -> StoreResult<Option<studio_json::Value>> {
        let Some(group) = Self::group(connection, id)? else {
            return Ok(None);
        };
        let members = members(connection, id)?;
        let last = connection
            .prepare_cached("SELECT text,at FROM messages WHERE room=?1 ORDER BY at DESC LIMIT 1")?
            .query_row([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })
            .optional()?;
        let count: i64 = connection
            .prepare_cached("SELECT count(*) FROM messages WHERE room=?1")?
            .query_row([id], |row| row.get(0))?;
        let tail = last
            .as_ref()
            .map(|item| item.0.chars().take(2_000).collect::<String>())
            .unwrap_or_default();
        Ok(Some(
            json!({"id":id,"name":group.name,"members":members,"kind":"chat","messageCount":count,"tail":tail,"lastMessageAt":last.map(|item| item.1)}),
        ))
    }

    /// Ask readers to build the view of chat `id` again, when `id` is a chat.
    fn mark_chat(transaction: &Transaction<'_>, id: &str) -> StoreResult<()> {
        if Self::group(transaction, id)?.is_some() {
            crate::durable_seq::mark(transaction, crate::durable_seq::CHAT_VIEW_MARK, id)?;
        }
        Ok(())
    }
}

fn members(connection: &rusqlite::Connection, id: &str) -> StoreResult<Vec<String>> {
    let mut statement = connection.prepare_cached(
        "SELECT source FROM graph_edges WHERE target=?1 AND kind='chat' ORDER BY source",
    )?;
    let rows = statement.query_map([id], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn python_string(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\u{8}' => result.push_str("\\b"),
            '\u{c}' => result.push_str("\\f"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c < ' ' => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "coordination",
        requirements: &REQUIREMENTS,
    }
}

#[cfg(test)]
mod tests {
    use super::CoordinationRepository;
    use rusqlite::Connection;

    #[test]
    fn coordination_reads_managed_agents_without_legacy_graph_table() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE runtime_agents(id TEXT PRIMARY KEY, record TEXT NOT NULL)")
            .unwrap();
        connection
            .execute_batch(crate::agent_volatile::agent_volatile_table_sql())
            .unwrap();
        assert!(
            CoordinationRepository::agents(&connection)
                .unwrap()
                .is_empty()
        );
    }
}
