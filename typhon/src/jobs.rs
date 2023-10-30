use crate::actions;
use crate::connection;
use crate::error::Error;
use crate::handles;
use crate::models;
use crate::nix;
use crate::responses;
use crate::schema;
use crate::{log_event, Event};
use crate::{JOBS_BEGIN, JOBS_BUILD, JOBS_END, JOBS_TASKS};

use diesel::prelude::*;
use serde_json::{json, Value};

use std::path::Path;
use typhon_types::data;

pub fn mk_action_status(status: &str, start: Option<i64>, end: Option<i64>) -> data::TaskStatus {
    use data::TaskStatus;
    let to_u64 = |o: Option<i64>| {
        o.map(|n| u64::try_from(n).expect("Job: broken invariant: negative `start` or `end`"))
    };
    let start = to_u64(start);
    let end = to_u64(end);
    let time_range = match (start, end) {
        (Some(start), Some(end)) => Some(data::TimeRange { start, end }),
        _ => None,
    };
    match status {
        "success" => TaskStatus::Success(time_range.expect(
            "Job: broken invariant: a successful action should have a `start` and a `end`",
        )),
        "error" => TaskStatus::Error(
            time_range
                .expect("Job: broken invariant: a failed action should have a `start` and a `end`"),
        ),
        "pending" => TaskStatus::Pending { start },
        "canceled" => TaskStatus::Canceled(time_range),
        status => panic!("Job: unknown status `{status}`"),
    }
}

impl From<models::Job> for data::Job {
    fn from(j: models::Job) -> Self {
        macro_rules! assert_unsigned {
            ($j:ident . $f: ident) => {{
                let msg = concat!("Log: broken invariant: negative `", stringify!(f), "`");
                $j.$f.try_into().ok().expect(msg)
            }};
        }
        let begin_s =
            mk_action_status(&j.begin_status, j.begin_time_started, j.begin_time_finished);
        let end_s = mk_action_status(&j.end_status, j.end_time_started, j.end_time_finished);
        let build_s =
            mk_action_status(&j.build_status, j.build_time_started, j.build_time_finished);
        data::Job {
            begin: data::Action {
                identifier: ActionIdentifier::Begin,
                status: begin_s,
            },
            end: data::Action {
                identifier: ActionIdentifier::End,
                status: end_s,
            },
            build: data::Build {
                drv: j.build_drv,
                out: j.build_out,
                status: build_s,
            },
            dist: j.dist,
            name: data::JobSystemName {
                name: j.name,
                system: j.system,
            },
            time_created: assert_unsigned!(j.time_created),
        }
    }
}

#[derive(Clone)]
pub struct Job {
    pub job: models::Job,
    pub evaluation: models::Evaluation,
    pub jobset: models::Jobset,
    pub project: models::Project,
}

use typhon_types::data::{ActionIdentifier, TaskIdentifier, TaskKind, TaskRef};

