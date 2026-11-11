CREATE TRIGGER IF NOT EXISTS runtime_tool_request_alias_index_insert
AFTER INSERT ON runtime_tool_requests BEGIN
    INSERT OR IGNORE INTO runtime_tool_request_alias_index
    SELECT NEW.agent,j.value,NEW.id FROM json_each(NEW.record,'$.aliases') j
    WHERE NEW.agent IS NOT NULL AND typeof(j.value)='text';
END;
CREATE TRIGGER IF NOT EXISTS runtime_tool_request_alias_index_update
AFTER UPDATE OF id,agent,record ON runtime_tool_requests
WHEN OLD.id IS NOT NEW.id OR OLD.agent IS NOT NEW.agent
    OR (OLD.record -> '$.aliases') IS NOT (NEW.record -> '$.aliases') BEGIN
    DELETE FROM runtime_tool_request_alias_index WHERE request_id=OLD.id AND agent=OLD.agent;
    INSERT OR IGNORE INTO runtime_tool_request_alias_index
    SELECT NEW.agent,j.value,NEW.id FROM json_each(NEW.record,'$.aliases') j
    WHERE NEW.agent IS NOT NULL AND typeof(j.value)='text';
END;
CREATE TRIGGER IF NOT EXISTS runtime_tool_request_alias_index_delete
AFTER DELETE ON runtime_tool_requests BEGIN
    DELETE FROM runtime_tool_request_alias_index WHERE request_id=OLD.id AND agent=OLD.agent;
END;
INSERT OR IGNORE INTO runtime_tool_request_alias_index
SELECT r.agent,j.value,r.id FROM runtime_tool_requests r,json_each(r.record,'$.aliases') j
WHERE r.agent IS NOT NULL AND typeof(j.value)='text';
