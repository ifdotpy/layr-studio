-- Revision triggers remain raw: DbTable has no trigger declaration.
CREATE TRIGGER IF NOT EXISTS analytics_usage_revision_insert AFTER INSERT ON analytics_usage BEGIN
           UPDATE analytics_usage_revision_clock SET revision=revision+1;
           INSERT INTO analytics_usage_revisions VALUES(NEW.id,(SELECT revision FROM analytics_usage_revision_clock))
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
         END;
CREATE TRIGGER IF NOT EXISTS analytics_usage_revision_update AFTER UPDATE ON analytics_usage BEGIN
           UPDATE analytics_usage_revision_clock SET revision=revision+1;
           INSERT INTO analytics_usage_revisions VALUES(OLD.id,(SELECT revision FROM analytics_usage_revision_clock))
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
           INSERT INTO analytics_usage_revisions VALUES(NEW.id,(SELECT revision FROM analytics_usage_revision_clock))
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
         END;
CREATE TRIGGER IF NOT EXISTS analytics_usage_revision_delete AFTER DELETE ON analytics_usage BEGIN
           UPDATE analytics_usage_revision_clock SET revision=revision+1;
           INSERT INTO analytics_usage_revisions VALUES(OLD.id,(SELECT revision FROM analytics_usage_revision_clock))
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
         END;
CREATE TRIGGER IF NOT EXISTS analytics_cost_group_revision_insert AFTER INSERT ON analytics_cost_groups BEGIN
            UPDATE analytics_usage_revision_clock SET revision=revision+1;
            INSERT INTO analytics_cost_group_revisions VALUES(NEW.rowid,(SELECT revision FROM analytics_usage_revision_clock))
                ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
        END;
CREATE TRIGGER IF NOT EXISTS analytics_cost_group_revision_update AFTER UPDATE ON analytics_cost_groups BEGIN
            UPDATE analytics_usage_revision_clock SET revision=revision+1;
            INSERT INTO analytics_cost_group_revisions VALUES(OLD.rowid,(SELECT revision FROM analytics_usage_revision_clock))
                ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
            INSERT INTO analytics_cost_group_revisions VALUES(NEW.rowid,(SELECT revision FROM analytics_usage_revision_clock))
                ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
        END;
CREATE TRIGGER IF NOT EXISTS analytics_cost_group_revision_delete AFTER DELETE ON analytics_cost_groups BEGIN
            UPDATE analytics_usage_revision_clock SET revision=revision+1;
            INSERT INTO analytics_cost_group_revisions VALUES(OLD.rowid,(SELECT revision FROM analytics_usage_revision_clock))
                ON CONFLICT(id) DO UPDATE SET revision=excluded.revision;
        END;
