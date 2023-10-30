#![feature(iter_next_chunk)]
#![feature(if_let_guard)]

use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use typhon_types::{handles, requests, responses};

mod handle_request;
use handle_request::*;
use std::rc::Rc;

use stylers::style;

mod evaluation;
mod log;
mod status;
mod stream;
pub use evaluation::Evaluation;
pub use log::Log;
pub use status::Status;
mod jobset;
pub mod routes;

mod prelude;

fn main() {
    leptos::mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    let styler_class = style! {
        :deep(body) {
            font-family: Roboto;
            font-weight: 300;
            margin: 0;
            padding: 0;
        }
    };
    view! { class=styler_class,
        <Router>
            <Style>{include_str!("../../target/main.css")}</Style>
            <nav></nav>
            <main>
                <routes::Router></routes::Router>
            </main>
        </Router>
    }
}

#[component(transparent)]
fn SuspenseRequest<S: 'static + Clone>(
    request: Resource<S, Result<responses::Response, responses::ResponseError>>,
    #[prop(optional, into)] fallback: ViewFn,
    children: ChildrenFn,
) -> impl IntoView {
    view! {
        <Suspense
            fallback
            children=Rc::new(move || match request.get() {
                Some(Ok(res)) => {
                    provide_context(res);
                    children()
                }
                _ => Fragment::from(view! { "Error :(" }.into_view()),
            })
        />
    }
}

#[component]
fn Project(name: String) -> impl IntoView {
    let info = create_resource(
        {
            let name = name.clone();
            move || name.clone()
        },
        |name: String| async move {
            use requests as req;
            use responses as res;
            let project = handles::Project { name };
            let Ok(res::Response::ProjectInfo(info)) =
                handle_request(&req::Request::Project(project.clone(), req::Project::Info)).await
            else {
                panic!()
            };
            info
        },
    );
    view! {
        <Suspense>
            <b>{format!("{:#?}", info.get())}</b>
        </Suspense>
    }
    // #[derive(PartialEq, Clone, Params)]
    // struct ProjectParams {
    //     id: String,
    // }
    // let params = use_params::<ProjectParams>().get().unwrap();
    // params.id
}

/// This is the page of an evaluation: project is defined, jobset as well.
#[component]
fn Test() -> impl IntoView {
    // use handles::*;
    // let project = Project { name: "hi".into() };
    // let jobset = Jobset {
    //     project,
    //     name: "main".into(),
    // };
    // let handle = Evaluation { jobset, num: 10 };
    // let (handle, _) = create_signal(handle);
    // view! { <Evaluation handle/> }
}

#[component]
fn Projects() -> impl IntoView {
    let projects = create_resource(
        || (),
        |_| async move { handle_request(&requests::Request::ListProjects).await },
    );
    view! {
        <SuspenseRequest
            request=projects

            fallback=move || {
                view! { <p>"Loading..."</p> }
            }
        >

            {
                let responses::Response::ListProjects(responses) = use_context::<
                    responses::Response,
                >()
                    .unwrap() else { panic!() };
                view! {
                    <table>
                        <tr>
                            <th>"Id"</th>
                            <th>"Name"</th>
                            <th>"Description"</th>
                        </tr>
                        {responses
                            .into_iter()
                            .map(|(handle, metadata)| {
                                view! {
                                    <tr>
                                        <td>
                                            <A href=String::from(
                                                routes::Root::Project(handle.clone()),
                                            )>{handle.name}</A>
                                        </td>
                                        <td>{metadata.title}</td>
                                        <td>{metadata.description}</td>
                                    </tr>
                                }
                            })
                            .collect_view()}

                    </table>
                }
            }

        </SuspenseRequest>
    }
}
