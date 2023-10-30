use crate::connection;
use crate::error::Error;
use crate::gcroots;
use crate::jobs;
use crate::log_event;
use crate::models;
use crate::nix;
use crate::responses;
use crate::schema;
use crate::Event;
use crate::EVALUATIONS;

use crate::nix::JobDerivation;
use typhon_types::{responses::JobSystemName, *};

use diesel::prelude::*;

#[derive(Clone)]
pub struct Evaluation {
    pub evaluation: models::Evaluation,
    pub jobset: models::Jobset,
    pub project: models::Project,
}

impl Evaluation {
    pub async fn cancel(&self) {
        EVALUATIONS.cancel(self.evaluation.id).await
    }

    pub async fn delete(&self) -> Result<(), Error> {
        self.cancel().await;

        let mut conn = connection().await;
        let jobs: Vec<jobs::Job> = schema::jobs::table
            .filter(schema::jobs::evaluation_id.eq(self.evaluation.id))
            .load::<models::Job>(&mut *conn)?
            .drain(..)
            .map(|job| jobs::Job {
                job,
                evaluation: self.evaluation.clone(),
                jobset: self.jobset.clone(),
                project: self.project.clone(),
            })
            .collect();
        diesel::delete(schema::logs::table.find(&self.evaluation.log_id)).execute(&mut *conn)?;
        drop(conn);

        for job in jobs.iter() {
            job.delete().await?;
        }

        let mut conn = connection().await;
        diesel::delete(schema::evaluations::table.find(&self.evaluation.id)).execute(&mut *conn)?;
        drop(conn);

        Ok(())
    }

    pub async fn get(handle: &handles::Evaluation) -> Result<Self, Error> {
        let mut conn = connection().await;
        let (evaluation, (jobset, project)) = schema::evaluations::table
            .inner_join(schema::jobsets::table.inner_join(schema::projects::table))
            .filter(schema::projects::name.eq(&handle.jobset.project.name))
            .filter(schema::jobsets::name.eq(&handle.jobset.name))
            .filter(schema::evaluations::num.eq(&handle.num))
            .first(&mut *conn)
            .optional()?
            .ok_or(Error::EvaluationNotFound(handle.clone()))?;
        Ok(Self {
            evaluation,
            jobset,
            project,
        })
    }

    pub fn handle(&self) -> handles::Evaluation {
        handles::evaluation((
            self.project.name.clone(),
            self.jobset.name.clone(),
            self.evaluation.num,
        ))
    }

    pub fn generic_info<Jobs>(&self, jobs: Jobs) -> responses::EvaluationInfo<Jobs> {
        responses::EvaluationInfo {
            actions_path: self.evaluation.actions_path.clone(),
            flake: self.jobset.flake,
            jobs,
            status: jobs::mk_action_status(
                &self.evaluation.status,
                Some(self.evaluation.time_created),
                Some(self.evaluation.time_created),
                // TODO: `time_finished` is never updated! self.evaluation.time_finished,
            ),
            time_created: self.evaluation.time_created,
            time_finished: self.evaluation.time_finished,
            url: self.evaluation.url.clone(),
        }
    }

    pub async fn info(&self) -> Result<responses::EvaluationInfo, Error> {
        let jobs = schema::jobs::table
            .filter(schema::jobs::evaluation_id.eq(self.evaluation.id))
            .load::<models::Job>(&mut *connection().await)?
            .iter()
            .map(|job| JobSystemName {
                system: job.system.clone(),
                name: job.name.clone(),
            })
            .collect();
        Ok(self.generic_info(jobs))
    }

    pub async fn log(&self) -> Result<Option<String>, Error> {
        let mut conn = connection().await;
        let stderr = schema::logs::dsl::logs
            .find(self.evaluation.log_id)
            .select(schema::logs::stderr)
            .first::<Option<String>>(&mut *conn)?;
        Ok(stderr)
    }

