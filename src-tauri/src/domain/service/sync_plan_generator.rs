use std::collections::HashMap;
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{EntryType, FileEntry, SyncAction, SyncConflict, SyncPlan};
use crate::domain::model::share::SharePermission;
use crate::domain::service::conflict_policy;

use kfilesync_core::domain::BlockInfo as CoreBlockInfo;
use kfilesync_core::domain::EntryType as CoreEntryType;
use kfilesync_core::domain::FileEntry as CoreFileEntry;

/// Sprint 3 desktop upgrade: converts a desktop `FileEntry` into core's flat
/// wire-shape `FileEntry`, purely so `kfilesync_core::service::sync_plan_generator::generate`
/// can classify it. Desktop's own `FileEntry` values (not this conversion's
/// output) are what end up in the returned `SyncPlan` — see [`generate`].
fn to_core_entry(e: &FileEntry) -> CoreFileEntry {
    CoreFileEntry {
        share_id: e.share_id.0.clone(),
        path: e.path.clone(),
        entry_type: match e.entry_type {
            EntryType::Directory => CoreEntryType::Directory,
            // Symlinks were dropped from the unified wire format (ADR-008).
            // `generate()`'s classification never inspects `entry_type` at
            // all (only path/version_vector/deleted), so this placeholder
            // is never observed.
            EntryType::File | EntryType::Symlink => CoreEntryType::File,
        },
        size: e.size,
        modified_at_ms: e.modified_at as i64,
        modified_by: e.modified_by.0.clone(),
        version_vector: e.version.clone(),
        sha256: e.sha256.clone(),
        blocks: e
            .blocks
            .0
            .iter()
            .map(|b| CoreBlockInfo {
                index: b.index,
                size: b.size,
                hash: b.hash.clone(),
            })
            .collect(),
        deleted: e.deleted,
        deleted_at_ms: e.deleted_at.map(|d| d as i64),
    }
}

pub struct SyncPlanGenerator;

impl SyncPlanGenerator {
    /// Pure function; generates a sync plan based on local and remote indexes.
    ///
    /// Sprint 3 desktop upgrade: path classification (push / pull / conflict
    /// / unchanged) is delegated to `kfilesync_core::service::sync_plan_generator::generate`,
    /// which fixes the desktop's tombstone-propagation bug (ADR-009): a path
    /// present on only one side is now push/pull'd even when it is a
    /// tombstone, so deletions actually reach the peer instead of being
    /// silently dropped and later "resurrected". Conflict resolution
    /// (`conflict_policy::decide`) and permission-based filtering (`can_pull`
    /// / `can_push`) remain desktop's own responsibility — core's `generate`
    /// takes no permission parameter (see the plan doc's Sprint 3 notes).
    pub fn generate(
        local_index: &[FileEntry],
        remote_index: &[FileEntry],
        _local_device: &DeviceId,
        permission: &SharePermission,
    ) -> SyncPlan {
        let core_local: Vec<CoreFileEntry> = local_index.iter().map(to_core_entry).collect();
        let core_remote: Vec<CoreFileEntry> = remote_index.iter().map(to_core_entry).collect();
        let core_plan = kfilesync_core::service::sync_plan_generator::generate(&core_local, &core_remote);

        let local_by_path: HashMap<&str, &FileEntry> =
            local_index.iter().map(|e| (e.path.as_str(), e)).collect();
        let remote_by_path: HashMap<&str, &FileEntry> =
            remote_index.iter().map(|e| (e.path.as_str(), e)).collect();

        let can_pull = permission.can_pull();
        let can_push = permission.can_push();

        let mut plan = SyncPlan {
            to_pull: Vec::new(),
            to_push: Vec::new(),
            conflicts: Vec::new(),
            unchanged: Vec::new(),
        };

        if can_pull {
            for e in &core_plan.to_pull {
                if let Some(entry) = remote_by_path.get(e.path.as_str()) {
                    plan.to_pull.push(SyncAction {
                        path: e.path.clone(),
                        entry: (*entry).clone(),
                        missing_blocks: Vec::new(),
                    });
                }
            }
        }

        if can_push {
            for e in &core_plan.to_push {
                if let Some(entry) = local_by_path.get(e.path.as_str()) {
                    plan.to_push.push(SyncAction {
                        path: e.path.clone(),
                        entry: (*entry).clone(),
                        missing_blocks: Vec::new(),
                    });
                }
            }
        }

        for c in &core_plan.conflicts {
            if let (Some(local), Some(remote)) = (
                local_by_path.get(c.path.as_str()),
                remote_by_path.get(c.path.as_str()),
            ) {
                let resolution = conflict_policy::decide(local, remote);
                plan.conflicts.push(SyncConflict {
                    conflict_id: uuid::Uuid::new_v4().to_string(),
                    path: c.path.clone(),
                    local: (*local).clone(),
                    remote: (*remote).clone(),
                    resolution,
                });
            }
        }

        for e in &core_plan.unchanged {
            plan.unchanged.push(e.path.clone());
        }

        plan
    }
}