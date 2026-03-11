pub mod gateway;
pub mod provider;

use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidTransition {
    pub state: &'static str,
    pub trigger: &'static str,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot '{}' while in state '{}'",
            self.trigger, self.state
        )
    }
}

impl std::error::Error for InvalidTransition {}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn display_shows_trigger_and_state() {
        let error = InvalidTransition {
            state: "Disconnected",
            trigger: "disconnect",
        };
        assert_eq!(
            error.to_string(),
            "cannot 'disconnect' while in state 'Disconnected'"
        );
    }
}
