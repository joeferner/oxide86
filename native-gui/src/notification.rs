use std::time::Instant;

pub(crate) enum NotificationType {
    Success,
    Error,
}

pub(crate) struct Notification {
    pub message: String,
    pub notification_type: NotificationType,
    shown_at: Instant,
}

impl Notification {
    pub(crate) fn new(message: String, notification_type: NotificationType) -> Self {
        Self {
            message,
            notification_type,
            shown_at: Instant::now(),
        }
    }

    pub(crate) fn is_expired(&self) -> bool {
        let duration = match self.notification_type {
            NotificationType::Success => std::time::Duration::from_secs(3),
            NotificationType::Error => std::time::Duration::from_secs(6),
        };
        self.shown_at.elapsed() > duration
    }
}
