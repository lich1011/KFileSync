pub trait DomainEvent: Send + Sync + 'static {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> &str;
    /// Extra structured fields (beyond aggregateId/eventType) the UI bridge
    /// should forward for this event, e.g. a PIN to display. Most events
    /// don't need this, hence the empty-object default.
    fn payload(&self) -> serde_json::Value {
        serde_json::json!({})
    }

}

pub trait EventBus: Send + Sync {
    fn publish(&self, event: Box<dyn DomainEvent>);
}
