use crate::actions::webhooks;
use crate::projects;
use crate::requests::*;
use crate::{handle_request, handles, Response, ResponseError, User};
use crate::{BUILD_LOGS, EVENT_LOGGER, SETTINGS};

use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::{
    body::EitherBody, guard, http::StatusCode, web, Error, HttpRequest, HttpResponse, Responder,
};

use std::collections::HashMap;

struct ResponseWrapper(crate::Response);
#[derive(Debug)]
struct ResponseErrorWrapper(crate::ResponseError);

impl std::fmt::Display for ResponseErrorWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Responder for ResponseWrapper {
    type Body = EitherBody<String>;
    fn respond_to(self, req: &HttpRequest) -> HttpResponse<Self::Body> {
        use crate::Response::*;
        match self.0 {
            Ok => web::Json(true).respond_to(req),
            SearchEvaluations(payload) => web::Json(payload).respond_to(req),
            ListProjects(payload) => web::Json(payload).respond_to(req),
            ProjectInfo(payload) => web::Json(payload).respond_to(req),
            ProjectUpdateJobsets(payload) => web::Json(payload).respond_to(req),
            JobsetInfo(payload) => web::Json(payload).respond_to(req),
            JobsetEvaluate(payload) => web::Json(payload).respond_to(req),
            EvaluationInfo(payload) => web::Json(payload).respond_to(req),
            JobInfo(payload) => web::Json(payload).respond_to(req),
            Log(payload) => web::Json(payload).respond_to(req),
            Login { token } => web::Json(token).respond_to(req),
        }
    }
}

impl actix_web::ResponseError for ResponseErrorWrapper {
    fn status_code(&self) -> StatusCode {
        match self.0 {
            ResponseError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ResponseError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseError::ResourceNotFound(_) => StatusCode::NOT_FOUND,
        }
    }
}

/// A macro to generate api endpoints
macro_rules! r {
    ($name: ident($($i: ident : $t: ty),*) => $e: expr
     ;$($rest: tt)*
    ) => {
    async fn $name (user: User, $($i : $t),*) -> Result<ResponseWrapper, ResponseErrorWrapper> {
        handle_request(user, $e).await.map(ResponseWrapper).map_err(ResponseErrorWrapper)
    } r!( $($rest)* );
    };
    (  ) => {}
}

r!(
    search_evaluations(body: web::Json<EvaluationSearch>) =>
        Request::SearchEvaluations(body.into_inner());

    create_project(path: web::Path<String>, body: web::Json<ProjectDecl>) => {
        let name = path.into_inner();
        let decl = body.into_inner();
        Request::CreateProject { name, decl }
    };

    list_projects() => Request::ListProjects;

    project_delete(path: web::Path<String>) =>
        Request::Project(
            handles::project(path.into_inner()),
            Project::Delete,
        );

    project_info(path: web::Path<String>) =>
        Request::Project(
            handles::project(path.into_inner()),
            Project::Info,
        );

    project_refresh(path: web::Path<String>) =>
        Request::Project(
            handles::project(path.into_inner()),
            Project::Refresh,
        );

    project_set_decl(path: web::Path<String>, body: web::Json<ProjectDecl>) =>
        Request::Project(
            handles::project(path.into_inner()),
            Project::SetDecl(body.into_inner()),
        );

    project_set_private_key(path: web::Path<String>, body: web::Json<String>) =>
        Request::Project(
            handles::project(path.into_inner()),
            Project::SetPrivateKey(body.into_inner()),
        );

    project_update_jobsets(path: web::Path<String>) =>
        Request::Project(
            handles::project(path.into_inner()),
            Project::UpdateJobsets,
        );

    jobset_evaluate(path: web::Path<(String,String)>) =>
        Request::Jobset(
            handles::jobset(path.into_inner()),
            Jobset::Evaluate(true),
        );

    jobset_info(path: web::Path<(String,String)>) =>
        Request::Jobset(
            handles::jobset(path.into_inner()),
            Jobset::Info,
        );

    evaluation_cancel(path: web::Path<(String,String,i64)>) =>
        Request::Evaluation(
            handles::evaluation(path.into_inner()),
            Evaluation::Cancel,
        );

    evaluation_info(path: web::Path<(String,String,i64)>) =>
        Request::Evaluation(
            handles::evaluation(path.into_inner()),
            Evaluation::Info,
        );

    evaluation_log(path: web::Path<(String,String,i64)>) =>
        Request::Evaluation(
            handles::evaluation(path.into_inner()),
            Evaluation::Log,
        );

    job_cancel(path: web::Path<(String,String,i64,String,String)>) =>
        Request::Job(
            handles::job(path.into_inner()),
            Job::Cancel,
        );

    job_info(path: web::Path<(String,String,i64,String,String)>) =>
        Request::Job(
            handles::job(path.into_inner()),
            Job::Info,
        );

    // job_log_begin(path: web::Path<(String,String,i64,String,String)>) =>
    //     Request::Job(
    //         handles::job(path.into_inner()),
    //         Job::LogBegin,
    //     );

    // job_log_end(path: web::Path<(String,String,i64,String,String)>) =>
    //     Request::Job(
    //         handles::job(path.into_inner()),
    //         Job::LogEnd,
    //     );

    login(body: web::Json<String>) =>
        Request::Login(body.into_inner());
);

