use crate::connection;
use crate::error::Error;
use crate::models::*;
use crate::schema::logs::dsl::*;
use diesel::prelude::*;
use typhon_types::*;

fn get_log_type(log: &handles::Log) -> &'static str {
    use handles::Log::*;
    match log {
        Begin(_) => "begin",
        End(_) => "end",
        Eval(_) => "eval",
    }
}

impl Log {
    pub async fn new(log_handle: handles::Log, stderr: String) -> Result<Self, Error> {
        let ty = get_log_type(&log_handle);
        let mut new_log = NewLog {
            log_evaluation: None,
            log_job: None,
            log_stderr: &stderr,
            log_type: &ty,
        };
        use handles::Log::*;
        match log_handle {
            Begin(h) | End(h) => {
                let job = Job::get(&h).await?;
                new_log.log_job = Some(job.job_id);
            }
            Eval(h) => {
                let evaluation = Evaluation::get(&h).await?;
                new_log.log_evaluation = Some(evaluation.evaluation_id);
            }
        };
        let mut conn = connection().await;
        Ok(diesel::insert_into(logs)
            .values(&new_log)
            .get_result(&mut *conn)?)
    }

    pub async fn get(log_handle: handles::Log) -> Result<Self, Error> {
        let ty = get_log_type(&log_handle).to_string();
        let req = logs.filter(log_type.eq(ty));
        use handles::Log::*;
        (match &log_handle {
            Begin(h) | End(h) => {
                let job = Job::get(&h).await?;
                let mut conn = connection().await;
                req.filter(log_job.eq(Some(job.job_id)))
                    .first::<Log>(&mut *conn)
            }
            Eval(h) => {
                let evaluation = Evaluation::get(&h).await?;
                let mut conn = connection().await;
                req.filter(log_evaluation.eq(Some(evaluation.evaluation_id)))
                    .first::<Log>(&mut *conn)
            }
        })
        .map_err(|_| Error::LogNotFound(log_handle))
    }
}

pub mod live {
    use std::collections::HashMap;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;

    #[derive(Debug)]
    enum Message<Id> {
        Reset {
            id: Id,
        },
        Line {
            id: Id,
            line: String,
        },
        Listen {
            id: Id,
            lines_sender: mpsc::Sender<String>,
            not_found_sender: oneshot::Sender<bool>,
        },
    }

    #[derive(Debug)]
    pub struct Cache<Id>(mpsc::Sender<Message<Id>>);

    impl<Id: Clone + Eq + PartialEq + Send + std::fmt::Debug + std::hash::Hash + 'static> Cache<Id>
    where
        for<'a> &'a Id: Send,
    {
        pub fn new() -> Self {
            let (sender, mut receiver) = mpsc::channel(32);
            tokio::spawn(async move {
                type Listeners = Vec<mpsc::Sender<String>>;
                let mut state: HashMap<Id, (Vec<String>, Listeners)> = HashMap::new();
                while let Some(msg) = receiver.recv().await {
                    match msg {
                        Message::Reset { id } => {
                            state.remove(&id);
                        }
                        Message::Line { id, line } => {
                            if !state.contains_key(&id) {
                                state.insert(id.clone(), (vec![], Vec::new()));
                            }
                            let (lines, listeners) = state.get_mut(&id).unwrap();
                            lines.push(line.clone());

                            for i in 0..listeners.len() {
                                if let Err(_) = listeners[i].send(line.clone()).await {
                                    listeners.remove(i);
                                }
                            }
                        }
                        Message::Listen {
                            id,
                            lines_sender,
                            not_found_sender,
                        } => {
                            if let Some((lines, listeners)) = state.get_mut(&id) {
                                not_found_sender.send(false).unwrap();
                                for line in lines {
                                    lines_sender.send(line.clone()).await.unwrap();
                                }
                                listeners.push(lines_sender);
                            } else {
                                not_found_sender.send(true).unwrap();
                                drop(lines_sender)
                            }
                        }
                    }
                }
            });
            Cache(sender)
        }

        pub async fn listen(
            &self,
            id: &Id,
        ) -> Option<impl futures_core::stream::Stream<Item = String>> {
            let (lines_sender, mut lines_receiver) = mpsc::channel(32);
            let (not_found_sender, not_found_receiver) = oneshot::channel();
            self.0
                .send(Message::Listen {
                    id: id.clone(),
                    lines_sender,
                    not_found_sender,
                })
                .await
                .unwrap();

            if not_found_receiver.await.unwrap() {
                None
            } else {
                Some(async_stream::stream! {
                    while let Some(i) = lines_receiver.recv().await {
                        yield i;
                    }
                })
            }
        }

        pub async fn send_line(&self, id: &Id, line: String) {
            self.0
                .send(Message::Line {
                    id: id.clone(),
                    line,
                })
                .await
                .unwrap()
        }

        pub async fn reset(&self, id: &Id) {
            self.0
                .send(Message::Reset { id: id.clone() })
                .await
                .unwrap()
        }
    }
}
