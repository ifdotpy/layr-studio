-- Current legacy objects without a reusable static production owner.
-- Dynamic definitions and frozen migration layouts remain explicit until conversion.
CREATE TABLE analytics_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE analytics_usage_roots (root TEXT PRIMARY KEY, generation INTEGER NOT NULL);
CREATE TABLE analytics_turns (
            id TEXT PRIMARY KEY, agent TEXT NOT NULL, root TEXT, at REAL NOT NULL, record TEXT NOT NULL);
CREATE INDEX analytics_turns_scope ON analytics_turns(agent,at);
CREATE TABLE analytics_notifications (
            id TEXT PRIMARY KEY, agent TEXT, root TEXT, method TEXT, hour INTEGER,
            count INTEGER NOT NULL, bytes INTEGER NOT NULL);
CREATE TABLE analytics_limits (
            id INTEGER PRIMARY KEY AUTOINCREMENT, account TEXT NOT NULL, at REAL NOT NULL, record TEXT NOT NULL);
CREATE TABLE analytics_agents (id TEXT PRIMARY KEY, record TEXT NOT NULL);
CREATE TABLE analytics_usage (
            seq INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE NOT NULL,
            agent TEXT NOT NULL, root TEXT, thread TEXT, turn TEXT, at REAL NOT NULL, record TEXT NOT NULL);
CREATE INDEX analytics_usage_scope ON analytics_usage(agent,at);
CREATE INDEX analytics_usage_team ON analytics_usage(root,at);
CREATE INDEX analytics_usage_root_seq ON analytics_usage(root,seq);
CREATE INDEX analytics_usage_response ON analytics_usage(agent,thread,json_extract(record,'$.responseId'));
CREATE INDEX analytics_usage_thread ON analytics_usage(agent,thread,seq);
CREATE INDEX analytics_usage_migration ON analytics_usage(agent,seq);
CREATE TABLE analytics_items (
            id TEXT PRIMARY KEY, agent TEXT NOT NULL, root TEXT, thread TEXT, turn TEXT,
            at REAL NOT NULL, type TEXT, name TEXT, is_tool INTEGER NOT NULL, record TEXT NOT NULL);
CREATE INDEX analytics_items_scope ON analytics_items(agent,at);
CREATE INDEX analytics_items_team ON analytics_items(root,at);
CREATE INDEX analytics_items_tool ON analytics_items(name,at);
CREATE INDEX analytics_agents_root ON analytics_agents(json_extract(record,'$.rootId'));
CREATE TABLE analytics_history (id TEXT PRIMARY KEY, agent TEXT NOT NULL, record TEXT NOT NULL);
CREATE INDEX analytics_cost_groups_cover
    ON analytics_cost_groups(root,model,account,input,cached,cache_write,output,uncached,cache_write_1h,count);
CREATE TABLE analytics_usage_revision_clock (revision INTEGER NOT NULL);
CREATE TABLE analytics_usage_revisions (id TEXT PRIMARY KEY, revision INTEGER NOT NULL);
CREATE INDEX analytics_usage_revisions_revision ON analytics_usage_revisions(revision);
CREATE TRIGGER analytics_usage_revision_insert AFTER INSERT ON analytics_usage BEGIN
           UPDATE analytics_usage_revision_clock SET revision=revision+1;
           INSERT INTO analytics_usage_revisions VALUES(NEW.id,(SELECT revision FROM analytics_usage_revision_clock))
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
         END;
CREATE TRIGGER analytics_usage_revision_update AFTER UPDATE ON analytics_usage BEGIN
           UPDATE analytics_usage_revision_clock SET revision=revision+1;
           INSERT INTO analytics_usage_revisions VALUES(OLD.id,(SELECT revision FROM analytics_usage_revision_clock))
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
           INSERT INTO analytics_usage_revisions VALUES(NEW.id,(SELECT revision FROM analytics_usage_revision_clock))
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
         END;
CREATE TRIGGER analytics_usage_revision_delete AFTER DELETE ON analytics_usage BEGIN
           UPDATE analytics_usage_revision_clock SET revision=revision+1;
           INSERT INTO analytics_usage_revisions VALUES(OLD.id,(SELECT revision FROM analytics_usage_revision_clock))
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
         END;
CREATE TABLE analytics_cost_group_revisions (id INTEGER PRIMARY KEY, revision INTEGER NOT NULL);
CREATE INDEX analytics_cost_group_revisions_revision ON analytics_cost_group_revisions(revision);
CREATE TRIGGER analytics_cost_group_revision_insert AFTER INSERT ON analytics_cost_groups BEGIN
            UPDATE analytics_usage_revision_clock SET revision=revision+1;
            INSERT INTO analytics_cost_group_revisions VALUES(NEW.rowid,(SELECT revision FROM analytics_usage_revision_clock))
                ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
        END;
CREATE TRIGGER analytics_cost_group_revision_update AFTER UPDATE ON analytics_cost_groups BEGIN
            UPDATE analytics_usage_revision_clock SET revision=revision+1;
            INSERT INTO analytics_cost_group_revisions VALUES(OLD.rowid,(SELECT revision FROM analytics_usage_revision_clock))
                ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
            INSERT INTO analytics_cost_group_revisions VALUES(NEW.rowid,(SELECT revision FROM analytics_usage_revision_clock))
                ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
        END;
CREATE TRIGGER analytics_cost_group_revision_delete AFTER DELETE ON analytics_cost_groups BEGIN
            UPDATE analytics_usage_revision_clock SET revision=revision+1;
            INSERT INTO analytics_cost_group_revisions VALUES(OLD.rowid,(SELECT revision FROM analytics_usage_revision_clock))
                ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
        END;
