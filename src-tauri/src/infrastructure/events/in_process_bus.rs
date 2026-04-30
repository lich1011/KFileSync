use tokio::sync::broadcast;
use std::sync::Arc;
use crate::domain::port::event_bus::{EventBus, DomainEvent};

pub struct InProcessEventBus {
    sender: broadcast::Sender<Arc<dyn DomainEvent>>,
}

impl Default for InProcessEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl InProcessEventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    /// 注册订阅者：返回一个接收端，调用方可在独立 Task 中循环监听事件。
    /// 设计说明：采用 broadcast channel 的多消费者模式，
    /// 任意数量的 Handler (如 AuditEventHandler, CascadeCleanupHandler) 均可独立订阅。
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<dyn DomainEvent>> {
        self.sender.subscribe()
    }
}

impl EventBus for InProcessEventBus {
    fn publish(&self, event: Box<dyn DomainEvent>) {
        if let Err(e) = self.sender.send(Arc::from(event)) {
            eprintln!("[EventBus] WARNING: Failed to deliver event — no active subscribers? Error: {}", e);
        }
    }
}
