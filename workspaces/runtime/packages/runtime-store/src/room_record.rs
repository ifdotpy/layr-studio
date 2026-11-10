use crate::{JsonRecord, RecordKeyMode, RecordQueryShape};
use studio_runtime_schema::room_record::RoomRecord;

impl JsonRecord for RoomRecord {
    type Id = String;
    const TABLE: &'static str = "runtime_rooms";
    const SCHEMA: &'static str = "PersistedRoomRecord";
    const KEY_MODE: RecordKeyMode = RecordKeyMode::PayloadId;
    const QUERY_SHAPE: RecordQueryShape = RecordQueryShape::None;
    const TYPED_SHAPE: bool = true;

    fn record_key(&self) -> Option<&String> {
        Some(&self.id)
    }
}

impl crate::interaction_tables::RoomRow {
    pub fn get(
        connection: &rusqlite::Connection,
        id: &str,
    ) -> crate::StoreResult<Option<RoomRecord>> {
        Ok(crate::peer_update_settings::canvas_read(connection)
            .get::<Self>(&(id.to_owned(),))?
            .map(|row| row.record.record().clone()))
    }

    pub fn put(
        transaction: &rusqlite::Transaction<'_>,
        record: &RoomRecord,
    ) -> crate::StoreResult<()> {
        crate::peer_update_settings::canvas_tx(transaction).upsert(&Self {
            id: record.id.clone(),
            record: crate::json_record_codec::RawJsonRecord::from_record(transaction, record)?,
        })?;
        Ok(())
    }

    pub fn list(connection: &rusqlite::Connection) -> crate::StoreResult<Vec<RoomRecord>> {
        // Hidden rowid ordering is part of the historical list contract.
        Ok(studio_db_api::db_query!(
            crate::peer_update_settings::canvas_read(connection),
            crate::interaction_tables::RoomRow,
            order_by(studio_db_api::Rowid::<crate::interaction_tables::RoomRow>::COLUMN.asc)
        )?
        .into_iter()
        .map(|row| row.record.record().clone())
        .collect())
    }
}