    pub async fn run(self) -> Result<(), Error> {
        let task = {
            let url = self.evaluation.url.clone();
            let flake = self.jobset.flake.clone();
            async move { nix::eval_jobs(&url, flake).await }
        };
        let evaluation_id = self.evaluation.id.clone();
        let finish = move |r: Option<Result<nix::NewJobs, nix::Error>>| async move {
            // TODO: when logging, hide internal error messages?
            let status = match r {
                Some(Ok(new_jobs)) => {
                    match self.run_aux(new_jobs).await {
                        Ok(()) => "success",
                        Err(e) => {
                            let mut conn = connection().await;
                            diesel::update(schema::logs::dsl::logs.find(self.evaluation.log_id))
                                .set(schema::logs::stderr.eq(e.to_string()))
                                .execute(&mut *conn)
                                .unwrap(); // FIXME: no unwrap
                            "error"
                        }
                    }
                }
                Some(Err(e)) => {
                    let mut conn = connection().await;
                    diesel::update(schema::logs::dsl::logs.find(self.evaluation.log_id))
                        .set(schema::logs::stderr.eq(e.to_string()))
                        .execute(&mut *conn)
                        .unwrap(); // FIXME: no unwrap
                    "error"
                }
                None => "canceled",
            };

            // update the evaluation status
            let _ = diesel::update(&self.evaluation)
                .set(schema::evaluations::status.eq(status))
                .execute(&mut *connection().await);

            gcroots::update().await;

            log_event(Event::EvaluationFinished(self.handle())).await;
        };
        EVALUATIONS.run(evaluation_id, task, finish).await;

        Ok(())
    }

    async fn run_aux(&self, mut new_jobs: nix::NewJobs) -> Result<(), Error> {
        let now = crate::time::now();
        let mut conn = connection().await;
        let created_jobs = conn.transaction::<Vec<jobs::Job>, Error, _>(|conn| {
            new_jobs
                .drain()
                .map(
                    |(JobSystemName { system, name }, JobDerivation { drv, has_dist })| {
                        let begin_log = diesel::insert_into(schema::logs::dsl::logs)
                            .values(&models::NewLog { stderr: None })
                            .get_result::<models::Log>(&mut *conn)?;
                        let end_log = diesel::insert_into(schema::logs::dsl::logs)
                            .values(&models::NewLog { stderr: None })
                            .get_result::<models::Log>(&mut *conn)?;
                        let job = diesel::insert_into(schema::jobs::table)
                            .values(&models::NewJob {
                                begin_log_id: begin_log.id,
                                begin_status: "pending",
                                build_drv: &String::from(drv.path.clone()).as_str(),
                                build_out: drv
                                    .outputs
                                    .iter()
                                    .last()
                                    .expect("TODO: derivations can have multiple outputs")
                                    .1,
                                build_status: "pending",
                                dist: has_dist,
                                end_log_id: end_log.id,
                                end_status: "pending",
                                evaluation_id: self.evaluation.id,
                                name: &name,
                                system: &system,
                                time_created: now,
                            })
                            .get_result(conn)?;
                        Ok(jobs::Job {
                            project: self.project.clone(),
                            jobset: self.jobset.clone(),
                            evaluation: self.evaluation.clone(),
                            job,
                        })
                    },
                )
                .collect()
        })?;
        drop(conn);

        for job in created_jobs.into_iter() {
            job.run().await?;
        }

        Ok(())
    }

    pub async fn search(
        search: &requests::EvaluationSearch,
    ) -> Result<Vec<(handles::Evaluation, responses::EvaluationInfo<()>)>, Error> {
        let mut query = schema::evaluations::table
            .inner_join(schema::jobsets::table.inner_join(schema::projects::table))
            .into_boxed();
        if let Some(name) = &search.project_name {
            query = query.filter(schema::projects::name.eq(name));
        }
        if let Some(name) = &search.jobset_name {
            query = query.filter(schema::jobsets::name.eq(name));
        }
        Ok(query
            .order(schema::evaluations::time_created.desc())
            .offset(search.offset as i64)
            .limit(search.limit as i64)
            .load(&mut *connection().await)?
            .into_iter()
            .map(|(evaluation, (jobset, project))| Evaluation {
                evaluation,
                jobset,
                project,
            })
            .map(|eval| (eval.handle(), eval.generic_info(())))
            .collect())
    }
}
