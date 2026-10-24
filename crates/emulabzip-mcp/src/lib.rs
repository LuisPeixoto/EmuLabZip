//! An [MCP](https://modelcontextprotocol.io) server over standard input and output, so AI
//! assistants can read and control Android emulators and iOS simulators with the same code the
//! app and the command line use.
//!
//! Start it with `emulabzip mcp`. Every tool takes an optional `device` (an id or a name), and
//! when only one device is running it can be left out.

mod args;
mod server;

pub use server::EmuLabServer;

use emulabzip_core::Lab;
use rmcp::ServiceExt;

/// Serves the tools on stdin and stdout until the client disconnects.
pub async fn serve(lab: Lab) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let service = EmuLabServer::new(lab).serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
