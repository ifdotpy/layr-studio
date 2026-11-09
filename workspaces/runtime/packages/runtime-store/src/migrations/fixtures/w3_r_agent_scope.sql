CREATE TRIGGER immutable_scope_runtime_agents
             BEFORE UPDATE ON runtime_agents WHEN (OLD.id IS NOT NEW.id) OR (json_valid(OLD.record) AND json_valid(NEW.record) AND json_extract(OLD.record,'$.id') IS NOT json_extract(NEW.record,'$.id')) OR (json_valid(OLD.record) AND json_valid(NEW.record) AND (json_extract(OLD.record,'$.rootId') IS NOT json_extract(NEW.record,'$.rootId') OR json_extract(OLD.record,'$.parentId') IS NOT json_extract(NEW.record,'$.parentId')))
             BEGIN SELECT RAISE(ABORT,'The identity and team scope of an existing entity are immutable'); END;
CREATE TRIGGER immutable_scope_runtime_agents_insert
             BEFORE INSERT ON runtime_agents
             WHEN EXISTS(SELECT 1 FROM runtime_agents previous WHERE previous.id=NEW.id AND ((json_valid(previous.record) AND json_valid(NEW.record) AND (json_extract(previous.record,'$.rootId') IS NOT json_extract(NEW.record,'$.rootId') OR json_extract(previous.record,'$.parentId') IS NOT json_extract(NEW.record,'$.parentId'))) OR (json_valid(previous.record) AND json_valid(NEW.record) AND json_extract(previous.record,'$.id') IS NOT json_extract(NEW.record,'$.id')))) OR (1 AND EXISTS(SELECT 1 FROM durable_rows previous WHERE previous.tbl='runtime_agents' AND previous.id=NEW.id AND previous.deleted=1 AND previous.scope IS NOT NULL AND previous.scope IS NOT (CASE WHEN json_valid(NEW.record) THEN json_object('rootId',json_extract(NEW.record,'$.rootId'),'parentId',json_extract(NEW.record,'$.parentId')) END)))
             BEGIN SELECT RAISE(ABORT,'The identity and team scope of an existing entity are immutable'); END;
CREATE TRIGGER durable_runtime_plans_delete AFTER DELETE ON runtime_plans BEGIN
  UPDATE durable_floor SET floor=MAX(floor,
    (SELECT MAX(seq) FROM durable_rows WHERE deleted=1 AND at<CAST(strftime('%s','now') AS INTEGER)-7776000))
  WHERE id=1 AND EXISTS(SELECT 1 FROM durable_rows WHERE deleted=1 AND at<CAST(strftime('%s','now') AS INTEGER)-7776000);
  DELETE FROM durable_rows WHERE deleted=1 AND at<CAST(strftime('%s','now') AS INTEGER)-7776000;
  INSERT INTO durable_rows(tbl,id,seq,deleted,at,scope) VALUES('runtime_plans',OLD.id,(SELECT MAX(entity_seq,document_seq,generation,durable_seq)+1 FROM (
    SELECT COALESCE(MAX(seq),0) AS entity_seq FROM sync_entities),
    (SELECT COALESCE(MAX(seq),0) AS document_seq FROM sync_documents),
    (SELECT COALESCE(MAX(value),0) AS generation FROM sync_generation WHERE id=1),
    (SELECT COALESCE(MAX(seq),0) AS durable_seq FROM durable_rows)),1,CAST(strftime('%s','now') AS INTEGER),CASE WHEN json_valid(OLD.record) THEN json_object('rootId',json_extract(OLD.record,'$.rootId')) END)
             ON CONFLICT(tbl,id) DO UPDATE SET seq=excluded.seq,deleted=excluded.deleted,at=excluded.at,scope=excluded.scope;

END;
