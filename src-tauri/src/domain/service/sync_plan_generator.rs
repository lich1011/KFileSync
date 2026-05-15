use std::collections::HashMap;
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{FileEntry, SyncAction, SyncConflict, SyncPlan};
use crate::domain::model::share::SharePermission;
use crate::domain::service::conflict_resolver::ConflictResolver;

pub struct SyncPlanGenerator;

impl SyncPlanGenerator {
    /// Pure function: Generates a sync plan based on local and remote indexes.
    pub fn generate(
        local_index: &[FileEntry],
        remote_index: &[FileEntry],
        _local_device: &DeviceId,
        permission: &SharePermission,
    ) -> SyncPlan {
        let mut plan = SyncPlan {
            to_pull: Vec::new(),
            to_push: Vec::new(),
            conflicts: Vec::new(),
            unchanged: Vec::new(),
        };

        let can_pull = permission.can_pull();
        let can_push = permission.can_push();

        let mut local_map: HashMap<&str, &FileEntry> = HashMap::new();
        for entry in local_index {
            local_map.insert(&entry.path, entry);
        }

        let mut remote_map: HashMap<&str, &FileEntry> = HashMap::new();
        for entry in remote_index {
            remote_map.insert(&entry.path, entry);
        }

        let mut all_paths = local_map.keys().chain(remote_map.keys()).collect::<Vec<_>>();
        all_paths.sort();
        all_paths.dedup();

        for path in all_paths {
            let local_opt = local_map.get(path);
            let remote_opt = remote_map.get(path);

            match (local_opt, remote_opt) {
                (Some(local), None) => {
                    // Only local has it -> Push to remote (unless it's a tombstone)
                    if can_push && !local.deleted {
                        plan.to_push.push(SyncAction {
                            path: path.to_string(),
                            entry: (*local).clone(),
                            missing_blocks: Vec::new(),
                        });
                    }
                }
                (None, Some(remote)) => {
                    // Only remote has it -> Pull from remote (unless it's a tombstone)
                    if can_pull && !remote.deleted {
                        plan.to_pull.push(SyncAction {
                            path: path.to_string(),
                            entry: (*remote).clone(),
                            missing_blocks: Vec::new(),
                        });
                    }
                }
                (Some(local), Some(remote)) => {
                    // Both have it, compare VersionVectors
                    let local_is_ancestor = local.version.is_ancestor_of(&remote.version);
                    let remote_is_ancestor = remote.version.is_ancestor_of(&local.version);

                    if local_is_ancestor && remote_is_ancestor {
                        // Versions are equal
                        plan.unchanged.push(path.to_string());
                    } else if local_is_ancestor && !remote_is_ancestor {
                        // Remote is strictly newer -> Pull
                        if can_pull {
                            plan.to_pull.push(SyncAction {
                                path: path.to_string(),
                                entry: (*remote).clone(),
                                missing_blocks: Vec::new(),
                            });
                        }
                    } else if remote_is_ancestor && !local_is_ancestor {
                        // Local is strictly newer -> Push
                        if can_push {
                            plan.to_push.push(SyncAction {
                                path: path.to_string(),
                                entry: (*local).clone(),
                                missing_blocks: Vec::new(),
                            });
                        }
                    } else {
                        // Conflict!
                        let resolution = ConflictResolver::resolve(local, remote);
                        plan.conflicts.push(SyncConflict {
                            conflict_id: uuid::Uuid::new_v4().to_string(),
                            path: path.to_string(),
                            local: (*local).clone(),
                            remote: (*remote).clone(),
                            resolution,
                        });
                    }
                }
                _ => {}
            }
        }

        plan
    }
}
