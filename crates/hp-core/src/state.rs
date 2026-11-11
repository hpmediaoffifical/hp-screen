//! Recording session state machine.
use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecState {
    Idle,
    Countdown,
    Recording,
    Paused,
    Finalizing,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    Start,
    CountdownDone,
    Cancel,
    Pause,
    Resume,
    Stop,
    Finalized,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InvalidTransition {
    pub state: RecState,
    pub command: Command,
}

/// Next state for `c` in state `s`; commands that do not apply are rejected.
pub fn transition(s: RecState, c: Command) -> Result<RecState, InvalidTransition> {
    use Command::*;
    use RecState::*;
    match (s, c) {
        (Idle, Start) => Ok(Countdown),
        (Countdown, CountdownDone) => Ok(Recording),
        (Countdown, Cancel) => Ok(Idle),
        (Recording, Pause) => Ok(Paused),
        (Paused, Resume) => Ok(Recording),
        (Recording | Paused, Stop) => Ok(Finalizing),
        (Finalizing, Finalized) => Ok(Idle),
        _ => Err(InvalidTransition { state: s, command: c }),
    }
}

/// What the start/stop hotkey does in state `s`.
pub fn toggle_command(s: RecState) -> Option<Command> {
    match s {
        RecState::Idle => Some(Command::Start),
        RecState::Countdown => Some(Command::Cancel),
        RecState::Recording | RecState::Paused => Some(Command::Stop),
        RecState::Finalizing => None,
    }
}

/// What the pause hotkey does in state `s`.
pub fn pause_toggle_command(s: RecState) -> Option<Command> {
    match s {
        RecState::Recording => Some(Command::Pause),
        RecState::Paused => Some(Command::Resume),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::Command::*;
    use super::RecState::*;
    use super::*;

    const STATES: [RecState; 5] = [Idle, Countdown, Recording, Paused, Finalizing];
    const COMMANDS: [Command; 7] = [Start, CountdownDone, Cancel, Pause, Resume, Stop, Finalized];
    const VALID: [(RecState, Command, RecState); 8] = [
        (Idle, Start, Countdown),
        (Countdown, CountdownDone, Recording),
        (Countdown, Cancel, Idle),
        (Recording, Pause, Paused),
        (Paused, Resume, Recording),
        (Recording, Stop, Finalizing),
        (Paused, Stop, Finalizing),
        (Finalizing, Finalized, Idle),
    ];

    #[test]
    fn every_state_command_pair_follows_the_table() {
        for s in STATES {
            for c in COMMANDS {
                let expected = VALID
                    .iter()
                    .find(|(vs, vc, _)| *vs == s && *vc == c)
                    .map(|(_, _, next)| *next)
                    .ok_or(InvalidTransition { state: s, command: c });
                assert_eq!(transition(s, c), expected, "{s:?} + {c:?}");
            }
        }
    }

    #[test]
    fn toggle_hotkey_starts_cancels_or_stops() {
        assert_eq!(toggle_command(Idle), Some(Start));
        assert_eq!(toggle_command(Countdown), Some(Cancel));
        assert_eq!(toggle_command(Recording), Some(Stop));
        assert_eq!(toggle_command(Paused), Some(Stop));
        assert_eq!(toggle_command(Finalizing), None);
    }

    #[test]
    fn pause_hotkey_only_acts_while_recording_or_paused() {
        assert_eq!(pause_toggle_command(Recording), Some(Pause));
        assert_eq!(pause_toggle_command(Paused), Some(Resume));
        assert_eq!(pause_toggle_command(Idle), None);
        assert_eq!(pause_toggle_command(Countdown), None);
        assert_eq!(pause_toggle_command(Finalizing), None);
    }

    #[test]
    fn hammering_the_toggle_hotkey_never_opens_a_second_session() {
        // Presses arrive faster than the session moves on: each press acts on the current state.
        let mut s = Idle;
        let mut starts = 0;
        for _ in 0..5 {
            if let Some(c) = toggle_command(s) {
                if c == Start {
                    starts += 1;
                }
                s = transition(s, c).unwrap();
            }
        }
        assert_eq!(starts, 3); // Idle→Countdown→Idle→Countdown→Idle→Countdown
        assert_eq!(s, Countdown);
        // While finalizing, the hotkey is ignored until the file is closed.
        assert_eq!(toggle_command(Finalizing), None);
        assert!(transition(Finalizing, Start).is_err());
    }
}
