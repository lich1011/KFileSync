pub mod model {
    pub mod device;
    pub mod pairing;
    pub mod file_entry;
    pub mod share;
    pub mod transfer;
}
pub mod port {
    pub mod repository;
    pub mod key_store;
    pub mod event_bus;
    pub mod discovery;
    pub mod audit_repo;
    pub mod transfer_repo;
    pub mod share_repo;
    pub mod file_index_repo;
    pub mod file_watcher;
    pub mod network;
}
pub mod event {
    pub mod identity;
    pub mod transfer;
    pub mod sharing;
    pub mod sync_events;
}
pub mod service {
    pub mod chunking;
    pub mod specification;
    pub mod policy_enforcer;
    pub mod conflict_resolver;
    pub mod sync_plan_generator;
    pub mod block_deduplicator;
}
pub mod error;
