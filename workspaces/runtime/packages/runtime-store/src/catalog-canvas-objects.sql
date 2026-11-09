-- Current legacy objects without a reusable static production owner.
-- Dynamic definitions and frozen migration layouts remain explicit until conversion.
CREATE TRIGGER runtime_agent_record_generation_insert AFTER INSERT ON runtime_agents BEGIN UPDATE runtime_agent_record_generation SET value=value+1 WHERE id=1; UPDATE runtime_scheduler_agent_change_state SET entries=entries+CASE WHEN EXISTS(SELECT 1 FROM runtime_scheduler_agent_changes WHERE id=NEW.id) THEN 0 ELSE 1 END,total=total-COALESCE((SELECT generation FROM runtime_scheduler_agent_changes WHERE id=NEW.id),0)+(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; INSERT INTO runtime_scheduler_agent_changes(id,generation) SELECT NEW.id,value FROM runtime_agent_record_generation WHERE id=1 ON CONFLICT(id) DO UPDATE SET generation=excluded.generation; UPDATE runtime_scheduler_agent_change_state SET generation=(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; END;
CREATE TRIGGER runtime_agent_record_generation_update AFTER UPDATE ON runtime_agents WHEN OLD.record IS NOT NEW.record OR OLD.id IS NOT NEW.id OR OLD.rowid IS NOT NEW.rowid BEGIN UPDATE runtime_agent_record_generation SET value=value+1 WHERE id=1; UPDATE runtime_scheduler_agent_change_state SET entries=entries+CASE WHEN EXISTS(SELECT 1 FROM runtime_scheduler_agent_changes WHERE id=OLD.id) THEN 0 ELSE 1 END,total=total-COALESCE((SELECT generation FROM runtime_scheduler_agent_changes WHERE id=OLD.id),0)+(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1 AND OLD.id IS NOT NEW.id; INSERT INTO runtime_scheduler_agent_changes(id,generation) SELECT OLD.id,value FROM runtime_agent_record_generation WHERE id=1 AND OLD.id IS NOT NEW.id ON CONFLICT(id) DO UPDATE SET generation=excluded.generation; UPDATE runtime_scheduler_agent_change_state SET entries=entries+CASE WHEN EXISTS(SELECT 1 FROM runtime_scheduler_agent_changes WHERE id=NEW.id) THEN 0 ELSE 1 END,total=total-COALESCE((SELECT generation FROM runtime_scheduler_agent_changes WHERE id=NEW.id),0)+(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; INSERT INTO runtime_scheduler_agent_changes(id,generation) SELECT NEW.id,value FROM runtime_agent_record_generation WHERE id=1 ON CONFLICT(id) DO UPDATE SET generation=excluded.generation; UPDATE runtime_scheduler_agent_change_state SET generation=(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; END;
CREATE TRIGGER runtime_agent_record_generation_delete AFTER DELETE ON runtime_agents BEGIN UPDATE runtime_agent_record_generation SET value=value+1 WHERE id=1; UPDATE runtime_scheduler_agent_change_state SET entries=entries+CASE WHEN EXISTS(SELECT 1 FROM runtime_scheduler_agent_changes WHERE id=OLD.id) THEN 0 ELSE 1 END,total=total-COALESCE((SELECT generation FROM runtime_scheduler_agent_changes WHERE id=OLD.id),0)+(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; INSERT INTO runtime_scheduler_agent_changes(id,generation) SELECT OLD.id,value FROM runtime_agent_record_generation WHERE id=1 ON CONFLICT(id) DO UPDATE SET generation=excluded.generation; UPDATE runtime_scheduler_agent_change_state SET generation=(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; END;
CREATE TRIGGER runtime_agent_volatile_journal_insert AFTER INSERT ON runtime_agent_volatile BEGIN UPDATE runtime_agent_record_generation SET value=value+1 WHERE id=1; UPDATE runtime_scheduler_agent_change_state SET entries=entries+CASE WHEN EXISTS(SELECT 1 FROM runtime_scheduler_agent_changes WHERE id=NEW.id) THEN 0 ELSE 1 END,total=total-COALESCE((SELECT generation FROM runtime_scheduler_agent_changes WHERE id=NEW.id),0)+(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; INSERT INTO runtime_scheduler_agent_changes(id,generation) SELECT NEW.id,value FROM runtime_agent_record_generation WHERE id=1 ON CONFLICT(id) DO UPDATE SET generation=excluded.generation; UPDATE runtime_scheduler_agent_change_state SET generation=(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; END;
CREATE TRIGGER runtime_agent_volatile_journal_update AFTER UPDATE ON runtime_agent_volatile BEGIN UPDATE runtime_agent_record_generation SET value=value+1 WHERE id=1; UPDATE runtime_scheduler_agent_change_state SET entries=entries+CASE WHEN EXISTS(SELECT 1 FROM runtime_scheduler_agent_changes WHERE id=NEW.id) THEN 0 ELSE 1 END,total=total-COALESCE((SELECT generation FROM runtime_scheduler_agent_changes WHERE id=NEW.id),0)+(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; INSERT INTO runtime_scheduler_agent_changes(id,generation) SELECT NEW.id,value FROM runtime_agent_record_generation WHERE id=1 ON CONFLICT(id) DO UPDATE SET generation=excluded.generation; UPDATE runtime_scheduler_agent_change_state SET generation=(SELECT value FROM runtime_agent_record_generation WHERE id=1) WHERE id=1; END;
CREATE TRIGGER runtime_agent_volatile_owner_delete AFTER DELETE ON runtime_agents BEGIN DELETE FROM runtime_agent_volatile WHERE id=OLD.id; END;
CREATE TRIGGER runtime_agent_volatile_transcript_insert AFTER INSERT ON runtime_agent_volatile BEGIN INSERT INTO runtime_transcript_revisions(agent,revision) VALUES (NEW.id,1) ON CONFLICT(agent) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER runtime_agent_volatile_transcript_update AFTER UPDATE ON runtime_agent_volatile BEGIN INSERT INTO runtime_transcript_revisions(agent,revision) VALUES (NEW.id,1) ON CONFLICT(agent) DO UPDATE SET revision=revision+1; END;
CREATE TRIGGER summary_plan_delete AFTER DELETE ON runtime_plans BEGIN
             INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted)
             SELECT 'plan',OLD.id,(SELECT max(seq)+1 FROM (
                 SELECT COALESCE(MAX(seq),0) seq FROM sync_entities UNION ALL
                 SELECT MAX(COALESCE(MAX(seq),0),COALESCE((SELECT value FROM sync_generation WHERE id=1),0)) FROM sync_documents UNION ALL
                 SELECT COALESCE(MAX(seq),0) FROM durable_rows)),
                 sync_entity_hash(json_object('collection','plan','id',OLD.id,'value',json('{}'))),json_object('collection','plan','id',OLD.id,'value',json('{}')),1
             WHERE EXISTS(SELECT 1 FROM sync_entities WHERE collection='plan' AND id=OLD.id AND deleted=0)
             ON CONFLICT(collection,id) DO UPDATE SET seq=excluded.seq,hash=excluded.hash,payload=excluded.payload,deleted=1;
             END;
CREATE TRIGGER summary_annotation_delete AFTER DELETE ON runtime_annotations BEGIN
             INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted)
             SELECT 'annotation',OLD.id,(SELECT max(seq)+1 FROM (
                 SELECT COALESCE(MAX(seq),0) seq FROM sync_entities UNION ALL
                 SELECT MAX(COALESCE(MAX(seq),0),COALESCE((SELECT value FROM sync_generation WHERE id=1),0)) FROM sync_documents UNION ALL
                 SELECT COALESCE(MAX(seq),0) FROM durable_rows)),
                 sync_entity_hash(json_object('collection','annotation','id',OLD.id,'value',json('{}'))),json_object('collection','annotation','id',OLD.id,'value',json('{}')),1
             WHERE EXISTS(SELECT 1 FROM sync_entities WHERE collection='annotation' AND id=OLD.id AND deleted=0)
             ON CONFLICT(collection,id) DO UPDATE SET seq=excluded.seq,hash=excluded.hash,payload=excluded.payload,deleted=1;
             END;
CREATE TRIGGER summary_questionHistory_delete AFTER DELETE ON runtime_requests BEGIN
             INSERT INTO sync_entities(collection,id,seq,hash,payload,deleted)
             SELECT 'questionHistory',OLD.id,(SELECT max(seq)+1 FROM (
                 SELECT COALESCE(MAX(seq),0) seq FROM sync_entities UNION ALL
                 SELECT MAX(COALESCE(MAX(seq),0),COALESCE((SELECT value FROM sync_generation WHERE id=1),0)) FROM sync_documents UNION ALL
                 SELECT COALESCE(MAX(seq),0) FROM durable_rows)),
                 sync_entity_hash(json_object('collection','questionHistory','id',OLD.id,'value',json('{}'))),json_object('collection','questionHistory','id',OLD.id,'value',json('{}')),1
             WHERE EXISTS(SELECT 1 FROM sync_entities WHERE collection='questionHistory' AND id=OLD.id AND deleted=0)
             ON CONFLICT(collection,id) DO UPDATE SET seq=excluded.seq,hash=excluded.hash,payload=excluded.payload,deleted=1;
             END;
CREATE TRIGGER runtime_annotation_revision AFTER UPDATE OF record ON runtime_annotations WHEN OLD.record IS NOT NEW.record BEGIN UPDATE runtime_annotations SET rev=OLD.rev+1 WHERE id=NEW.id; END;
CREATE TRIGGER runtime_chat_message_revision AFTER UPDATE OF room,sender,text,created,deliveries ON runtime_chat_messages WHEN OLD.room IS NOT NEW.room OR OLD.sender IS NOT NEW.sender OR OLD.text IS NOT NEW.text OR OLD.created IS NOT NEW.created OR OLD.deliveries IS NOT NEW.deliveries BEGIN UPDATE runtime_chat_messages SET rev=OLD.rev+1 WHERE seq=NEW.seq; END;
CREATE TRIGGER runtime_event_revision AFTER UPDATE OF status,text ON runtime_events
         WHEN OLD.status IS NOT NEW.status OR OLD.text IS NOT NEW.text
         BEGIN INSERT INTO runtime_event_revisions(id,rev) VALUES(NEW.id,2)
               ON CONFLICT(id) DO UPDATE SET rev=rev+1; END;
CREATE TRIGGER runtime_event_revision_cleanup AFTER DELETE ON runtime_events
         BEGIN DELETE FROM runtime_event_revisions WHERE id=OLD.id; END;
CREATE TRIGGER chats_check_insert BEFORE INSERT ON chats WHEN NOT (NEW.state IN ('active','paused','archived') AND NEW.status IN ('unknown','idle','queued','starting','running','waiting','waiting_on_monitor','approval','completed','interrupted','failed','blocked','paused','parked','abandoned','capacity-retry') AND COALESCE(NEW.auto_wake,0) IN (0,1) AND COALESCE(NEW.usage_resume_enabled,0) IN (0,1) AND (NEW.execution_mode IS NULL OR NEW.execution_mode IN ('native','host','vm')))
             BEGIN SELECT RAISE(ABORT,'invalid chats state'); END;
CREATE TRIGGER chats_check_update BEFORE UPDATE ON chats WHEN NOT (NEW.state IN ('active','paused','archived') AND NEW.status IN ('unknown','idle','queued','starting','running','waiting','waiting_on_monitor','approval','completed','interrupted','failed','blocked','paused','parked','abandoned','capacity-retry') AND COALESCE(NEW.auto_wake,0) IN (0,1) AND COALESCE(NEW.usage_resume_enabled,0) IN (0,1) AND (NEW.execution_mode IS NULL OR NEW.execution_mode IN ('native','host','vm')))
             BEGIN SELECT RAISE(ABORT,'invalid chats state'); END;
CREATE TRIGGER turns_check_insert BEFORE INSERT ON turns WHEN NOT (NEW.state IN ('starting','running','completed','failed','interrupted') AND (NEW.state='running')=(NEW.session IS NOT NULL) AND (NEW.interrupt_ticket IS NULL OR NEW.attempt IS NOT NULL))
             BEGIN SELECT RAISE(ABORT,'invalid turns state'); END;
CREATE TRIGGER turns_check_update BEFORE UPDATE ON turns WHEN NOT (NEW.state IN ('starting','running','completed','failed','interrupted') AND (NEW.state='running')=(NEW.session IS NOT NULL) AND (NEW.interrupt_ticket IS NULL OR NEW.attempt IS NOT NULL))
             BEGIN SELECT RAISE(ABORT,'invalid turns state'); END;
CREATE TRIGGER sessions_check_insert BEFORE INSERT ON sessions WHEN NOT (NEW.state IN ('starting','ready','lost'))
             BEGIN SELECT RAISE(ABORT,'invalid sessions state'); END;
CREATE TRIGGER sessions_check_update BEFORE UPDATE ON sessions WHEN NOT (NEW.state IN ('starting','ready','lost'))
             BEGIN SELECT RAISE(ABORT,'invalid sessions state'); END;
CREATE TRIGGER chats_owner_delete AFTER DELETE ON runtime_agents BEGIN
           UPDATE turns SET state='interrupted',session=NULL,attempt=NULL,interrupt_ticket=NULL
             WHERE chat=OLD.id AND state IN ('starting','running');
           DELETE FROM sessions WHERE chat=OLD.id;
           DELETE FROM chats WHERE id=OLD.id;
         END;
CREATE TRIGGER "enum_check_request_results_status_ins" BEFORE INSERT ON "request_results" WHEN NEW."status" NOT IN ('applied','rejected') BEGIN SELECT RAISE(ABORT,'request_results.status is outside its enum'); END;
CREATE TRIGGER "enum_check_request_results_status_upd" BEFORE UPDATE OF "status" ON "request_results" WHEN NEW."status" NOT IN ('applied','rejected') BEGIN SELECT RAISE(ABORT,'request_results.status is outside its enum'); END;
CREATE TRIGGER "enum_check_request_results_ref_kind_ins" BEFORE INSERT ON "request_results" WHEN NEW."ref_kind" NOT IN ('agent','annotation','complaint','message','project','room','work','outbox','approval') BEGIN SELECT RAISE(ABORT,'request_results.ref_kind is outside its enum'); END;
CREATE TRIGGER "enum_check_request_results_ref_kind_upd" BEFORE UPDATE OF "ref_kind" ON "request_results" WHEN NEW."ref_kind" NOT IN ('agent','annotation','complaint','message','project','room','work','outbox','approval') BEGIN SELECT RAISE(ABORT,'request_results.ref_kind is outside its enum'); END;
