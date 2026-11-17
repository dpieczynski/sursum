// SPDX-License-Identifier: GPL-3.0-or-later
//! Talking to the display about its power (ADR 0012), as pure text in and out; the server owns
//! the network connection.
//!
//! PJLink class 1, from the JBMIA "PJLink Specifications" version 1.04 (2013-12-10), sections 4.1,
//! 4.2 and 5: TCP port 4352; every line ends with CR; on connect the projector greets with
//! `PJLINK 0` (no password) or `PJLINK 1 <8 lowercase hex digits>`; with a password, the first
//! command is prefixed by the 32-character MD5 of `<seed><password>`; a wrong password is
//! answered with `PJLINK ERRA`.
//!
//! HDMI-CEC (the Pi, through the kernel's CEC device): the TV answers "Give Device Power Status"
//! with "Report Power Status", whose one-byte operand [`cec_power_status`] reads (HDMI 1.4 CEC,
//! "Power Status" operand: 0 on, 1 standby, 2 standby to on, 3 on to standby).

use md5::{Digest as _, Md5};
use sursum_protocol::{DisplayPowerState, PowerAction};

/// The PJLink TCP port.
pub const PJLINK_PORT: u16 = 4352;

/// Asks for the power state.
pub const POWER_QUERY: &str = "%1POWR ?\r";

/// What a projector says right after the connection opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Greeting {
    /// No password set: commands can be sent as they are.
    Open,
    /// A password is set: prefix the first command with [`auth_prefix`] of this seed.
    Challenge { seed: String },
}

/// Why a PJLink exchange did not give an answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PjLinkError {
    #[error("the projector rejected the password")]
    AuthFailed,
    #[error("the projector does not know the command")]
    UndefinedCommand,
    #[error("the projector rejected the parameter")]
    BadParameter,
    /// `ERR3`, "unavailable time": e.g. cooling down after standby.
    #[error("the projector cannot do that right now")]
    Busy,
    /// `ERR4`.
    #[error("the projector reports a failure")]
    DisplayFailure,
    #[error("unexpected answer {0:?}")]
    Unexpected(String),
}

/// An answer to a power command or query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerReply {
    /// The command was accepted.
    Accepted,
    /// The answer to [`POWER_QUERY`].
    State(DisplayPowerState),
}

/// Reads the greeting line (without or with its CR).
pub fn parse_greeting(line: &str) -> Result<Greeting, PjLinkError> {
    let line = line.trim_end_matches(['\r', '\n']);
    let mut words = line.split(' ');
    if !words.next().is_some_and(|word| word.eq_ignore_ascii_case("PJLINK")) {
        return Err(PjLinkError::Unexpected(line.to_owned()));
    }
    match (words.next(), words.next(), words.next()) {
        (Some("0"), None, None) => Ok(Greeting::Open),
        (Some("1"), Some(seed), None) if seed.len() == 8 && seed.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Ok(Greeting::Challenge { seed: seed.to_owned() })
        }
        (Some(word), None, None) if word.eq_ignore_ascii_case("ERRA") => Err(PjLinkError::AuthFailed),
        _ => Err(PjLinkError::Unexpected(line.to_owned())),
    }
}

/// The 32 lowercase hex characters to put in front of the first command after a challenge.
pub fn auth_prefix(seed: &str, password: &str) -> String {
    let digest = Md5::digest(format!("{seed}{password}").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The command line for a power action.
pub fn power_command(action: PowerAction) -> &'static str {
    match action {
        PowerAction::On => "%1POWR 1\r",
        PowerAction::Standby => "%1POWR 0\r",
    }
}

/// Reads the answer to a power command or query.
pub fn parse_power_reply(line: &str) -> Result<PowerReply, PjLinkError> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.eq_ignore_ascii_case("PJLINK ERRA") {
        return Err(PjLinkError::AuthFailed);
    }
    let Some(value) = line.get(..7).filter(|head| head.eq_ignore_ascii_case("%1POWR=")).map(|_| &line[7..])
    else {
        return Err(PjLinkError::Unexpected(line.to_owned()));
    };
    match value.to_ascii_uppercase().as_str() {
        "OK" => Ok(PowerReply::Accepted),
        "0" => Ok(PowerReply::State(DisplayPowerState::Standby)),
        "1" => Ok(PowerReply::State(DisplayPowerState::On)),
        "2" => Ok(PowerReply::State(DisplayPowerState::CoolingDown)),
        "3" => Ok(PowerReply::State(DisplayPowerState::WarmingUp)),
        "ERR1" => Err(PjLinkError::UndefinedCommand),
        "ERR2" => Err(PjLinkError::BadParameter),
        "ERR3" => Err(PjLinkError::Busy),
        "ERR4" => Err(PjLinkError::DisplayFailure),
        _ => Err(PjLinkError::Unexpected(line.to_owned())),
    }
}

