use crate::interaction_tables::ChatMessagesRow;
use crate::peer_update_settings::{canvas_read, canvas_tx};
use rusqlite::Connection;
use studio_db_api::db_query;

use crate::StoreResult;
use crate::records::{ChatMessage, ChatMessageId, MessageRepository};
use crate::schema::{SchemaRequirements, TableRequirement};

pub(crate) const REQUIREMENTS: SchemaRequirements = SchemaRequirements {
    tables: &[TableRequirement {
        name: "runtime_chat_messages",
        columns: &[
            "seq",
            "id",
            "room",
            "sender",
            "text",
            "created",
            "deliveries",
            "rev",
        ],
    }],
    conditional_tables: &[],
    triggers: &[],
    conditional_triggers: &[],
    connection_functions: &[],
    bootstrap_tables: &[],
    bootstrap_triggers: &[],
    bootstrap_connection_functions: &[],
};

impl MessageRepository {
    pub fn get(connection: &Connection, id: &ChatMessageId) -> StoreResult<Option<ChatMessage>> {
        let rows = db_query!(
            canvas_read(connection),
            ChatMessagesRow,
            filter(ChatMessagesRow::ID.eq(&id.0)),
            limit(1)
        )?;
        Ok(rows.into_iter().next().map(|row| ChatMessage {
            sequence: row.seq,
            id: ChatMessageId(row.id),
            room: row.room,
            sender: row.sender,
            text: row.text,
            created: row.created,
            deliveries: row.deliveries,
        }))
    }

    pub fn put(transaction: &rusqlite::Transaction<'_>, message: &ChatMessage) -> StoreResult<()> {
        canvas_tx(transaction).upsert(&ChatMessagesRow {
            seq: message.sequence,
            id: message.id.0.clone(),
            room: message.room.clone(),
            sender: message.sender.clone(),
            text: message.text.clone(),
            created: message.created,
            deliveries: message.deliveries.clone(),
            rev: 1,
        })?;
        Ok(())
    }
}
inventory::submit! {
    crate::repositories::SchemaRegistration {
        repository: "messages",
        requirements: &REQUIREMENTS,
    }
}
