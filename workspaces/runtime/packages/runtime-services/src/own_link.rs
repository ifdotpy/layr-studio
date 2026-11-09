//! The row check for `Scope::OwnLink` (design 2026-10-28, "Permissions": a peer may spawn
//! "for its own link"). A link is a `runtime_server_links` row seen from the home side
//! (`side` is `home`). Its `server` is the peer, and its `parent` is the home agent that
//! the peer's workers hang under. A peer may spawn for `parent` only when such a row
//! exists. Any other case, including an unknown link, is refused.

use rusqlite::Connection;

/// True when `server` owns a home-side link whose parent is `parent`.
pub fn peer_owns_link(
    connection: &Connection,
    server: &str,
    parent: &str,
) -> rusqlite::Result<bool> {
    connection.query_row(
        // Raw SQL: JSON selectors/projections are outside db_query.
        "SELECT EXISTS(SELECT 1 FROM runtime_server_links \
         WHERE json_extract(record,'$.parent')=?2 \
         AND json_extract(record,'$.side')='home' \
         AND json_extract(record,'$.server')=?1)",
        (server, parent),
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_json::json;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE runtime_server_links (id TEXT PRIMARY KEY, record TEXT NOT NULL);",
            )
            .unwrap();
        let rows = [
            (
                "own-link",
                json!({"id":"own-link","side":"home","server":"peer-a","parent":"lead-1","parentEpoch":2,"workers":[]}),
            ),
            (
                "other-peer-link",
                json!({"id":"other-peer-link","side":"home","server":"peer-b","parent":"lead-2","parentEpoch":2,"workers":[]}),
            ),
            (
                "remote-side-link",
                json!({"id":"remote-side-link","side":"remote","server":"peer-a","parent":"lead-3","parentEpoch":2,"workers":[]}),
            ),
        ];
        for (id, record) in rows {
            connection
                .execute(
                    "INSERT INTO runtime_server_links(id,record) VALUES(?1,?2)",
                    (id, record.to_string()),
                )
                .unwrap();
        }
        connection
    }

    #[test]
    fn a_peer_may_spawn_for_its_own_home_link() {
        let connection = database();
        assert!(peer_owns_link(&connection, "peer-a", "lead-1").unwrap());
    }

    #[test]
    fn a_peer_may_not_spawn_for_another_peers_link() {
        let connection = database();
        assert!(!peer_owns_link(&connection, "peer-a", "lead-2").unwrap());
    }

    #[test]
    fn a_remote_side_link_of_the_same_peer_is_refused() {
        let connection = database();
        assert!(!peer_owns_link(&connection, "peer-a", "lead-3").unwrap());
    }

    #[test]
    fn a_local_parent_without_a_link_is_refused() {
        let connection = database();
        assert!(!peer_owns_link(&connection, "peer-a", "local-lead").unwrap());
    }

    #[test]
    fn an_unknown_link_is_refused_for_every_peer() {
        let connection = database();
        assert!(!peer_owns_link(&connection, "stranger", "lead-1").unwrap());
        let empty = Connection::open_in_memory().unwrap();
        empty
            .execute_batch(
                "CREATE TABLE runtime_server_links (id TEXT PRIMARY KEY, record TEXT NOT NULL);",
            )
            .unwrap();
        assert!(!peer_owns_link(&empty, "peer-a", "lead-1").unwrap());
    }
}
