//

use network::Command as NetworkCommand;
use kore_common::ComunicateInfo;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::error;

#[derive(Clone)]
/// The Kore Client Service.
pub struct ClientService {
    network_sender: mpsc::Sender<NetworkCommand>,
    client_sender: mpsc::Sender<NetworkCommand>,
    cancellation_token: CancellationToken,
}

impl ClientService {
    /// Create a new Client Service.
    pub fn new(
        network_sender: mpsc::Sender<NetworkCommand>,
        cancellation_token: CancellationToken,
    ) -> Self {
        // Client service channel.
        let (client_sender, mut client_receiver) = mpsc::channel(100000);

        Self {
            network_sender,
            client_sender,
            cancellation_token,
        }
        .run(client_receiver)
    }

    pub fn sender(&self) -> mpsc::Sender<NetworkCommand> {
        self.client_sender.clone()
    }

    fn run(&mut self, mut client_receiver: mpsc::Receiver<NetworkCommand>) -> Self {
        let cancellation_token = self.cancellation_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(command) = client_receiver.recv() => {
                        // Handle the command.
                    }
                    _ = cancellation_token.cancelled() => {
                        break;
                    }
                }
            }
        });
        self.clone()
    }

}