-- Current legacy objects without a reusable static production owner.
-- Dynamic definitions and frozen migration layouts remain explicit until conversion.
CREATE TRIGGER runtime_agent_volatile_owner_delete AFTER DELETE ON runtime_agents BEGIN DELETE FROM runtime_agent_volatile WHERE id=OLD.id; END;
CREATE TRIGGER runtime_annotation_revision AFTER UPDATE OF record ON runtime_annotations WHEN OLD.record IS NOT NEW.record BEGIN UPDATE runtime_annotations SET rev=OLD.rev+1 WHERE id=NEW.id; END;
CREATE TRIGGER runtime_chat_message_revision AFTER UPDATE OF room,sender,text,created,deliveries ON runtime_chat_messages WHEN OLD.room IS NOT NEW.room OR OLD.sender IS NOT NEW.sender OR OLD.text IS NOT NEW.text OR OLD.created IS NOT NEW.created OR OLD.deliveries IS NOT NEW.deliveries BEGIN UPDATE runtime_chat_messages SET rev=OLD.rev+1 WHERE seq=NEW.seq; END;
CREATE TRIGGER runtime_event_revision AFTER UPDATE OF status,text ON runtime_events
         WHEN OLD.status IS NOT NEW.status OR OLD.text IS NOT NEW.text
         BEGIN INSERT INTO runtime_event_revisions(id,rev) VALUES(NEW.id,2)
               ON CONFLICT(id) DO UPDATE SET rev=rev+1; END;
CREATE TRIGGER runtime_event_revision_cleanup AFTER DELETE ON runtime_events
         BEGIN DELETE FROM runtime_event_revisions WHERE id=OLD.id; END;
CREATE TRIGGER "enum_check_request_results_status_ins" BEFORE INSERT ON "request_results" WHEN NEW."status" NOT IN ('applied','rejected') BEGIN SELECT RAISE(ABORT,'request_results.status is outside its enum'); END;
CREATE TRIGGER "enum_check_request_results_status_upd" BEFORE UPDATE OF "status" ON "request_results" WHEN NEW."status" NOT IN ('applied','rejected') BEGIN SELECT RAISE(ABORT,'request_results.status is outside its enum'); END;
CREATE TRIGGER "enum_check_request_results_ref_kind_ins" BEFORE INSERT ON "request_results" WHEN NEW."ref_kind" NOT IN ('agent','annotation','complaint','message','project','room','work','outbox','approval') BEGIN SELECT RAISE(ABORT,'request_results.ref_kind is outside its enum'); END;
CREATE TRIGGER "enum_check_request_results_ref_kind_upd" BEFORE UPDATE OF "ref_kind" ON "request_results" WHEN NEW."ref_kind" NOT IN ('agent','annotation','complaint','message','project','room','work','outbox','approval') BEGIN SELECT RAISE(ABORT,'request_results.ref_kind is outside its enum'); END;
