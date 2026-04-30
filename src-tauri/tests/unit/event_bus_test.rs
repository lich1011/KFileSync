use kfilesync_lib::infrastructure::events::in_process_bus::InProcessEventBus;
use kfilesync_lib::domain::port::event_bus::{EventBus, DomainEvent};

struct TestEvent {
    pub id: String,
}

impl DomainEvent for TestEvent {
    fn event_type(&self) -> &str { "TestEvent" }
    fn aggregate_id(&self) -> &str { &self.id }
}

#[tokio::test]
async fn test_event_bus_publish_subscribe() {
    let bus = InProcessEventBus::new();
    let mut rx = bus.subscribe();

    bus.publish(Box::new(TestEvent { id: "agg-001".to_string() }));

    // Allow brief time for async delivery
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let received = rx.try_recv();
    assert!(received.is_ok(), "Should receive the event");
    let event = received.unwrap();
    assert_eq!(event.event_type(), "TestEvent");
    assert_eq!(event.aggregate_id(), "agg-001");
}

#[tokio::test]
async fn test_event_bus_multiple_subscribers() {
    let bus = InProcessEventBus::new();
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    bus.publish(Box::new(TestEvent { id: "agg-002".to_string() }));

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    assert!(rx1.try_recv().is_ok(), "Subscriber 1 should receive event");
    assert!(rx2.try_recv().is_ok(), "Subscriber 2 should receive event");
}
