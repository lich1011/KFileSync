use crate::domain::model::file_entry::{ConflictResolution, FileEntry};

/// Decide how a `(local, remote)` conflict should be resolved.
///
/// Sprint 2 desktop upgrade: this is the desktop's own policy (unchanged from
/// the old `ConflictResolver::resolve` decision ladder — delete vs. modify,
/// then newer-wins, then device-id tiebreaker). Per
/// [ADR-0007](../../../../KFileSyncCore/docs/adr/0007-conflict-resolver-caller-decides-strategy.md),
/// core does not pick a winner; only the *filename* for the losing side's
/// conflict copy is computed by
/// `kfilesync_core::service::conflict_resolver::conflict_copy_name`, using
/// the losing entry's own `modified_at` (which is milliseconds, per the
/// Sprint 2 timestamp-unit migration) so both peers compute the same name
/// independently.
pub fn decide(local: &FileEntry, remote: &FileEntry) -> ConflictResolution {
    if local.deleted && !remote.deleted {
        return ConflictResolution::KeepRemote;
    }
    if !local.deleted && remote.deleted {
        return ConflictResolution::KeepLocal;
    }
    if local.deleted && remote.deleted {
        // Both deleted, safely keep local (it's already a tombstone).
        return ConflictResolution::KeepLocal;
    }

    let losing = if local.modified_at > remote.modified_at {
        Some(remote)
    } else if remote.modified_at > local.modified_at {
        Some(local)
    } else if local.modified_by.0 > remote.modified_by.0 {
        Some(remote)
    } else if remote.modified_by.0 > local.modified_by.0 {
        Some(local)
    } else {
        // Exact same device, same timestamp, but conflicting versions.
        // In theory this shouldn't happen unless clocks jumped and content
        // differs. We just keep local.
        None
    };

    match losing {
        Some(losing_entry) => {
            let conflict_copy_path = kfilesync_core::service::conflict_resolver::conflict_copy_name(
                &losing_entry.path,
                &losing_entry.modified_by.0,
                losing_entry.modified_at as i64,
            );
            ConflictResolution::KeepBoth { conflict_copy_path }
        }
        None => ConflictResolution::KeepLocal,
    }
}