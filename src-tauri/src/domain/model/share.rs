use serde::{Deserialize, Serialize};
use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ShareId(pub String);

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum SyncMode {
    TwoWay,
    SendOnly,
    ReceiveOnly,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum ShareStatus {
    Active,
    Paused,
    Error(String),
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum SharePermission {
    ReadOnly,    // can_pull=true, can_push=false
    ReadWrite,   // can_pull=true, can_push=true
    SendOnly,    // can_pull=false, can_push=true
    ReceiveOnly, // can_pull=true, can_push=false, local changes reverted
}

impl SharePermission {
    pub fn can_push(&self) -> bool {
        matches!(self, Self::ReadWrite | Self::SendOnly)
    }
    
    pub fn can_pull(&self) -> bool {
        matches!(self, Self::ReadWrite | Self::ReadOnly | Self::ReceiveOnly)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShareMember {
    pub device_id: DeviceId,
    pub permission: SharePermission,
    pub authorized_by: DeviceId,
    pub authorized_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Share {
    pub share_id: ShareId,
    pub share_name: String,
    pub local_path: String,
    pub sync_mode: SyncMode,
    pub status: ShareStatus,
    pub members: Vec<ShareMember>,
    pub created_by: DeviceId,
    pub created_at: u64,
}

impl Share {
    pub fn create(
        share_id: ShareId,
        share_name: String,
        local_path: String,
        sync_mode: SyncMode,
        created_by: DeviceId,
    ) -> Self {
        let created_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let members = vec![ShareMember {
            device_id: created_by.clone(),
            permission: SharePermission::ReadWrite,
            authorized_by: created_by.clone(),
            authorized_at: created_at,
        }];

        Self {
            share_id,
            share_name,
            local_path,
            sync_mode,
            status: ShareStatus::Active,
            members,
            created_by,
            created_at,
        }
    }

    pub fn authorize_member(
        mut self,
        device_id: DeviceId,
        permission: SharePermission,
        authorized_by: DeviceId,
    ) -> Result<Self, DomainError> {
        if self.has_member(&device_id) {
            return Err(DomainError::BusinessRuleViolation(
                format!("Device {} is already a member", device_id.0)
            ));
        }

        let authorized_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        self.members.push(ShareMember {
            device_id,
            permission,
            authorized_by,
            authorized_at,
        });
        Ok(self)
    }

    pub fn remove_member(mut self, device_id: &DeviceId) -> Result<Self, DomainError> {
        if &self.created_by == device_id {
            return Err(DomainError::BusinessRuleViolation(
                "Cannot remove the creator of the share".to_string()
            ));
        }
        
        let initial_len = self.members.len();
        self.members.retain(|m| &m.device_id != device_id);
        
        if self.members.len() == initial_len {
            return Err(DomainError::BusinessRuleViolation(
                format!("Device {} is not a member", device_id.0)
            ));
        }
        
        Ok(self)
    }

    pub fn update_permission(
        mut self,
        device_id: &DeviceId,
        new_permission: SharePermission,
    ) -> Result<Self, DomainError> {
        if let Some(member) = self.members.iter_mut().find(|m| &m.device_id == device_id) {
            member.permission = new_permission;
            Ok(self)
        } else {
            Err(DomainError::BusinessRuleViolation(
                format!("Device {} is not a member", device_id.0)
            ))
        }
    }

    pub fn pause(mut self) -> Result<Self, DomainError> {
        if !matches!(self.status, ShareStatus::Active) {
            return Err(DomainError::InvalidStateTransition("Share must be Active to pause"));
        }
        self.status = ShareStatus::Paused;
        Ok(self)
    }

    pub fn resume(mut self) -> Result<Self, DomainError> {
        if !matches!(self.status, ShareStatus::Paused) {
            return Err(DomainError::InvalidStateTransition("Share must be Paused to resume"));
        }
        self.status = ShareStatus::Active;
        Ok(self)
    }

    pub fn has_member(&self, device_id: &DeviceId) -> bool {
        self.members.iter().any(|m| &m.device_id == device_id)
    }

    pub fn get_permission(&self, device_id: &DeviceId) -> Option<SharePermission> {
        self.members
            .iter()
            .find(|m| &m.device_id == device_id)
            .map(|m| m.permission.clone())
    }
}
