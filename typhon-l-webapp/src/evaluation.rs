use crate::prelude::*;
use data::TaskStatusKind;

pub async fn fetch_data(
    handle: handles::Evaluation,
) -> (
    responses::EvaluationInfo,
    Vec<(handles::Job, responses::JobInfo)>,
) {
    let Ok(responses::Response::EvaluationInfo(eval)) = handle_request(
        &requests::Request::Evaluation(handle.clone(), requests::Evaluation::Info),
    )
    .await
    else {
        panic!()
    };
    let mut jobs = vec![];
    for job in &eval.jobs {
        let handle = handles::Job {
            evaluation: handle.clone(),
            name: job.name.clone(),
            system: job.system.clone(),
        };
        let r = handle_request(&requests::Request::Job(handle.clone(), requests::Job::Info)).await;
        let Ok(responses::Response::JobInfo(job)) = r else {
            panic!()
        };
        jobs.push((handle.clone(), job));
    }
    (eval, jobs)
}

#[component]
pub fn Status(#[prop(into)] status: Signal<TaskStatusKind>) -> impl IntoView {
    let styler_class = style! {
        .status {
            display: flex;
            height: 100%;
            aspect-ratio: "1 / 1";
            text-align: center;
            align-items: flex-start;
            width: "1em";
            height: "1em";
        }
        .status[data-status=Success] {
            color: rgb(26, 127, 55);
        }
        .status[data-status=Error] {
            color: rgb(209, 36, 47);
        }
        .status[data-status=Canceled] {
            color: rgb(101, 109, 118);
        }
        .status[data-status=Pending] {
            display: inline-block;
            color: #FFD32A;
            animation-name: spin;
            animation-duration: 2000ms;
            animation-iteration-count: infinite;
            animation-timing-function: linear;
            display: inline-block;
        }
        .status[data-status=Pending] :deep(.icon-wrapper) {
        }
        @keyframes spin {
            from { transform:rotate(0deg); }
            to { transform:rotate(360deg); }
        }
    };
    view! { class=styler_class,
        <span class="status" data-status=move || format!("{:?}", status())>
            <span class="icon-wrapper">
                {move || {
                    view! {
                        <Icon icon=Icon::from(
                            match status() {
                                TaskStatusKind::Success => BiCheckCircleSolid,
                                TaskStatusKind::Pending => BiLoaderAltRegular,
                                TaskStatusKind::Error => BiXCircleSolid,
                                TaskStatusKind::Canceled => BiStopCircleRegular,
                            },
                        )/>
                    }
                }}

            </span>

        </span>
    }
}

