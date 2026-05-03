use std::path::Path;
use chrono::{DateTime, TimeZone, Utc};
use crate::domain::model::file_entry::{ConflictResolution, FileEntry};

pub struct ConflictResolver;

impl ConflictResolver {
    /// Pure function for deterministic conflict resolution
    pub fn resolve(local: &FileEntry, remote: &FileEntry) -> ConflictResolution {
        // 1. Delete vs Modify -> Modify wins
        if local.deleted && !remote.deleted {
            return ConflictResolution::KeepRemote;
        }
        if !local.deleted && remote.deleted {
            return ConflictResolution::KeepLocal;
        }
        if local.deleted && remote.deleted {
            // Both deleted, safely keep local (it's already a tombstone)
            return ConflictResolution::KeepLocal;
        }

        // 2. Compare modified_at -> Newer wins
        if local.modified_at > remote.modified_at {
            // Local is newer, but remote has diverging changes.
            // We keep local, but we rename remote as conflict copy so user doesn't lose data.
            let conflict_path = Self::generate_conflict_path(&remote.path, remote.modified_at, &remote.modified_by.0);
            return ConflictResolution::KeepBoth { conflict_copy_path: conflict_path };
        } else if remote.modified_at > local.modified_at {
            // Remote is newer, keep remote as main, rename local as conflict
            let conflict_path = Self::generate_conflict_path(&local.path, local.modified_at, &local.modified_by.0);
            return ConflictResolution::KeepBoth { conflict_copy_path: conflict_path };
        }

        // 3. modified_at is equal -> Tiebreaker: DeviceId dictionary order
        // Larger DeviceId string wins (keeps original name). Smaller renamed to conflict.
        if local.modified_by.0 > remote.modified_by.0 {
            let conflict_path = Self::generate_conflict_path(&remote.path, remote.modified_at, &remote.modified_by.0);
            ConflictResolution::KeepBoth { conflict_copy_path: conflict_path }
        } else if remote.modified_by.0 > local.modified_by.0 {
            let conflict_path = Self::generate_conflict_path(&local.path, local.modified_at, &local.modified_by.0);
            ConflictResolution::KeepBoth { conflict_copy_path: conflict_path }
        } else {
            // Exact same device, same timestamp, but conflicts? 
            // In theory this shouldn't happen unless clocks jumped and content differs.
            // We just keep local.
            ConflictResolution::KeepLocal
        }
    }

    /// Generates: <name>.sync-conflict-<YYYYMMDD>-<HHMMSS>-<device_short>.<ext>
    fn generate_conflict_path(original_path: &str, timestamp: u64, device_id: &str) -> String {
        let path = Path::new(original_path);
        let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        
        let datetime = Utc.timestamp_opt(timestamp as i64, 0)
            .single()
            .unwrap_or_else(|| DateTime::<Utc>::from(std::time::UNIX_EPOCH));
        let date_str = datetime.format("%Y%m%d").to_string();
        let time_str = datetime.format("%H%M%S").to_string();
        
        let device_short = if device_id.len() > 8 {
            &device_id[0..8]
        } else {
            device_id
        };

        let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");
        
        let new_filename = if extension.is_empty() {
            format!("{}.sync-conflict-{}-{}-{}", file_stem, date_str, time_str, device_short)
        } else {
            format!("{}.sync-conflict-{}-{}-{}.{}", file_stem, date_str, time_str, device_short, extension)
        };

        if parent.is_empty() {
            new_filename
        } else {
            format!("{}/{}", parent, new_filename)
        }
    }
}
