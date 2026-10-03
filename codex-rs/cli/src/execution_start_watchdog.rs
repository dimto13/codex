use codex_app_server_protocol::ThreadRuntimeState;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WatchdogPolicy {
    pub max_observations: u32,
    pub nudge_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WatchdogAction {
    Observe,
    Nudge,
    Succeed,
    Fail,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum StartConfirmation {
    Running,
    StartNotConfirmed,
    NotFound,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchdogOutcome {
    pub correlation_id: String,
    pub status: StartConfirmation,
    pub nudges: u8,
}

#[derive(Debug)]
pub(crate) struct ExecutionStartWatchdog {
    correlation_id: String,
    policy: WatchdogPolicy,
    observations: u32,
    nudged: bool,
}

impl ExecutionStartWatchdog {
    pub(crate) fn new(correlation_id: String, policy: WatchdogPolicy) -> Self {
        Self {
            correlation_id,
            policy,
            observations: 0,
            nudged: false,
        }
    }

    pub(crate) fn observe(&mut self, state: ThreadRuntimeState, active_turn: bool) -> WatchdogAction {
        if state == ThreadRuntimeState::NotFound {
            return WatchdogAction::Fail;
        }
        if state == ThreadRuntimeState::Running && active_turn {
            return WatchdogAction::Succeed;
        }
        if state == ThreadRuntimeState::Queued {
            return WatchdogAction::Observe;
        }

        self.observations = self.observations.saturating_add(1);
        if self.observations < self.policy.max_observations {
            return WatchdogAction::Observe;
        }
        self.observations = 0;

        if self.policy.nudge_enabled && !self.nudged {
            self.nudged = true;
            WatchdogAction::Nudge
        } else {
            WatchdogAction::Fail
        }
    }

    pub(crate) fn outcome(&self, status: StartConfirmation) -> WatchdogOutcome {
        WatchdogOutcome {
            correlation_id: self.correlation_id.clone(),
            status,
            nudges: u8::from(self.nudged),
        }
    }
}

#[cfg(test)]
#[path = "execution_start_watchdog_tests.rs"]
mod tests;