async fn dist(
    user: User,
    path: web::Path<(String, String, i64, String, String, String)>,
) -> Result<impl Responder, ResponseErrorWrapper> {
    let (project, jobset, evaluation, system, job, path) = path.into_inner();
    let handle = handles::job((project, jobset, evaluation, system, job));
    let req = Request::Job(handle, Job::Info);
    let rsp = handle_request(user, req)
        .await
        .map_err(ResponseErrorWrapper)?;
    let info = match rsp {
        Response::JobInfo(info) => Ok(info),
        _ => Err(ResponseErrorWrapper(ResponseError::InternalError)),
    }?;
    if info.dist {
        Ok(NamedFile::open_async(format!("{}/{}", info.build.out, path)).await)
    } else {
        Err(ResponseErrorWrapper(ResponseError::BadRequest(
            "typhonDist is not set".into(),
        )))
    }
}

/// Serves the log in live for derivation [path].
async fn drv_log(path: web::Path<String>) -> Option<HttpResponse> {
    use crate::nix;
    use futures::stream::StreamExt;
    let path = path.into_inner().to_string();
    let drv = nix::DrvPath::new(&path);
    match BUILD_LOGS.listen(&drv).await {
        Some(stream) => {
            let stream = stream.map(|x: String| {
                Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(format!("{}\n", x)))
            });
            Some(HttpResponse::Ok().streaming(stream))
        }
        None => match nix::log(path).await {
            Ok(log) => Some(HttpResponse::Ok().body(web::Bytes::from(log))),
            Err(_) => None,
        },
    }
}

/// Serves the log in live for derivation [path].
async fn job_live_log(
    user: User,
    handle: web::Json<typhon_types::handles::Log>,
) -> Option<HttpResponse> {
    use typhon_types::{
        data::{ActionIdentifier, TaskIdentifier},
        handles::Log,
    };
    match handle.identifier {
        TaskIdentifier::Action(identifier) => {
            use typhon_types::responses::Response;
            let Ok(Response::Log(contents)) =
                handle_request(user, Request::Job(handle.job.clone(), Job::LogBegin)).await
            else {
                None?
            };
            Some(HttpResponse::Ok().body(web::Bytes::from(contents?)))
        }
        TaskIdentifier => {
            use crate::nix;
            use futures::stream::StreamExt;
            let Ok(job) = crate::jobs::Job::get(&handle.job).await else {
                None?
            };
            let path = job.job.build_drv;
            let drv = nix::DrvPath::new(&path);
            match BUILD_LOGS.listen(&drv).await {
                Some(stream) => {
                    let stream = stream.map(|x: String| {
                        Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(format!("{}\n", x)))
                    });
                    Some(HttpResponse::Ok().streaming(stream))
                }
                None => match nix::log(path).await {
                    Ok(log) => Some(HttpResponse::Ok().body(web::Bytes::from(log))),
                    Err(_) => None,
                },
            }
        }
    }
}

