use super::InvalidTransition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderState {
    Disconnected,
    Connecting,
    Unauthenticated,
    Authenticating,
    Authenticated,
    Connected,
    Disconnecting,
}

impl ProviderState {
    fn name(self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Unauthenticated => "Unauthenticated",
            Self::Authenticating => "Authenticating",
            Self::Authenticated => "Authenticated",
            Self::Connected => "Connected",
            Self::Disconnecting => "Disconnecting",
        }
    }

    pub fn initiate_connection(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Disconnected => Ok(Self::Connecting),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "initiate_connection",
            }),
        }
    }

    pub fn connection_ready(self, auth_required: bool) -> Result<Self, InvalidTransition> {
        match self {
            Self::Connecting if auth_required => Ok(Self::Unauthenticated),
            Self::Connecting => Ok(Self::Connected),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "connection_ready",
            }),
        }
    }

    pub fn begin_auth(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Unauthenticated => Ok(Self::Authenticating),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "begin_auth",
            }),
        }
    }

    pub fn auth_success(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Authenticating => Ok(Self::Authenticated),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "auth_success",
            }),
        }
    }

    pub fn auth_failure(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Authenticating => Ok(Self::Unauthenticated),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "auth_failure",
            }),
        }
    }

    pub fn auth_complete(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Authenticated => Ok(Self::Connected),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "auth_complete",
            }),
        }
    }

    pub fn token_expired(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Connected => Ok(Self::Unauthenticated),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "token_expired",
            }),
        }
    }

    pub fn disconnect(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Connected => Ok(Self::Disconnecting),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "disconnect",
            }),
        }
    }

    pub fn cleanup_complete(self) -> Result<Self, InvalidTransition> {
        match self {
            Self::Disconnecting => Ok(Self::Disconnected),
            _ => Err(InvalidTransition {
                state: self.name(),
                trigger: "cleanup_complete",
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_disconnected() {
        let state = ProviderState::Disconnected;
        assert_eq!(state, ProviderState::Disconnected);
    }

    // -- Valid transitions --

    #[test]
    fn should_transition_to_connecting_when_connection_initiated() {
        let state = ProviderState::Disconnected.initiate_connection().unwrap();
        assert_eq!(state, ProviderState::Connecting);
    }

    #[test]
    fn should_transition_to_connected_when_no_auth_required() {
        let state = ProviderState::Connecting.connection_ready(false).unwrap();
        assert_eq!(state, ProviderState::Connected);
    }

    #[test]
    fn should_transition_to_unauthenticated_when_auth_required() {
        let state = ProviderState::Connecting.connection_ready(true).unwrap();
        assert_eq!(state, ProviderState::Unauthenticated);
    }

    #[test]
    fn should_transition_to_authenticating_when_auth_begins() {
        let state = ProviderState::Unauthenticated.begin_auth().unwrap();
        assert_eq!(state, ProviderState::Authenticating);
    }

    #[test]
    fn should_transition_to_authenticated_on_auth_success() {
        let state = ProviderState::Authenticating.auth_success().unwrap();
        assert_eq!(state, ProviderState::Authenticated);
    }

    #[test]
    fn should_transition_to_unauthenticated_on_auth_failure() {
        let state = ProviderState::Authenticating.auth_failure().unwrap();
        assert_eq!(state, ProviderState::Unauthenticated);
    }

    #[test]
    fn should_transition_to_connected_when_auth_complete() {
        let state = ProviderState::Authenticated.auth_complete().unwrap();
        assert_eq!(state, ProviderState::Connected);
    }

    #[test]
    fn should_transition_to_unauthenticated_on_token_expired() {
        let state = ProviderState::Connected.token_expired().unwrap();
        assert_eq!(state, ProviderState::Unauthenticated);
    }

    #[test]
    fn should_transition_to_disconnecting_on_disconnect() {
        let state = ProviderState::Connected.disconnect().unwrap();
        assert_eq!(state, ProviderState::Disconnecting);
    }

    #[test]
    fn should_transition_to_disconnected_on_cleanup_complete() {
        let state = ProviderState::Disconnecting.cleanup_complete().unwrap();
        assert_eq!(state, ProviderState::Disconnected);
    }

    // -- Invalid transitions --

    #[test]
    fn should_reject_connection_when_already_connected() {
        let result = ProviderState::Connected.initiate_connection();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Connected",
                trigger: "initiate_connection"
            }
        );
    }

    #[test]
    fn should_reject_connection_ready_when_disconnected() {
        let result = ProviderState::Disconnected.connection_ready(false);
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Disconnected",
                trigger: "connection_ready"
            }
        );
    }

    #[test]
    fn should_reject_begin_auth_when_connected() {
        let result = ProviderState::Connected.begin_auth();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Connected",
                trigger: "begin_auth"
            }
        );
    }

    #[test]
    fn should_reject_auth_success_when_unauthenticated() {
        let result = ProviderState::Unauthenticated.auth_success();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Unauthenticated",
                trigger: "auth_success"
            }
        );
    }

    #[test]
    fn should_reject_auth_failure_when_connected() {
        let result = ProviderState::Connected.auth_failure();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Connected",
                trigger: "auth_failure"
            }
        );
    }

    #[test]
    fn should_reject_auth_complete_when_connecting() {
        let result = ProviderState::Connecting.auth_complete();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Connecting",
                trigger: "auth_complete"
            }
        );
    }

    #[test]
    fn should_reject_token_expired_when_disconnected() {
        let result = ProviderState::Disconnected.token_expired();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Disconnected",
                trigger: "token_expired"
            }
        );
    }

    #[test]
    fn should_reject_disconnect_when_disconnected() {
        let result = ProviderState::Disconnected.disconnect();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Disconnected",
                trigger: "disconnect"
            }
        );
    }

    #[test]
    fn should_reject_cleanup_when_connected() {
        let result = ProviderState::Connected.cleanup_complete();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Connected",
                trigger: "cleanup_complete"
            }
        );
    }

    #[test]
    fn should_reject_disconnect_when_authenticating() {
        let result = ProviderState::Authenticating.disconnect();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Authenticating",
                trigger: "disconnect"
            }
        );
    }

    #[test]
    fn should_reject_disconnect_when_authenticated() {
        let result = ProviderState::Authenticated.disconnect();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Authenticated",
                trigger: "disconnect"
            }
        );
    }

    #[test]
    fn should_reject_begin_auth_when_disconnecting() {
        let result = ProviderState::Disconnecting.begin_auth();
        assert_eq!(
            result.unwrap_err(),
            InvalidTransition {
                state: "Disconnecting",
                trigger: "begin_auth"
            }
        );
    }

    // -- Full lifecycles --

    #[test]
    fn full_lifecycle_without_auth() {
        let state = ProviderState::Disconnected;
        let state = state.initiate_connection().unwrap();
        let state = state.connection_ready(false).unwrap();
        let state = state.disconnect().unwrap();
        let state = state.cleanup_complete().unwrap();
        assert_eq!(state, ProviderState::Disconnected);
    }

    #[test]
    fn full_lifecycle_with_auth() {
        let state = ProviderState::Disconnected;
        let state = state.initiate_connection().unwrap();
        let state = state.connection_ready(true).unwrap();
        let state = state.begin_auth().unwrap();
        let state = state.auth_success().unwrap();
        let state = state.auth_complete().unwrap();
        let state = state.disconnect().unwrap();
        let state = state.cleanup_complete().unwrap();
        assert_eq!(state, ProviderState::Disconnected);
    }

    #[test]
    fn re_auth_after_token_expired() {
        let state = ProviderState::Connected;
        let state = state.token_expired().unwrap();
        let state = state.begin_auth().unwrap();
        let state = state.auth_success().unwrap();
        let state = state.auth_complete().unwrap();
        assert_eq!(state, ProviderState::Connected);
    }

    #[test]
    fn retry_after_auth_failure() {
        let state = ProviderState::Unauthenticated;
        let state = state.begin_auth().unwrap();
        let state = state.auth_failure().unwrap();
        assert_eq!(state, ProviderState::Unauthenticated);
        let state = state.begin_auth().unwrap();
        let state = state.auth_success().unwrap();
        let state = state.auth_complete().unwrap();
        assert_eq!(state, ProviderState::Connected);
    }
}
