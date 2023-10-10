use typhon_types::Event;

use futures_core::stream::Stream;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub enum Msg {
    Emit(Event),
    Listen(mpsc::Sender<Event>),
}

pub struct EventLogger {
    sender: mpsc::Sender<Msg>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl EventLogger {
    pub fn new() -> Self {
        use Msg::*;
        let (sender, mut receiver) = mpsc::channel(256);
        let handle = tokio::spawn(async move {
            let mut senders: Vec<mpsc::Sender<Event>> = Vec::new();
            while let Some(msg) = receiver.recv().await {
                match msg {
                    Emit(event) => {
                        let mut new_senders: Vec<mpsc::Sender<Event>> = Vec::new();
                        for sender in senders.drain(..) {
                            match sender.send(event.clone()).await {
                                Ok(()) => new_senders.push(sender),
                                Err(_) => (),
                            }
                        }
                        senders = new_senders;
                    }
                    Listen(sender) => senders.push(sender),
                }
            }
        });
        Self {
            sender,
            handle: Mutex::new(Some(handle)),
        }
    }

    pub async fn log(&self, event: Event) {
        let _ = self.sender.send(Msg::Emit(event)).await;
    }

    pub async fn listen(&self) -> Option<impl Stream<Item = Event>> {
        let (sender, mut receiver) = mpsc::channel(256);
        let _ = self.sender.send(Msg::Listen(sender)).await;
        Some(async_stream::stream! {
            while let Some(e) = receiver.recv().await {
                yield e;
            }
        })
    }

    pub async fn shutdown(&self) {
        let _ = self.handle.lock().await.take().map(|handle| handle.abort());
    }
}
