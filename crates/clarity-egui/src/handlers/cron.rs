use std::time::Instant;

use crate::stores::CronStore;

/// Handles the cron list event.
#[allow(dead_code)]
pub fn on_cron_list(
    cron_store: &mut CronStore,
    tasks: Vec<clarity_core::background::cron::CronTask>,
) {
    cron_store.tasks = tasks;
    cron_store.last_refresh = Instant::now();
}
