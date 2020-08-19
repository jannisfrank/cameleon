use std::{sync::Arc, time::Instant};

use async_std::{
    prelude::*,
    sync::{Receiver, Sender, Mutex},
};
use futures::channel::oneshot;

use super::{
    signal::StreamSignal,
    control_module::Timestamp,
};

// TODO: Implement stream module.
pub(super) struct StreamModule {
    ctrl_rx: Receiver<StreamSignal>,
    ack_tx: Sender<Vec<u8>>,
    enabled: bool,
    timestamp: Timestamp,
}

impl StreamModule {
    pub(super) fn new(ctrl_rx: Receiver<StreamSignal>, ack_tx: Sender<Vec<u8>>, timestamp: Timestamp)-> Self {
        Self {
            ctrl_rx,
            ack_tx,
            enabled: false,
            timestamp,
        }
    }

    pub(super) async fn run(mut self) {
        let mut completed = None;

        while let Some(signal) = self.ctrl_rx.next().await {
            match signal {
                StreamSignal::Enable => {
                    if self.enabled {
                        log::warn! {"receive event enable signal, but event module is already enabled"}
                    } else {
                        self.enabled = true;
                        log::info! {"event module is enabled"};
                    }
                }
                StreamSignal::Disable(_completed) => {
                    if self.enabled {
                        self.enabled = false;
                        log::info! {"event module is disenabled"};
                    } else {
                        log::warn! {"receive event disable signal, but event module is already disabled"}
                    }
                }
                StreamSignal::Shutdown(completed_tx) => {
                    completed = Some(completed_tx);
                    break;
                }
            }
        }

        if completed.is_none() {
            log::error!("stream module ends abnormally. cause: stream signal sender is dropped");
        }
    }
}
