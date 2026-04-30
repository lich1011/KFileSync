pub trait DomainEvent: Send + Sync + 'static {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> &str;
}

pub trait EventBus: Send + Sync {
    fn publish(&self, event: Box<dyn DomainEvent>);
}