#[component]
pub fn JobSubpage(
    #[prop(into)] job: handles::Job,
    #[prop(into)] info: Signal<responses::JobInfo>,
) -> impl IntoView {
    let job_item_style = style! {
        details :deep(> summary > span) {
            display: inline-block;
        }
        details :deep(> summary) {
            padding: 4px;
            margin: 4px;
        }
        details :deep(> summary) {
            display: grid;
            grid-template-columns: auto auto 1fr auto;
        }
        details[open] :deep(> summary > .icon > *) {
            transform: rotate(90deg);
        }
        details :deep(> summary > .icon > *) {
            transition: transform 100ms;
        }
        details :deep(> summary > time) {
            font-family: JetBrains Mono;
        }
        details :deep(> summary > .status) {
            padding: 0 "0.5em";
        }
    };
    let log = {
        let job = job.clone();
        use typhon_types::data;
        move |task_ref: Signal<data::TaskRef>| {
            let action_status = Signal::derive(move || task_ref().status);
            let title = move || match data::TaskIdentifier::from(&task_ref()) {
                data::TaskIdentifier::Action(data::ActionIdentifier::Begin) => "Pre action",
                data::TaskIdentifier::Action(data::ActionIdentifier::End) => "Post action",
                data::TaskIdentifier::Build => "Nix build",
            };
            view! {
                <details class=job_item_style>
                    <summary>
                        <span class="icon">
                            <Icon icon=Icon::from(BiChevronRightRegular)/>
                        </span>
                        <span class="status">
                            <Status status=Signal::derive(move || action_status().into())/>
                        </span>
                        <span>{title}</span>
                        <time datetime="PT2H30M">2h 30m</time>
                    </summary>

                    {
                        view! {
                            <LiveLog request={
                                let url = &format!(
                                    "{}/log",
                                    crate::handle_request::Settings::load().api_url,
                                );
                                use gloo_net::http;
                                http::RequestBuilder::new(url.as_str())
                                    .method(http::Method::POST)
                                    .json(
                                        &handles::Log {
                                            job: job.clone(),
                                            identifier: task_ref.with(|task_ref| task_ref.into()),
                                        },
                                    )
                                    .unwrap()
                            }/>
                        }
                    }

                </details>
            }
        }
    };
    let style = style! {
        div.header, div.contents {
            padding: 16px;
        }
        div.header {
            border-bottom: 1px solid #32383F;
            display: grid;
            grid-template-columns: 1fr auto auto auto;
            align-items: center;
        }
        h1, h2 {
            padding: 0;
            margin: 0;
        }
        h1 {
            font-size: 110%;
            font-weight: 400;
        }
        h2 {
            font-size: 75%;
            font-weight: 300;
            padding-top: 2px;
            color: #8C959F;
        }
        .search {
            display: flex;
            align-items: center;
            background: #32383F;
            border-radius: 6px;
            padding: 0px 8px;
            height: "32px";
        }
        .search input {
            padding: 0px 8px;
            margin: 0px;
            background: none;
            border: none;
            color: inherit;
        }
        .search .indicator {
            color: #8C959F;
            font-size: 80%;
            margin-right: 4px;
        }
        .search :deep(svg) {
            color: #8C959F;
        }
        .search input:focus {
            outline: none;
        }
    };
    view! { class=style,
        <div class="header">
            <div class="name">
                <h1>
                    <span>{job.name}</span>
                    <span>{format!(" ({})", job.system)}</span>
                </h1>
                <h2>succeeded 2 days ago in 2m 3s</h2>
            </div>
            <div class="search">
                <Icon icon=Icon::from(BiSearchAltRegular)/>
                <input placeholder="Search logs"/>
                <div class="indicator">0/0</div>
                <Icon icon=Icon::from(BiChevronUpRegular)/>
                <Icon icon=Icon::from(BiChevronDownRegular)/>
            </div>
            <Icon icon=Icon::from(BiRefreshRegular)/>
            <Icon icon=Icon::from(BiCogRegular)/>
        </div>
        <div class="contents">
            {vec![
                log(Signal::derive(move || typhon_types::data::TaskRef::from(info().begin))),
                log(Signal::derive(move || typhon_types::data::TaskRef::from(info().build))),
                log(Signal::derive(move || typhon_types::data::TaskRef::from(info().end))),
            ]}

        </div>
    }
}