impl Job {
    pub fn get_task_ref(&self, identifier: TaskIdentifier) -> TaskRef {
        let j = self.job.clone();
        let (kind, status) = match identifier {
            TaskIdentifier::Action(identifier) => {
                let status = match &identifier {
                    ActionIdentifier::Begin => mk_action_status(
                        &j.begin_status,
                        j.begin_time_started,
                        j.begin_time_finished,
                    ),
                    ActionIdentifier::End => {
                        mk_action_status(&j.end_status, j.end_time_started, j.end_time_finished)
                    }
                };
                (TaskKind::Action { identifier }, status)
            }
            TaskIdentifier::Build => (
                TaskKind::Build {
                    drv: j.build_drv.clone(),
                    out: j.build_out.clone(),
                },
                mk_action_status(&j.build_status, j.build_time_started, j.build_time_finished),
            ),
        };
        TaskRef { kind, status }
    }
    pub async fn set_task_ref(&mut self, task: TaskRef, conn: &mut SqliteConnection) {
        use data::TaskStatus;
        let (start, end) = match task.status.clone() {
            TaskStatus::Canceled(Some(r)) | TaskStatus::Success(r) | TaskStatus::Error(r) => {
                (Some(r.start), Some(r.end))
            }
            TaskStatus::Pending { start } => (start, None),
            TaskStatus::Canceled(None) => (None, None),
        };
        match task.kind {
            TaskKind::Action {
                identifier: ActionIdentifier::Begin,
            } => {
                let job = &mut self.job;
                job.begin_status = task.status.tag().to_string();
                job.begin_time_started = start.map(|n| n as i64);
                job.begin_time_finished = end.map(|n| n as i64);
                let job = job.clone();
                let _ = diesel::update(&job)
                    .set((
                        schema::jobs::begin_status.eq(&job.begin_status),
                        schema::jobs::begin_time_started.eq(&job.begin_time_started),
                        schema::jobs::begin_time_finished.eq(&job.begin_time_finished),
                    ))
                    .execute(conn);
            }
            TaskKind::Action {
                identifier: ActionIdentifier::End,
            } => {
                let job = &mut self.job;
                job.end_status = task.status.tag().to_string();
                job.end_time_started = start.map(|n| n as i64);
                job.end_time_finished = end.map(|n| n as i64);
                let job = job.clone();
                let _ = diesel::update(&job)
                    .set((
                        schema::jobs::end_status.eq(&job.end_status),
                        schema::jobs::end_time_started.eq(&job.end_time_started),
                        schema::jobs::end_time_finished.eq(&job.end_time_finished),
                    ))
                    .execute(conn);
            }
            TaskKind::Build { drv, out } => {
                let job = &mut self.job;
                job.build_out = out;
                job.build_drv = drv;
                job.build_status = task.status.tag().to_string();
                job.build_time_started = start.map(|n| n as i64);
                job.build_time_finished = end.map(|n| n as i64);
                let job = job.clone();
                let _ = diesel::update(&job)
                    .set((
                        schema::jobs::build_out.eq(&job.build_out),
                        schema::jobs::build_drv.eq(&job.build_drv),
                        schema::jobs::build_status.eq(&job.build_status),
                        schema::jobs::build_time_started.eq(&job.build_time_started),
                        schema::jobs::build_time_finished.eq(&job.build_time_finished),
                    ))
                    .execute(conn);
            }
        }
    }
}

impl Job {
    pub async fn cancel(&self) {
        JOBS_BEGIN.cancel(self.job.id).await;
        JOBS_BUILD.cancel(self.job.id).await;
        JOBS_END.cancel(self.job.id).await;
        nix::build::BUILDS
            .abort(nix::DrvPath::new(&self.job.build_drv))
            .await;
    }

    pub async fn delete(&self) -> Result<(), Error> {
        self.cancel().await;

        let mut conn = connection().await;
        diesel::delete(schema::logs::table.find(&self.job.begin_log_id)).execute(&mut *conn)?;
        diesel::delete(schema::logs::table.find(&self.job.end_log_id)).execute(&mut *conn)?;
        drop(conn);

        nix::build::BUILDS
            .abort(nix::DrvPath::new(&self.job.build_drv))
            .await;
        JOBS_BEGIN.cancel(self.job.id).await;
        JOBS_BUILD.cancel(self.job.id).await;
        JOBS_END.cancel(self.job.id).await;

        let mut conn = connection().await;
        diesel::delete(schema::jobs::table.find(self.job.id)).execute(&mut *conn)?;
        drop(conn);

        Ok(())
    }

