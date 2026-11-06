//! Generated enum check triggers for the voice and graph columns (#630 item 142). The
//! step installs the triggers of every listed column; existing rows are not checked.
use rusqlite::Transaction;

use crate::StoreResult;

pub(super) fn apply(tx: &Transaction<'_>) -> StoreResult<()> {
    crate::enum_columns::install(tx)
}
