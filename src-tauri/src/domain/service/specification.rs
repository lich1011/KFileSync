use std::path::Path;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use crate::domain::model::device::{Device, DeviceState};
use crate::domain::model::share::{Share, SyncMode};

pub trait Specification<T> {
    fn is_satisfied_by(&self, candidate: &T) -> bool;
}

pub struct AndSpec<A, B>(pub A, pub B);

impl<T, A, B> Specification<T> for AndSpec<A, B>
where
    A: Specification<T>,
    B: Specification<T>,
{
    fn is_satisfied_by(&self, candidate: &T) -> bool {
        self.0.is_satisfied_by(candidate) && self.1.is_satisfied_by(candidate)
    }
}

// ---------------------------------------------------------
// Unified Context for Sync Authorization
// ---------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncDirection {
    Push,
    Pull,
}

pub struct SyncContext<'a> {
    pub device: &'a Device,
    pub share: &'a Share,
    pub action: SyncDirection,
}

// ---------------------------------------------------------
// Device Trust Specification
// ---------------------------------------------------------

pub struct TrustedDeviceSpec;

impl<'a> Specification<SyncContext<'a>> for TrustedDeviceSpec {
    fn is_satisfied_by(&self, candidate: &SyncContext<'a>) -> bool {
        matches!(candidate.device.state, DeviceState::Paired(_))
    }
}

// ---------------------------------------------------------
// Share Member Specification
// ---------------------------------------------------------

pub struct ShareMemberSpec;

impl<'a> Specification<SyncContext<'a>> for ShareMemberSpec {
    fn is_satisfied_by(&self, candidate: &SyncContext<'a>) -> bool {
        candidate.share.has_member(&candidate.device.id)
    }
}

// ---------------------------------------------------------
// Permission Specification
// ---------------------------------------------------------

pub struct PermissionSpec;

impl<'a> Specification<SyncContext<'a>> for PermissionSpec {
    fn is_satisfied_by(&self, candidate: &SyncContext<'a>) -> bool {
        // 1. Check global Share sync_mode
        match candidate.share.sync_mode {
            SyncMode::TwoWay => {} // Normal
            SyncMode::SendOnly => {
                // Share is SendOnly (local device sends to peers).
                // Peers are only allowed to Pull. They cannot Push to us.
                if candidate.action == SyncDirection::Push {
                    return false;
                }
            }
            SyncMode::ReceiveOnly => {
                // Share is ReceiveOnly (local device receives from peers).
                // Peers are only allowed to Push. They cannot Pull from us.
                if candidate.action == SyncDirection::Pull {
                    return false;
                }
            }
        }

        // 2. Check per-member permission
        if let Some(perm) = candidate.share.get_permission(&candidate.device.id) {
            match candidate.action {
                SyncDirection::Push => perm.can_push(),
                SyncDirection::Pull => perm.can_pull(),
            }
        } else {
            false
        }
    }
}

// ---------------------------------------------------------
// Ignore Specification (.syncignore)
// ---------------------------------------------------------

pub struct IgnoreSpec {
    gitignore: Gitignore,
}

impl IgnoreSpec {
    pub fn new(base_dir: &Path, additional_rules: &[&str]) -> Result<Self, ignore::Error> {
        let mut builder = GitignoreBuilder::new(base_dir);
        
        // Add built-in default ignore rules
        let defaults = [
            ".DS_Store",
            "Thumbs.db",
            "desktop.ini",
            "$RECYCLE.BIN",
            ".lansync-tmp/",
        ];
        
        for rule in defaults {
            builder.add_line(None, rule)?;
        }
        
        for rule in additional_rules {
            builder.add_line(None, rule)?;
        }
        
        let gitignore = builder.build()?;
        Ok(Self { gitignore })
    }
    
    pub fn from_file(ignore_file: &Path) -> Result<Self, ignore::Error> {
        let base_dir = ignore_file.parent().unwrap_or(Path::new(""));
        let mut builder = GitignoreBuilder::new(base_dir);
        
        // Add built-in defaults
        let defaults = [
            ".DS_Store",
            "Thumbs.db",
            "desktop.ini",
            "$RECYCLE.BIN",
            ".lansync-tmp/",
        ];
        for rule in defaults {
            builder.add_line(None, rule)?;
        }
        
        // Add from file
        if ignore_file.exists() {
            let error = builder.add(ignore_file);
            if let Some(e) = error {
                return Err(e);
            }
        }
        
        let gitignore = builder.build()?;
        Ok(Self { gitignore })
    }
}

pub struct IgnoreContext<'a> {
    pub path: &'a Path,
    pub is_dir: bool,
}

impl<'a> Specification<IgnoreContext<'a>> for IgnoreSpec {
    fn is_satisfied_by(&self, candidate: &IgnoreContext<'a>) -> bool {
        // match_path returns Match::Ignore, Match::None, or Match::Whitelist
        // We return true if it is matched by an ignore rule (i.e. should be ignored)
        self.gitignore.matched_path_or_any_parents(candidate.path, candidate.is_dir).is_ignore()
    }
}
