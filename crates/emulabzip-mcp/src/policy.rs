//! What an assistant may change. Tools that destroy or rewrite data are not offered at all unless the server was started
//! with `--allow-writes`, and then each call asks the person, through the client, before it does anything.

use rmcp::RoleServer;
use rmcp::service::{ElicitationError, RequestContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::server::EmuLabServer;

/// How the server treats tools that change or destroy data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Policy {
    /// Offer the tools that wipe, delete, restore or rewrite. Off by default.
    pub allow_writes: bool,
    /// Skip the confirmation. Only for unattended use, such as a CI job on a disposable emulator.
    pub no_confirm: bool,
}

/// The answer to a confirmation.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Confirm {
    /// Tick to allow it.
    pub confirm: bool,
}

rmcp::elicit_safe!(Confirm);

/// The tools that need `--allow-writes`. `destructive_hint` is true on exactly these.
pub const GATED: &[&str] = &[
    "restore_state",
    "wipe_device",
    "delete_device",
    "load_emulator_snapshot",
    "delete_emulator_snapshot",
    "write_file",
    "webview_eval",
    "delete_session",
    "remove_flow",
    "remove_mapping",
    "forget_link",
    "visual_update",
];

/// Tools no assistant is offered: approving code to run, and running arbitrary commands, stay with the person.
pub const NEVER: &[&str] = &["approve_script", "enable_plugin", "ci"];

impl EmuLabServer {
    /// Asks the person to confirm `what`, in the words the client will show. Returns why not when they decline, cancel,
    /// or the client cannot ask.
    pub(crate) async fn authorize(&self, context: &RequestContext<RoleServer>, what: &str) -> Result<(), String> {
        if self.policy.no_confirm {
            return Ok(());
        }
        match context.peer.elicit::<Confirm>(what.to_string()).await {
            Ok(Some(answer)) if answer.confirm => Ok(()),
            Ok(_) | Err(ElicitationError::UserDeclined) | Err(ElicitationError::UserCancelled) => {
                Err("The person did not confirm, so nothing was changed.".to_string())
            }
            Err(ElicitationError::CapabilityNotSupported) => {
                Err("This client cannot ask the person to confirm, so nothing was changed. Start the server with \
                 `--no-confirm` only for unattended use."
                    .to_string())
            }
            Err(e) => Err(format!("The confirmation failed, so nothing was changed: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_gated_twice_or_also_never_offered() {
        let mut seen = std::collections::HashSet::new();
        for name in GATED {
            assert!(seen.insert(*name), "{name} is listed twice");
            assert!(!NEVER.contains(name), "{name} cannot be both gated and never offered");
        }
    }

    #[test]
    fn writes_are_off_unless_asked_for() {
        let policy = Policy::default();
        assert!(!policy.allow_writes && !policy.no_confirm);
    }
}
