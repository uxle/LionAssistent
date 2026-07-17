//! Scheduling (TCLIS SS4 "Automation": "Scheduled tasks"). Deliberately
//! a pure function of "given the current time, what's due" rather than
//! an active background timer -- easy to test, and it's up to whatever
//! long-running process embeds this (`app`, eventually) to poll `due()`
//! on its own schedule.

use common::{Id, Timestamp};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum Schedule {
    Once(Timestamp),
    Every(Duration),
}

#[derive(Debug, Clone)]
struct Entry {
    workflow_id: Id,
    schedule: Schedule,
    next_run: Timestamp,
}

#[derive(Debug, Default)]
pub struct Scheduler {
    entries: Vec<Entry>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedules a workflow. For `Every`, the *first* run happens after
    /// one interval elapses, not immediately -- the same convention as
    /// cron or `setInterval`, so "every hour" means "starting an hour
    /// from now", not "right now, and also every hour after".
    pub fn schedule(&mut self, workflow_id: Id, schedule: Schedule) {
        let next_run = match schedule {
            Schedule::Once(at) => at,
            Schedule::Every(interval) => common::now() + to_chrono(interval),
        };
        self.entries.push(Entry {
            workflow_id,
            schedule,
            next_run,
        });
    }

    pub fn unschedule(&mut self, workflow_id: Id) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.workflow_id != workflow_id);
        self.entries.len() != before
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Workflow ids due to run at or before `now`. One-shot schedules
    /// are removed after firing; recurring ones get their `next_run`
    /// advanced from `now` (not from the missed time, so a long gap
    /// -- e.g. the app was closed -- doesn't cause a burst of
    /// back-to-back catch-up runs).
    pub fn due(&mut self, now: Timestamp) -> Vec<Id> {
        let mut due_ids = Vec::new();
        let mut remaining = Vec::with_capacity(self.entries.len());

        for mut entry in std::mem::take(&mut self.entries) {
            if entry.next_run > now {
                remaining.push(entry);
                continue;
            }
            due_ids.push(entry.workflow_id);
            if let Schedule::Every(interval) = entry.schedule {
                entry.next_run = now + to_chrono(interval);
                remaining.push(entry);
            }
        }

        self.entries = remaining;
        due_ids
    }
}

fn to_chrono(d: Duration) -> chrono::Duration {
    chrono::Duration::from_std(d).unwrap_or(chrono::Duration::zero())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_one_shot_schedule_fires_once_then_is_gone() {
        let mut scheduler = Scheduler::new();
        let id = Id::new();
        let at = common::now();
        scheduler.schedule(id, Schedule::Once(at));

        assert_eq!(scheduler.due(at), vec![id]);
        assert!(scheduler.is_empty());
        assert!(scheduler.due(at).is_empty());
    }

    #[test]
    fn a_future_one_shot_schedule_is_not_due_yet() {
        let mut scheduler = Scheduler::new();
        let far_future = common::now() + chrono::Duration::days(1);
        scheduler.schedule(Id::new(), Schedule::Once(far_future));

        assert!(scheduler.due(common::now()).is_empty());
        assert_eq!(scheduler.len(), 1);
    }

    #[test]
    fn a_recurring_schedule_does_not_fire_immediately() {
        // A recurring schedule's first run happens after one interval
        // has elapsed, not the moment it's scheduled -- the same
        // convention as cron or `setInterval`.
        let mut scheduler = Scheduler::new();
        let id = Id::new();
        let start = common::now();
        scheduler.schedule(id, Schedule::Every(Duration::from_secs(60)));

        assert!(scheduler.due(start).is_empty());
        assert_eq!(scheduler.len(), 1);
    }

    #[test]
    fn a_recurring_schedule_stays_scheduled_after_firing() {
        let mut scheduler = Scheduler::new();
        let id = Id::new();
        let start = common::now();
        scheduler.schedule(id, Schedule::Every(Duration::from_secs(60)));

        let first_fire = start + chrono::Duration::seconds(61);
        assert_eq!(scheduler.due(first_fire), vec![id]);
        assert_eq!(scheduler.len(), 1); // still scheduled -- it's recurring

        // Not due again immediately after firing.
        assert!(scheduler.due(first_fire).is_empty());
        // Due again after a second interval.
        assert_eq!(
            scheduler.due(first_fire + chrono::Duration::seconds(61)),
            vec![id]
        );
    }

    #[test]
    fn unschedule_removes_a_pending_entry() {
        let mut scheduler = Scheduler::new();
        let id = Id::new();
        scheduler.schedule(
            id,
            Schedule::Once(common::now() + chrono::Duration::days(1)),
        );
        assert!(scheduler.unschedule(id));
        assert!(scheduler.is_empty());
    }

    #[test]
    fn unscheduling_an_unknown_workflow_returns_false() {
        assert!(!Scheduler::new().unschedule(Id::new()));
    }
}