#[component]
pub fn Main(
    #[prop(into)] handle: handles::Evaluation,
    #[allow(unused)]
    #[prop(into)]
    info: responses::EvaluationInfo,
    #[prop(into)] jobs: Vec<(handles::Job, ReadSignal<responses::JobInfo>)>,
    #[prop(into)] tab: Signal<crate::routes::EvaluationTab>,
) -> impl IntoView {
    let active_tab = tab;
    use crate::routes::EvaluationTab as ActiveItem;
    let item_style = style! {
        .active {
            font-weight: 400;
        }
        li {
            margin: 0;
            list-style-type: none;
            padding: "0.1em";
        }
        li :deep(> a > span.label) {
            text-overflow: ellipsis;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }
        li:hover :deep(> a) {
            background: #F4F5F7;
            border-radius: 5px;
        }
        li.active :deep(> a) {
            background: #F4F5F7;
            border-radius: 5px;
        }
        li :deep(> a) {
            text-decoration: none;
            color: inherit;
            display: flex;
            align-items: center;
            padding: "0.5em";
        }
        .icon {
            margin-right: "0.4em";
            display: flex;
            align-items: center;
            color: gray;
        }
    };
    let mk_item = |tab: ActiveItem, icon, contents: View| {
        let handle = handle.clone();
        view! { class=item_style,
            <li class:active={
                let tab = tab.clone();
                move || active_tab() == tab.clone()
            }>

                <A href=Box::new(move || crate::routes::to_url(crate::routes::EvaluationPage {
                    handle: handle.clone(),
                    tab: tab.clone(),
                }))>
                    <span class="icon">{icon}</span>
                    <span class="label">{contents}</span>
                </A>
            </li>
        }
    };
    let items = jobs
        .clone()
        .into_iter()
        .map(|(job, job_info)| {
            mk_item(
                ActiveItem::Job(job.clone()),
                view! { <Status status=Signal::derive(move || job_info().into())/> },
                view! {
                    <span>
                        {job.name}
                        <span style="color: gray; font-size: 90%;">
                            {format!(" ({})", job.system)}
                        </span>
                    </span>
                }
                .into_view(),
            )
        })
        .collect::<Vec<_>>();
    let style = style! {
        nav {
            padding: 16px;
        }
        nav :deep(section > ul) {
            padding: 0;
            margin: 0;
        }
        nav :deep(section > h1) {
            color: rgb(101, 109, 118);
            font-weight: 500;
            font-size: 80%;
            border-top: 1px solid rgba(208, 215, 222, 0.48);
            padding-top: 16px;
            margin-top: 8px;
        }
    };
    let main = view! {
        <nav class=style>
            <section>
                <ul style="padding: 0;">
                    {mk_item(
                        ActiveItem::Summary,
                        view! { <Icon icon=Icon::from(BiHomeAltRegular)/> },
                        view! { Summary }.into_view(),
                    )}

                </ul>
            </section>
            <section>
                <h1>Jobs</h1>
                <ul style="padding: 0;">{items}</ul>
            </section>
            <section>
                <h1>Details</h1>
                <ul style="padding: 0;">
                    {mk_item(
                        ActiveItem::Usage,
                        view! { <Icon icon=Icon::from(BiTimerRegular)/> },
                        view! { Usage }.into_view(),
                    )}

                </ul>
            </section>
        </nav>
        // <div>
        <div class="contents">

            {
                let jobs = jobs.clone();
                move || {
                    match active_tab() {
                        ActiveItem::Summary => "Summary page, todo".into_view(),
                        ActiveItem::Job(job) => {
                            let info = jobs.iter().find(|(j, _)| &job == j).unwrap().1;
                            view! { <JobSubpage job info/> }
                        }
                        ActiveItem::Usage => "Usage page, todo".into_view(),
                    }
                }
            }

        </div>
    };
    let global_status = move || {
        jobs.iter()
            .map(|(_, info)| TaskStatusKind::from(info()))
            .max()
            .unwrap_or(TaskStatusKind::Success)
    };
    let style = style! {
        div {
            display: grid;
            grid-template-areas: raw_str("header header") raw_str("nav contents");
            grid-template-columns: 250px 1fr;
            margin-right: 16px;
        }
        div :deep(> header) {
            grid-area: header;
            padding: 16px;
        }
        div :deep(> nav) {
            grid-area: nav;
        }
        div :deep(> .contents) {
            grid-area: contents;
            background: rgb(36, 41, 47);
            border-radius: 3px;
            color: rgb(246, 248, 250);
        }
    };
    let header_style = style! {
        header {
            display: grid;
            grid-template-areas: raw_str("a a a") raw_str("s b1 b2");
            grid-template-columns: 1fr auto auto;
        }
        header :deep(> .all-evaluations) {
            grid-area: a;
            text-decoration: inherit;
            color: inherit;
        }
        header :deep(> .summary) {
            grid-area: s;
            font-size: 160%;
        }
        header :deep(> button) {
            padding: "0.4em";
            margin: "0.4em";
        }
        header :deep(> .rerun-jobs) {
            grid-area: b1;
        }
        header :deep(> .more) {
            grid-area: b2;
        }
    };
    view! {
        <div class=style>
            <header class=header_style>
                <A
                    class="all-evaluations"
                    href=crate::routes::Root::Jobset {
                        handle: handle.jobset.clone(),
                        page: 0,
                    }
                >
                    <Icon icon=Icon::from(BiLeftArrowAltSolid)/>
                    Other evaluations
                </A>
                <div class="summary">
                    <span style="display: inline-block;">
                        <Status status=Signal::derive(global_status)/>
                    </span>
                    {format!("{}:{}", handle.jobset.name, handle.num)}
                </div>
                <button class="rerun-jobs">Re-run all jobs</button>
                <button class="more">
                    <Icon icon=Icon::from(BiDotsHorizontalRoundedRegular)/>
                </button>
            </header>
            {main}
        </div>
    }
}

#[component]
pub fn Evaluation(
    #[prop(into)] handle: Signal<handles::Evaluation>,
    #[prop(into)] tab: Signal<crate::routes::EvaluationTab>,
) -> impl IntoView {
    let eval = create_resource(handle, fetch_data);
    move || match eval() {
        Some((info, jobs)) => {
            let jobs = jobs
                .into_iter()
                .map(|(job, info)| (job, create_signal(info)));
            let jobs: Vec<_> = jobs.map(|(job, (info, _set_info))| (job, info)).collect();
            (view! { <Main handle=handle() info jobs tab/> }).into_view()
        }
        _ => (view! { "Loading" }).into_view(),
    }
}
