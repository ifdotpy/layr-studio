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
