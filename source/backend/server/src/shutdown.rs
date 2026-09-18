use tokio::sync::watch;

/// Sends a shutdown request to the HTTP server.
#[derive(Debug, Clone)]
pub struct ShutdownController {
    tx: watch::Sender<bool>,
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}

/// The asynchronous shutdown signal observed by the HTTP server.
#[derive(Debug, Clone)]
pub struct ShutdownSignal {
    rx: watch::Receiver<bool>,
}

impl ShutdownController {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self { tx }
    }

    pub fn request_shutdown(&self) -> bool {
        self.tx.send_replace(true);
        true
    }
}

impl ShutdownSignal {
    pub fn from_controller(controller: &ShutdownController) -> Self {
        Self {
            rx: controller.tx.subscribe(),
        }
    }

    pub async fn wait(&mut self) {
        if *self.rx.borrow_and_update() {
            return;
        }

        let _ = self.rx.wait_for(|requested| *requested).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_signal_is_initially_idle_and_then_completes() {
        let controller = ShutdownController::new();
        let mut signal = ShutdownSignal::from_controller(&controller);

        assert!(!*signal.rx.borrow_and_update());

        controller.request_shutdown();
        signal.wait().await;
    }
}