/// The operand of a CEC "Report Power Status" message as a power state; `None` for values the
/// standard does not define (a TV may send anything).
pub fn cec_power_status(operand: u8) -> Option<DisplayPowerState> {
    match operand {
        0 => Some(DisplayPowerState::On),
        1 => Some(DisplayPowerState::Standby),
        2 => Some(DisplayPowerState::WarmingUp),
        3 => Some(DisplayPowerState::CoolingDown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cec_power_status_follows_the_standard() {
        assert_eq!(cec_power_status(0), Some(DisplayPowerState::On));
        assert_eq!(cec_power_status(1), Some(DisplayPowerState::Standby));
        assert_eq!(cec_power_status(2), Some(DisplayPowerState::WarmingUp));
        assert_eq!(cec_power_status(3), Some(DisplayPowerState::CoolingDown));
        assert_eq!(cec_power_status(4), None);
    }

    #[test]
    fn the_specification_example_gives_the_specification_digest() {
        // PJLink 1.04, section 5.1: password "JBMIAProjectorLink", random number 0x498e4a67.
        assert_eq!(auth_prefix("498e4a67", "JBMIAProjectorLink"), "5d8409bc1c3fa39749434aa3a5c38682");
    }

    #[test]
    fn greetings() {
        assert_eq!(parse_greeting("PJLINK 0\r"), Ok(Greeting::Open));
        assert_eq!(
            parse_greeting("PJLINK 1 498e4a67\r"),
            Ok(Greeting::Challenge { seed: "498e4a67".into() })
        );
        assert_eq!(parse_greeting("PJLINK ERRA\r"), Err(PjLinkError::AuthFailed));
        assert!(matches!(parse_greeting("PJLINK 1 xyz\r"), Err(PjLinkError::Unexpected(_))));
        assert!(matches!(parse_greeting("HTTP/1.1 400\r"), Err(PjLinkError::Unexpected(_))));
    }

    #[test]
    fn power_replies() {
        assert_eq!(power_command(PowerAction::On), "%1POWR 1\r");
        assert_eq!(power_command(PowerAction::Standby), "%1POWR 0\r");
        assert_eq!(parse_power_reply("%1POWR=OK\r"), Ok(PowerReply::Accepted));
        assert_eq!(parse_power_reply("%1powr=ok"), Ok(PowerReply::Accepted), "case-insensitive");
        let states = [
            ("0", DisplayPowerState::Standby),
            ("1", DisplayPowerState::On),
            ("2", DisplayPowerState::CoolingDown),
            ("3", DisplayPowerState::WarmingUp),
        ];
        for (code, state) in states {
            assert_eq!(parse_power_reply(&format!("%1POWR={code}\r")), Ok(PowerReply::State(state)));
        }
        assert_eq!(parse_power_reply("%1POWR=ERR3\r"), Err(PjLinkError::Busy));
        assert_eq!(parse_power_reply("%1POWR=ERR4\r"), Err(PjLinkError::DisplayFailure));
        assert_eq!(parse_power_reply("PJLINK ERRA\r"), Err(PjLinkError::AuthFailed));
        assert!(matches!(parse_power_reply("%1INPT=OK"), Err(PjLinkError::Unexpected(_))));
        assert!(matches!(parse_power_reply("%1"), Err(PjLinkError::Unexpected(_))));
    }
}
