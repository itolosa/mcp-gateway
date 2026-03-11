use super::InvalidTransition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayState {
    Stopped,
    Initializing,
    Listening,
    ShuttingDown,
}

impl GatewayState {
    fn name(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Initializing => "Initializing",
            Self::Listening => "Listening",
            Self::ShuttingDown => "ShuttingDown",
        }
    }

    pub fn start(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Stopped => Ok(Self::Initializing),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "start",
            }),
        }
    }

    pub fn init_success(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Initializing => Ok(Self::Listening),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "init_success",
            }),
        }
    }

    pub fn init_failure(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Initializing => Ok(Self::Stopped),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "init_failure",
            }),
        }
    }

    pub fn stop(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Listening => Ok(Self::ShuttingDown),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "stop",
            }),
        }
    }

    pub fn shutdown_complete(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::ShuttingDown => Ok(Self::Stopped),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "shutdown_complete",
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_stopped() {
        let state = GatewayState::Stopped;
        assert_eq!(state, GatewayState::Stopped);
    }

    // -- Valid transitions --

    #[test]
    fn should_transition_to_initializing_when_started() {
        let state = GatewayState::Stopped.start().unwrap();
        assert_eq!(state, GatewayState::Initializing);
    }

    #[test]
    fn should_transition_to_listening_when_init_succeeds() {
        let state = GatewayState::Initializing.init_success().unwrap();
        assert_eq!(state, GatewayState::Listening);
    }

    #[test]
    fn should_transition_to_stopped_when_init_fails() {
        let state = GatewayState::Initializing.init_failure().unwrap();
        assert_eq!(state, GatewayState::Stopped);
    }

    #[test]
    fn should_transition_to_shutting_down_when_stopped() {
        let state = GatewayState::Listening.stop().unwrap();
        assert_eq!(state, GatewayState::ShuttingDown);
    }

    #[test]
    fn should_transition_to_stopped_when_shutdown_completes() {
        let state = GatewayState::ShuttingDown.shutdown_complete().unwrap();
        assert_eq!(state, GatewayState::Stopped);
    }

    // -- Invalid transitions --

    #[test]
    fn should_reject_start_when_listening() {
        let result = GatewayState::Listening.start();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Listening",
                trigger: "start"
            }
        );
    }

    #[test]
    fn should_reject_stop_when_stopped() {
        let result = GatewayState::Stopped.stop();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Stopped",
                trigger: "stop"
            }
        );
    }

    #[test]
    fn should_reject_init_success_when_stopped() {
        let result = GatewayState::Stopped.init_success();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Stopped",
                trigger: "init_success"
            }
        );
    }

    #[test]
    fn should_reject_init_failure_when_listening() {
        let result = GatewayState::Listening.init_failure();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Listening",
                trigger: "init_failure"
            }
        );
    }

    #[test]
    fn should_reject_shutdown_complete_when_listening() {
        let result = GatewayState::Listening.shutdown_complete();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Listening",
                trigger: "shutdown_complete"
            }
        );
    }

    #[test]
    fn should_reject_stop_when_initializing() {
        let result = GatewayState::Initializing.stop();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Initializing",
                trigger: "stop"
            }
        );
    }

    #[test]
    fn should_reject_start_when_shutting_down() {
        let result = GatewayState::ShuttingDown.start();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "ShuttingDown",
                trigger: "start"
            }
        );
    }

    // -- Full lifecycles --

    #[test]
    fn full_lifecycle_start_listen_stop() {
        let state = GatewayState::Stopped;
        let state = state.start().unwrap();
        let state = state.init_success().unwrap();
        let state = state.stop().unwrap();
        let state = state.shutdown_complete().unwrap();
        assert_eq!(state, GatewayState::Stopped);
    }

    #[test]
    fn restart_after_init_failure() {
        let state = GatewayState::Stopped;
        let state = state.start().unwrap();
        let state = state.init_failure().unwrap();
        let state = state.start().unwrap();
        let state = state.init_success().unwrap();
        assert_eq!(state, GatewayState::Listening);
    }
}