async fn raw_request(
    user: User,
    body: web::Json<Request>,
) -> web::Json<Result<Response, ResponseError>> {
    web::Json(handle_request(user, body.into_inner()).await)
}

async fn events() -> Option<HttpResponse> {
    use futures::StreamExt;
    EVENT_LOGGER.listen().await.map(|stream| {
        HttpResponse::Ok().streaming(stream.map(|x: typhon_types::Event| {
            Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(
                serde_json::to_string(&x).unwrap(),
            ))
        }))
    })
}

async fn webhook(
    path: web::Path<String>,
    req: HttpRequest,
    body: String,
) -> Result<HttpResponse, Error> {
    let input = webhooks::Input {
        headers: req
            .headers()
            .into_iter()
            .map(|(name, value)| {
                Ok((
                    name.as_str().to_string(),
                    std::str::from_utf8(value.as_bytes())
                        .map_err(|_| {
                            ResponseErrorWrapper(ResponseError::BadRequest(
                                "non-utf8 characters in request headers".to_string(),
                            ))
                        })?
                        .to_string(),
                ))
            })
            .collect::<Result<HashMap<_, _>, Error>>()?,
        body,
    };

    log::info!("handling webhook {:?}", input);

    let project_handle = handles::project(path.into_inner().to_string());
    let project = projects::Project::get(&project_handle).await.map_err(|e| {
        if e.is_internal() {
            log::error!("webhook raised error: {:?}", e);
        }
        ResponseErrorWrapper(e.into())
    })?;

    let requests = project
        .webhook(input)
        .await
        .map_err(|e| {
            if e.is_internal() {
                log::error!("webhook raised error: {:?}", e);
            }
            ResponseErrorWrapper(e.into())
        })?
        .into_iter();

    for req in requests {
        let _ = handle_request(User::Admin, req).await;
    }

    Ok(HttpResponse::Ok().finish())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    let cors = Cors::permissive(); // TODO: configure
    cfg.service(
        web::scope(&format!("{}/api", SETTINGS.webroot))
            .route("", web::post().to(raw_request))
            .route("/evaluations", web::post().to(search_evaluations))
            .route("/events", web::get().to(events))
            .route("/projects", web::get().to(list_projects))
            .route("/drv-log{path:.*}", web::get().to(drv_log))
            .route("/log", web::post().to(job_live_log))
            .service(
                web::scope("/projects/{project}")
                    .route("", web::get().to(project_info))
                    .route("/create", web::post().to(create_project))
                    .route("/delete", web::post().to(project_delete))
                    .route("/refresh", web::post().to(project_refresh))
                    .route("/update_jobsets", web::post().to(project_update_jobsets))
                    .route("/set_decl", web::post().to(project_set_decl))
                    .route("/set_private_key", web::post().to(project_set_private_key))
                    .route("/webhook", web::post().to(webhook))
                    .service(
                        web::scope("/jobsets/{jobset}")
                            .route("", web::get().to(jobset_info))
                            .route("/evaluate", web::post().to(jobset_evaluate))
                            .service(
                                web::scope("/evaluations/{evaluation}")
                                    .route("", web::get().to(evaluation_info))
                                    .route("/cancel", web::post().to(evaluation_cancel))
                                    .route("/log", web::get().to(evaluation_log))
                                    .service(
                                        web::scope("/jobs/{system}/{job}")
                                            .route("", web::get().to(job_info))
                                            .route("/cancel", web::post().to(job_cancel))
                                            // .route("/logs/begin", web::get().to(job_log_begin))
                                            // .route("/logs/end", web::get().to(job_log_end))
                                            .route("/dist/{path:.*}", web::get().to(dist)),
                                    ),
                            ),
                    ),
            )
            .route("/login", web::post().to(login))
            .route(
                "{anything:.*}",
                web::route()
                    .guard(guard::Options())
                    .to(|| HttpResponse::Ok()),
            )
            .wrap(cors),
    );
}