    pub async fn get(handle: &handles::Job) -> Result<Self, Error> {
        let mut conn = connection().await;
        let (job, (evaluation, (jobset, project))) = schema::jobs::table
            .inner_join(
                schema::evaluations::table
                    .inner_join(schema::jobsets::table.inner_join(schema::projects::table)),
            )
            .filter(schema::projects::name.eq(&handle.evaluation.jobset.project.name))
            .filter(schema::jobsets::name.eq(&handle.evaluation.jobset.name))
            .filter(schema::evaluations::num.eq(&handle.evaluation.num))
            .filter(schema::jobs::system.eq(&handle.system))
            .filter(schema::jobs::name.eq(&handle.name))
            .first(&mut *conn)
            .optional()?
            .ok_or(Error::JobNotFound(handle.clone()))?;
        Ok(Self {
            job,
            evaluation,
            jobset,
            project,
        })
    }

    pub fn handle(&self) -> handles::Job {
        handles::job((
            self.project.name.clone(),
            self.jobset.name.clone(),
            self.evaluation.num,
            self.job.system.clone(),
            self.job.name.clone(),
        ))
    }

    pub fn info(&self) -> responses::JobInfo {
        self.job.clone().into()
    }

    pub async fn log_begin(&self) -> Result<Option<String>, Error> {
        let mut conn = connection().await;
        let stderr = schema::logs::dsl::logs
            .find(self.job.begin_log_id)
            .select(schema::logs::stderr)
            .first::<Option<String>>(&mut *conn)?;
        Ok(stderr)
    }

    pub async fn log_end(&self) -> Result<Option<String>, Error> {
        let mut conn = connection().await;
        let stderr = schema::logs::dsl::logs
            .find(self.job.end_log_id)
            .select(schema::logs::stderr)
            .first::<Option<String>>(&mut *conn)?;
        Ok(stderr)
    }

    async fn mk_input(&self, status: &str) -> Result<Value, Error> {
        Ok(json!({
            "drv": self.job.build_drv,
            "evaluation": self.evaluation.num,
            "flake": self.jobset.flake,
            "job": self.job.name,
            "jobset": self.jobset.name,
            "out": self.job.build_out,
            "project": self.project.name,
            "status": status,
            "system": self.job.system,
            "url": self.evaluation.url,
        }))
    }

