//! When HP Screen may look for and install a new release. The download itself lives in the app.
use crate::state::RecState;

/// First automatic check, a little after start so it does not slow the app down.
pub const FIRST_CHECK_DELAY_SECS: u64 = 20;
/// Automatic checks after the first one.
pub const CHECK_INTERVAL_SECS: u64 = 6 * 60 * 60;

/// Installing closes HP Screen, so it waits until nothing is being recorded or saved.
pub fn may_install(state: RecState) -> bool {
    state == RecState::Idle
}

/// Whether an automatic check is due, `secs_since_last` seconds after the previous one
/// (`None` when there has been none since the app started).
pub fn check_due(auto_update: bool, secs_since_start: u64, secs_since_last: Option<u64>) -> bool {
    if !auto_update {
        return false;
    }
    match secs_since_last {
        None => secs_since_start >= FIRST_CHECK_DELAY_SECS,
        Some(since) => since >= CHECK_INTERVAL_SECS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_only_when_nothing_is_recording_or_saving() {
        assert!(may_install(RecState::Idle));
        for busy in [RecState::Countdown, RecState::Recording, RecState::Paused, RecState::Finalizing] {
            assert!(!may_install(busy), "{busy:?}");
        }
    }

    #[test]
    fn automatic_checks_wait_a_little_after_start_then_repeat_every_few_hours() {
        assert!(!check_due(true, 5, None));
        assert!(check_due(true, FIRST_CHECK_DELAY_SECS, None));
        assert!(!check_due(true, 100_000, Some(60)));
        assert!(check_due(true, 100_000, Some(CHECK_INTERVAL_SECS)));
    }

    #[test]
    fn no_automatic_checks_when_auto_update_is_off() {
        assert!(!check_due(false, 100_000, None));
        assert!(!check_due(false, 100_000, Some(CHECK_INTERVAL_SECS * 10)));
    }
}
