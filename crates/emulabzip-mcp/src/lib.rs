//! An [MCP](https://modelcontextprotocol.io) server over standard input and output, so AI
//! assistants can read and control Android emulators and iOS simulators with the same code the
//! app and the command line use.
//!
//! Start it with `emulabzip mcp`. Every tool takes an optional `device` (an id or a name), and
//! when only one device is running it can be left out.

mod args;
mod hub;
mod policy;
mod prompts;
mod resources;
mod server;
mod watch;

pub use policy::Policy;
pub use server::EmuLabServer;

use emulabzip_core::Lab;
use rmcp::ServiceExt;

/// Serves the tools on stdin and stdout until the client disconnects.
pub async fn serve(lab: Lab, policy: Policy) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = EmuLabServer::with_policy(lab.clone(), policy);
    let hub = server.hub();
    let service = server.serve(rmcp::transport::stdio()).await?;
    // The client closing standard input ends the session, and so does Ctrl+C or a termination signal.
    tokio::select! {
        result = service.waiting() => { result?; }
        _ = tokio::signal::ctrl_c() => {}
    }
    // Whatever this session changed on a device (the proxy setting, above all) is put back.
    hub.shutdown(&lab).await;
    Ok(())
}