    pub async fn run(self) -> Result</*TODO: return `data::TaskStatus`*/ (), Error> {
        use crate::time::now;

        let drv = nix::DrvPath::new(&self.job.build_drv);

        // FIXME?
        let self_1 = self.clone();
        let self_2 = self.clone();
        let self_3 = self.clone();
        let self_4 = self.clone();
        let self_5 = self.clone();
        let self_6 = self.clone();

        // TODO: factor out common code between `begin` and `end`
        let task_begin = async move {
            let mut conn = connection().await;
            let _ = diesel::update(&self_1.job)
                .set(schema::jobs::begin_time_started.eq(now()))
                .execute(&mut *conn);
            drop(conn);

            let input = self_1.mk_input(&"pending".to_string()).await?;
            let default_log = serde_json::to_string_pretty(&input).unwrap();
            let log = if let Some(path) = self_1.evaluation.actions_path {
                if Path::new(&format!("{}/begin", path)).exists() {
                    let (_, log) = actions::run(
                        &self_1.project.key,
                        &format!("{}/begin", path),
                        &format!("{}/secrets", path),
                        &input,
                    )
                    .await?;
                    log
                } else {
                    default_log
                }
            } else {
                default_log
            };

            Ok::<_, Error>(log)
        };
        let finish_begin = move |r: Option<Result<String, Error>>| async move {
            let status = match r {
                Some(Ok(log)) => {
                    let mut conn = connection().await;
                    diesel::update(schema::logs::dsl::logs.find(self_2.job.begin_log_id))
                        .set(schema::logs::stderr.eq(log))
                        .execute(&mut *conn)
                        .unwrap(); // FIXME: no unwrap
                    "success"
                }
                Some(Err(e)) => {
                    let mut conn = connection().await;
                    diesel::update(schema::logs::dsl::logs.find(self_2.job.end_log_id))
                        .set(schema::logs::stderr.eq(e.to_string()))
                        .execute(&mut *conn)
                        .unwrap(); // FIXME: no unwrap
                    "error"
                }
                None => "canceled",
            };
            // FIXME: error management
            let mut conn = connection().await;
            let _ = diesel::update(&self_2.job)
                .set((
                    schema::jobs::begin_status.eq(status),
                    schema::jobs::begin_time_finished.eq(now()),
                ))
                .execute(&mut *conn);
            drop(conn);
            log_event(Event::JobUpdated(self_2.handle())).await;
        };
        JOBS_BEGIN.run(self.job.id, task_begin, finish_begin).await;

        let (sender, receiver) = tokio::sync::oneshot::channel::<String>();
        let task_build = async move {
            let mut conn = connection().await;
            let _ = diesel::update(&self_3.job)
                .set(schema::jobs::build_time_started.eq(now()))
                .execute(&mut *conn);
            drop(conn);
            nix::build::BUILDS.run(drv).await
        };
        let finish_build = move |r: Option<Option<Result<nix::DrvOutputs, nix::Error>>>| async move {
            let r = r.flatten();
            let status = match r {
                Some(Ok(_)) => "success",
                Some(Err(_)) => "error", // TODO: log error
                None => "canceled",
            };
            let _ = sender.send(status.to_string());
            let mut conn = connection().await;
            let _ = diesel::update(&self_4.job)
                .set((
                    schema::jobs::build_status.eq(status),
                    schema::jobs::build_time_finished.eq(now()),
                ))
                .execute(&mut *conn);
            drop(conn);
            log_event(Event::JobUpdated(self_4.handle())).await;
        };
        JOBS_BUILD.run(self.job.id, task_build, finish_build).await;

        let task_end = async move {
            // wait for `begin` to finish
            JOBS_BEGIN.wait(&self_5.job.id).await;
            // wait for the build to finish
            JOBS_BUILD.wait(&self_5.job.id).await;
            let build_status = receiver.await.unwrap_or_else(|_| panic!());

            let mut conn = connection().await;
            diesel::update(&self_5.job)
                .set(schema::jobs::end_time_started.eq(now()))
                .execute(&mut *conn)?;
            drop(conn);

            let input = self_5.mk_input(&build_status).await?;
            let default_log = serde_json::to_string_pretty(&input).unwrap();
            let log = if let Some(path) = self_5.evaluation.actions_path {
                if Path::new(&format!("{}/end", path)).exists() {
                    let (_, log) = actions::run(
                        &self_5.project.key,
                        &format!("{}/end", path),
                        &format!("{}/secrets", path),
                        &input,
                    )
                    .await?;
                    log
                } else {
                    default_log
                }
            } else {
                default_log
            };

            Ok::<_, Error>(log)
        };
        let finish_end = move |r: Option<Result<String, Error>>| async move {
            let status = match r {
                Some(Ok(log)) => {
                    let mut conn = connection().await;
                    diesel::update(schema::logs::dsl::logs.find(self_6.job.end_log_id))
                        .set(schema::logs::stderr.eq(log))
                        .execute(&mut *conn)
                        .unwrap(); // FIXME: no unwrap
                    "success"
                }
                Some(Err(e)) => {
                    let mut conn = connection().await;
                    diesel::update(schema::logs::dsl::logs.find(self_6.job.end_log_id))
                        .set(schema::logs::stderr.eq(e.to_string()))
                        .execute(&mut *conn)
                        .unwrap(); // FIXME: no unwrap
                    "error"
                }
                None => "canceled",
            };
            let mut conn = connection().await;
            // FIXME: error management
            let _ = diesel::update(&self_6.job)
                .set((
                    schema::jobs::end_status.eq(status),
                    schema::jobs::end_time_finished.eq(now()),
                ))
                .execute(&mut *conn);
            drop(conn);
            log_event(Event::JobUpdated(self_6.handle())).await;
        };
        JOBS_END.run(self.job.id, task_end, finish_end).await;

        Ok(())
    }
}
