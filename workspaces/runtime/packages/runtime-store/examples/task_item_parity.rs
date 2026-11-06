//! Measure how far `runtime_tasks` repeats the transcript items (#630 item 75).
//! Usage: `cargo run -p studio-runtime-store --example task_item_parity -- <canvas.sqlite3>`.
//! The database opens read only; a live state folder is safe to measure.

use rusqlite::{Connection, OpenFlags};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: task_item_parity <canvas.sqlite3>")?;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let parity = studio_runtime_store::task_item_parity::measure(&connection)?;
    println!(
        "tasks {} with item {} status equal {} exit compared {} exit equal {}",
        parity.tasks,
        parity.with_item,
        parity.status_equal,
        parity.exit_compared,
        parity.exit_equal
    );
    for difference in parity.status_differences {
        println!(
            "{:?} task {:?} item {:?}: {}",
            difference.kind, difference.task_status, difference.item_status, difference.count
        );
    }
    Ok(())
}
