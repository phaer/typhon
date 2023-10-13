use crate::drv_log;
use crate::{appurl::AppUrl, perform_request, view_error, view_log, SETTINGS};

use typhon_types::*;

use seed::{prelude::*, *};

pub struct Model {
    error: Option<responses::ResponseError>,
    handle: handles::Job,
    info: Option<responses::JobInfo>,
    log_begin: Option<String>,
    log_end: Option<String>,
    log: drv_log::Model,
}

impl Model {
    pub fn app_url(&self) -> AppUrl {
        Vec::<String>::from(self.handle.clone()).into()
    }
}

#[derive(Clone, Debug)]
pub enum Msg {
    Cancel,
    Error(responses::ResponseError),
    ErrorIgnored,
    Event(Event),
    FetchInfo,
    FetchLogBegin,
    FetchLogEnd,
    GetInfo(responses::JobInfo),
    GetLogBegin(Option<String>),
    GetLogEnd(Option<String>),
    LogMsg(drv_log::Msg),
    Noop,
}

pub fn init(orders: &mut impl Orders<Msg>, handle: handles::Job) -> Model {
    orders.send_msg(Msg::FetchInfo);
    Model {
        error: None,
        handle: handle.clone(),
        info: None,
        log_begin: None,
        log_end: None,
        log: drv_log::init(&mut orders.proxy(Msg::LogMsg)),
    }
}

pub fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::Cancel => {
            let handle = model.handle.clone();
            let req = requests::Request::Job(handle, requests::Job::Cancel);
            perform_request!(
                orders,
                req,
                responses::Response::Ok => Msg::Noop,
                Msg::Error,
            );
        }
        Msg::Error(err) => {
            model.error = Some(err);
        }
        Msg::ErrorIgnored => {
            model.error = None;
        }
        Msg::Event(_) => {
            orders.send_msg(Msg::FetchInfo);
        }
        Msg::FetchInfo => {
            let handle = model.handle.clone();
            let req = requests::Request::Job(handle, requests::Job::Info);
            perform_request!(
                orders,
                req,
                responses::Response::JobInfo(info) => Msg::GetInfo(info),
                Msg::Error,
            );
        }
        Msg::FetchLogBegin => {
            let handle = model.handle.clone();
            let req = requests::Request::Job(handle, requests::Job::LogBegin);
            perform_request!(
                orders,
                req,
                responses::Response::Log(log) => Msg::GetLogBegin(log),
                Msg::Error,
            );
        }
        Msg::FetchLogEnd => {
            let handle = model.handle.clone();
            let req = requests::Request::Job(handle, requests::Job::LogEnd);
            perform_request!(
                orders,
                req,
                responses::Response::Log(log) => Msg::GetLogEnd(log),
                Msg::Error,
            );
        }
        Msg::GetInfo(info) => {
            if info.begin_status != "pending" {
                orders.send_msg(Msg::FetchLogBegin);
            }
            if info.end_status != "pending" {
                orders.send_msg(Msg::FetchLogEnd);
            }
            let drv = info.build_drv.clone();
            orders.send_msg(Msg::LogMsg(drv_log::Msg::Load(drv)));
            model.info = Some(info);
        }
        Msg::GetLogBegin(log) => {
            model.log_begin = log;
        }
        Msg::GetLogEnd(log) => {
            model.log_end = log;
        }
        Msg::LogMsg(msg) => drv_log::update(msg, &mut model.log, &mut orders.proxy(Msg::LogMsg)),
        Msg::Noop => (),
    }
}

fn view_job(model: &Model) -> Node<Msg> {
    div![
        h2![
            "Job",
            " ",
            a![
                &model.handle.evaluation.jobset.project.name,
                attrs! {
                    At::Href => crate::Urls::project(&model.handle.evaluation.jobset.project),
                },
            ],
            ":",
            a![
                &model.handle.evaluation.jobset.name,
                attrs! {
                    At::Href => crate::Urls::jobset(&model.handle.evaluation.jobset),
                },
            ],
            ":",
            a![
                &model.handle.evaluation.num,
                attrs! {
                    At::Href => crate::Urls::evaluation(&model.handle.evaluation)
                },
            ],
            ":",
            &model.handle.system,
            ":",
            &model.handle.name,
        ],
        match &model.info {
            None => div!["loading..."],
            Some(info) => div![
                p![format!("Drv: {}", info.build_drv)],
                p![format!("Status (begin): {}", info.begin_status)],
                p![format!("Status (build): {}", info.build_status)],
                p![format!("Status (end): {}", info.end_status)],
                if info.dist {
                    let api_url = SETTINGS.get().unwrap().api_server.url();
                    let job = &model.handle.name;
                    let system = &model.handle.system;
                    let evaluation = &model.handle.evaluation.num;
                    let jobset = &model.handle.evaluation.jobset.name;
                    let project = &model.handle.evaluation.jobset.project.name;
                    a![
                        "Dist",
                        attrs! {
                            At::Href => format!("{}/projects/{}/jobsets/{}/evaluations/{}/jobs/{}/{}/dist/index.html",
                                                api_url, project, jobset, evaluation, system, job),
                        },
                    ]
                } else {
                    empty![]
                }
            ],
        },
        drv_log::view(&model.log).map_msg(Msg::LogMsg),
        match &model.log_begin {
            None => empty![],
            Some(log) => div![h3!["Log (begin)"], view_log(log.clone()),],
        },
        match &model.log_end {
            None => empty![],
            Some(log) => div![h3!["Log (end)"], view_log(log.clone()),],
        },
    ]
}

fn view_admin() -> Node<Msg> {
    div![
        h3!["Administration"],
        p![button!["Cancel", ev(Ev::Click, |_| Msg::Cancel),]],
    ]
}

pub fn view(model: &Model, admin: bool) -> Node<Msg> {
    model
        .error
        .as_ref()
        .map(|err| view_error(err, Msg::ErrorIgnored))
        .unwrap_or(div![
            view_job(model),
            if admin { view_admin() } else { empty![] },
        ])
}
