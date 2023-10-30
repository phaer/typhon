use crate::prelude::*;

const PAGE_MAX_ITEMS: u8 = 2;

async fn fetch_infos(handle: handles::Jobset) -> responses::JobsetInfo {
    let req = requests::Request::Jobset(handle.clone(), requests::Jobset::Info);
    let Ok(responses::Response::JobsetInfo(infos)) = handle_request(&req).await else {
        panic!()
    };
    infos
}
async fn fetch_evaluations(
    handle: handles::Jobset,
    page: u32,
) -> Vec<(handles::Evaluation, responses::EvaluationInfo<()>)> {
    let req = requests::Request::SearchEvaluations(requests::EvaluationSearch {
        jobset_name: Some(handle.name),
        project_name: Some(handle.project.name),
        limit: PAGE_MAX_ITEMS,
        offset: PAGE_MAX_ITEMS as u32 * page,
    });
    let Ok(responses::Response::SearchEvaluations(evals)) = handle_request(&req).await else {
        panic!()
    };
    evals
}

#[component]
pub fn Main(
    #[prop(into)] handle: handles::Jobset,
    #[prop(into)] infos: responses::JobsetInfo,
    #[prop(into)] page: Signal<u32>,
) -> impl IntoView {
}

#[component]
pub fn Main(
    #[prop(into)] handle: handles::Jobset,
    #[prop(into)] infos: responses::JobsetInfo,
    #[prop(into)] page: Signal<u32>,
) -> impl IntoView {
    let style = style! {
        .pages :deep(a.page) {
            text-decoration: inherit;
            color: inherit;
        }
        .pages :deep(span.page) {
            display: block-inline;
            padding: 8px 12px;
            margin: 8px 3px;
        }
        .pages :deep(.active) {
            background: rgb(9, 105, 218);
            color: white;
            border-radius: 5px;
        }
    };
    let evaluations = create_resource(page, {
        let handle = handle.clone();
        move |page| fetch_evaluations(handle.clone(), page - 1)
    });
    let pages_no = infos.evaluations_count / PAGE_MAX_ITEMS as u32;
    let pages = 0;
    // let pages = (1..pages_no).map(|n| move || (Some(n), n.into_view()));
    // let pages = [move || (if page() == 1 { None } else { Some(0) }, "PRev".into_view())]
    //     .into_iter()
    //     .chain(pages)
    //     .chain([].into_iter());
    // let pages = pages
    //     .map(|contents| {
    //         let handle = handle.clone();
    //         move || {
    //             let contents = contents.clone();
    //             let (i, contents) = contents();
    //             let contents = view! {
    //                 <span class="page" class:active=move || Some(page()) == i>
    //                     {contents}
    //                 </span>
    //             };
    //             if let Some(i) = i {
    //                 let href = routes::Root::Jobset {
    //                     handle: handle.clone(),
    //                     page: i,
    //                 };
    //                 view! {
    //                     <A href class="page">
    //                         {contents}
    //                     </A>
    //                 }
    //             } else {
    //                 contents.into_view()
    //             }
    //         }
    //     })
    //     .collect::<Vec<_>>();
    view! { class=style,
        <header>
            <h1>{format!("{:?}", handle)}</h1>
        </header>
        <div class="pages">

            {pages}
        </div>
    }
}

#[component]
pub fn Jobset(
    #[prop(into)] handle: Signal<handles::Jobset>,
    #[prop(into)] page: Signal<u32>,
) -> impl IntoView {
    let infos = create_resource(handle, fetch_infos);
    move || match infos() {
        Some(infos) => {
            // let jobs = jobs
            //     .into_iter()
            //     .map(|(job, info)| (job, create_signal(info)));
            // let jobs: Vec<_> = jobs.map(|(job, (info, _set_info))| (job, info)).collect();
            (view! { <Main handle=handle() infos page/> }).into_view()
            // view! { <pre>{format!("{:#?}", evaluations)}</pre> }.into_view()
            // ().into_view()
        }
        _ => (view! { "Loading" }).into_view(),
    }
}
