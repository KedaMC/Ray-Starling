//! The control socket: open when the game was started with `--control` or `--control-socket`.
use super::Game;
use horizon_client::{Result, control};

impl Game<'_> {
    pub(super) fn start_control_socket(&mut self) -> Result<()> {
        if let Some(path) = &self.options.control {
            // Asked for on the command line: failing to listen is an error.
            let server = control::Server::start(path)
                .map_err(|e| format!("control socket {}: {e}", path.display()))?;
            log::info!("Horizon: control socket {}", server.path().display());
            self.control = Some(server);
        }
        Ok(())
    }
}
