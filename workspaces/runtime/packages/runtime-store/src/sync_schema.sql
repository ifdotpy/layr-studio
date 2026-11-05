-- section: common
CREATE TABLE IF NOT EXISTS sync_identity (id TEXT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS sync_generation (id INTEGER PRIMARY KEY, value INTEGER NOT NULL);
INSERT OR IGNORE INTO sync_generation VALUES (1, 0);
CREATE TABLE IF NOT EXISTS sync_documents (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL,
    id TEXT NOT NULL,
    payload TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    UNIQUE(scope, id)
);
CREATE INDEX IF NOT EXISTS sync_scope_seq ON sync_documents(scope,seq);
CREATE TABLE IF NOT EXISTS sync_entities (
    collection TEXT NOT NULL,
    id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    hash TEXT NOT NULL,
    payload TEXT,
    deleted INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(collection, id)
);
CREATE INDEX IF NOT EXISTS sync_entities_seq ON sync_entities(seq);
CREATE INDEX IF NOT EXISTS sync_entities_collection_seq ON sync_entities(collection,seq);
CREATE INDEX IF NOT EXISTS sync_entities_non_transcript_seq ON sync_entities(seq)
        WHERE collection NOT LIKE 'transcript:%';
-- Retired subscription fingerprints must not keep maintaining their private indexes.
DROP INDEX IF EXISTS sync_entities_live_root_scope;
DROP INDEX IF EXISTS sync_entities_live_agent_scope;
DROP INDEX IF EXISTS sync_entities_invalid_payload;
CREATE INDEX IF NOT EXISTS sync_entities_collection_deleted
        ON sync_entities(collection,deleted);
CREATE INDEX IF NOT EXISTS sync_entities_tombstone_order
        ON sync_entities(seq,collection,id)
        WHERE deleted=1 AND collection NOT LIKE 'transcript:%';
CREATE TABLE IF NOT EXISTS sync_entity_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
-- Change numbers of the main tables (durable_seq.rs). Its numbers share the counter of
-- sync_entities, sync_documents and sync_generation, so every allocator below reads it.
CREATE TABLE IF NOT EXISTS durable_rows (
        tbl TEXT NOT NULL, id TEXT NOT NULL, seq INTEGER NOT NULL,
        deleted INTEGER NOT NULL DEFAULT 0, at INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(tbl, id));
CREATE INDEX IF NOT EXISTS durable_rows_seq ON durable_rows(seq);
CREATE INDEX IF NOT EXISTS durable_rows_tombstone_at ON durable_rows(at) WHERE deleted=1;
CREATE TABLE IF NOT EXISTS durable_floor (
        id INTEGER PRIMARY KEY, floor INTEGER NOT NULL);
INSERT OR IGNORE INTO durable_floor(id, floor) VALUES (1, 0);
CREATE TRIGGER IF NOT EXISTS sync_entity_tombstone_count_insert
      AFTER INSERT ON sync_entities
      WHEN NEW.deleted=1 AND NEW.collection NOT LIKE 'transcript:%' BEGIN
        UPDATE sync_entity_meta SET value=CAST(value AS INTEGER)+1
        WHERE key='entity_tombstone_count';
      END;
CREATE TRIGGER IF NOT EXISTS sync_entity_tombstone_count_update
      AFTER UPDATE OF deleted,collection ON sync_entities
      WHEN (OLD.deleted=1 AND OLD.collection NOT LIKE 'transcript:%') !=
           (NEW.deleted=1 AND NEW.collection NOT LIKE 'transcript:%') BEGIN
        UPDATE sync_entity_meta SET value=CAST(value AS INTEGER)+
          CASE WHEN NEW.deleted=1 AND NEW.collection NOT LIKE 'transcript:%' THEN 1 ELSE -1 END
        WHERE key='entity_tombstone_count';
      END;
CREATE TRIGGER IF NOT EXISTS sync_entity_tombstone_count_delete
      AFTER DELETE ON sync_entities
      WHEN OLD.deleted=1 AND OLD.collection NOT LIKE 'transcript:%' BEGIN
        UPDATE sync_entity_meta SET value=CAST(value AS INTEGER)-1
        WHERE key='entity_tombstone_count';
      END;
-- Visibility keys are owned by sync_documents::ENTITY_SCOPE_VISIBILITY_KEY_PREFIX.
-- Reassignment invalidates only affected direct selectors and their current team
-- roots. Content updates, ordinary inserts and tombstones remain row deltas. Team
-- membership comes from runtime_agents; its durable trigger (durable_seq.rs) moves
-- the team fence when an agent's root or liveness changes.
CREATE TRIGGER IF NOT EXISTS sync_entity_scope_visibility_update
AFTER UPDATE OF payload,deleted ON sync_entities
WHEN (OLD.deleted=0 AND NEW.deleted=0 AND (
        json_extract(OLD.payload,'$.value.agent') IS NOT json_extract(NEW.payload,'$.value.agent') OR
        json_extract(OLD.payload,'$.value.rootId') IS NOT json_extract(NEW.payload,'$.value.rootId')))
 BEGIN
  INSERT INTO sync_entity_meta(key,value)
  SELECT 'entity_scope_visibility:' || identity,CAST(NEW.seq AS TEXT)
  FROM (
    SELECT json_array(NEW.collection,json_extract(OLD.payload,'$.value.rootId')) AS identity
      WHERE json_extract(OLD.payload,'$.value.rootId') IS NOT NULL AND json_extract(OLD.payload,'$.value.rootId') IS NOT json_extract(NEW.payload,'$.value.rootId')
    UNION SELECT json_array(NEW.collection,json_extract(NEW.payload,'$.value.rootId'))
      WHERE json_extract(NEW.payload,'$.value.rootId') IS NOT NULL AND json_extract(OLD.payload,'$.value.rootId') IS NOT json_extract(NEW.payload,'$.value.rootId')
    UNION SELECT json_array(NEW.collection,selector,'team') FROM (
      SELECT json_extract(OLD.payload,'$.value.agent') AS selector
      UNION SELECT json_extract(NEW.payload,'$.value.agent')
      UNION SELECT root_id FROM runtime_agents
        WHERE deleted_at IS NULL AND id IN
        (json_extract(OLD.payload,'$.value.agent'),json_extract(NEW.payload,'$.value.agent'))
    ) WHERE selector IS NOT NULL AND json_extract(OLD.payload,'$.value.agent') IS NOT json_extract(NEW.payload,'$.value.agent')
  ) WHERE OLD.deleted=0 AND NEW.deleted=0
  ON CONFLICT(key) DO UPDATE SET value=CAST(MAX(CAST(sync_entity_meta.value AS INTEGER),CAST(excluded.value AS INTEGER)) AS TEXT);
END;
-- Tombstone selector provenance has the same lifetime as its retained source row.
-- It is consumed only on restore, so ordinary deletion stays incremental.
CREATE TRIGGER IF NOT EXISTS sync_entity_scope_tombstone_update
AFTER UPDATE OF deleted ON sync_entities
WHEN OLD.deleted IS NOT NEW.deleted BEGIN
  INSERT INTO sync_entity_meta(key,value)
  SELECT 'entity_scope_tombstone:' || json_array(OLD.collection,OLD.id), json_group_array(identity)
  FROM (
    SELECT json_array(OLD.collection,json_extract(OLD.payload,'$.value.rootId')) AS identity
      WHERE json_extract(OLD.payload,'$.value.rootId') IS NOT NULL
    UNION SELECT json_array(OLD.collection,selector,'team') FROM (
      SELECT json_extract(OLD.payload,'$.value.agent') AS selector
      UNION SELECT root_id FROM runtime_agents
        WHERE deleted_at IS NULL AND id=json_extract(OLD.payload,'$.value.agent')
    ) WHERE selector IS NOT NULL
  ) WHERE OLD.deleted=0 AND NEW.deleted=1
  HAVING COUNT(*)>0
  ON CONFLICT(key) DO UPDATE SET value=excluded.value;
  INSERT INTO sync_entity_meta(key,value)
  SELECT 'entity_scope_visibility:' || old.value,CAST(NEW.seq AS TEXT)
  FROM sync_entity_meta AS provenance,json_each(provenance.value) AS old
  WHERE provenance.key='entity_scope_tombstone:' || json_array(NEW.collection,NEW.id)
    AND OLD.deleted=1 AND NEW.deleted=0
    AND EXISTS (
      SELECT value FROM json_each(provenance.value)
      EXCEPT SELECT identity FROM (
        SELECT json_array(NEW.collection,json_extract(NEW.payload,'$.value.rootId')) AS identity
          WHERE json_extract(NEW.payload,'$.value.rootId') IS NOT NULL
        UNION SELECT json_array(NEW.collection,selector,'team') FROM (
          SELECT json_extract(NEW.payload,'$.value.agent') AS selector
          UNION SELECT root_id FROM runtime_agents
            WHERE deleted_at IS NULL AND id=json_extract(NEW.payload,'$.value.agent')
        ) WHERE selector IS NOT NULL
      )
    )
  ON CONFLICT(key) DO UPDATE SET value=CAST(MAX(CAST(sync_entity_meta.value AS INTEGER),CAST(excluded.value AS INTEGER)) AS TEXT);
  DELETE FROM sync_entity_meta
  WHERE key='entity_scope_tombstone:' || json_array(NEW.collection,NEW.id) AND NEW.deleted=0;
END;
CREATE TRIGGER IF NOT EXISTS sync_entity_scope_tombstone_delete
AFTER DELETE ON sync_entities BEGIN
  DELETE FROM sync_entity_meta WHERE key='entity_scope_tombstone:' || json_array(OLD.collection,OLD.id);
END;
INSERT INTO sync_entity_meta(key, value)
SELECT 'entity_tombstone_count', (
    SELECT CAST(COUNT(*) AS TEXT) FROM sync_entities
    WHERE deleted=1 AND collection NOT LIKE 'transcript:%'
)
WHERE NOT EXISTS (
    SELECT 1 FROM sync_entity_meta WHERE key='entity_tombstone_count'
);
INSERT INTO sync_identity(id)
SELECT lower(hex(randomblob(16)))
WHERE NOT EXISTS (SELECT 1 FROM sync_identity);

-- section: runtime_events
-- The event window is built at read time (`entity_scope` in runtime-services): the
-- newest events by this index, numbered by their `durable_rows` entries.
CREATE INDEX IF NOT EXISTS runtime_event_created_id ON runtime_events(created DESC,id);

-- section: runtime_agents
CREATE TABLE IF NOT EXISTS runtime_transcript_revisions(
                        agent TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 0);
CREATE TRIGGER IF NOT EXISTS sync_transcript_revision_runtime_agents_insert AFTER INSERT ON runtime_agents BEGIN INSERT INTO runtime_transcript_revisions(agent,revision)
                                    VALUES (NEW.id,1) ON CONFLICT(agent) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER IF NOT EXISTS sync_transcript_revision_runtime_agents_update AFTER UPDATE ON runtime_agents BEGIN INSERT INTO runtime_transcript_revisions(agent,revision)
                                    VALUES (NEW.id,1) ON CONFLICT(agent) DO UPDATE SET revision=revision+1;
                                    INSERT INTO runtime_transcript_revisions(agent,revision)
                                    SELECT OLD.id,1 WHERE OLD.id!=NEW.id
                                    ON CONFLICT(agent) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER IF NOT EXISTS sync_transcript_revision_runtime_agents_delete AFTER DELETE ON runtime_agents BEGIN INSERT INTO runtime_transcript_revisions(agent,revision)
                                    VALUES (OLD.id,1) ON CONFLICT(agent) DO UPDATE SET revision=revision+1; END;

-- section: runtime_items
CREATE TRIGGER IF NOT EXISTS sync_transcript_revision_runtime_items_insert AFTER INSERT ON runtime_items BEGIN INSERT INTO runtime_transcript_revisions(agent,revision)
                                    VALUES (NEW.agent,1) ON CONFLICT(agent) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER IF NOT EXISTS sync_transcript_revision_runtime_items_update AFTER UPDATE ON runtime_items BEGIN INSERT INTO runtime_transcript_revisions(agent,revision)
                                    VALUES (NEW.agent,1) ON CONFLICT(agent) DO UPDATE SET revision=revision+1;
                                    INSERT INTO runtime_transcript_revisions(agent,revision)
                                    SELECT OLD.agent,1 WHERE OLD.agent!=NEW.agent
                                    ON CONFLICT(agent) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER IF NOT EXISTS sync_transcript_revision_runtime_items_delete AFTER DELETE ON runtime_items BEGIN INSERT INTO runtime_transcript_revisions(agent,revision)
                                    VALUES (OLD.agent,1) ON CONFLICT(agent) DO UPDATE SET revision=revision+1; END;
