pub mod handles {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct Project {
        pub name: String,
    }
    impl Project {
        pub fn legal(&self) -> bool {
            use lazy_static::lazy_static;
            use regex::Regex;
            lazy_static! {
                static ref RE: Regex = Regex::new("^[A-z0-9-_]+$").unwrap();
            }
            RE.is_match(&self.name)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct Jobset {
        pub project: Project,
        pub name: String,
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct Evaluation {
        pub jobset: Jobset,
        pub num: i64,
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct Job {
        pub evaluation: Evaluation,
        pub system: String,
        pub name: String,
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct Log {
        pub identifier: crate::data::TaskIdentifier,
        pub job: Job,
    }

    macro_rules! impl_display {
        ($ty:ident) => {
            impl std::fmt::Display for $ty {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(f, "{}", Vec::<String>::from(self.clone()).join(":"))
                }
            }
        };
    }
    impl_display!(Project);
    impl From<Project> for Vec<String> {
        fn from(x: Project) -> Self {
            vec![x.name]
        }
    }
    impl_display!(Jobset);
    impl From<Jobset> for Vec<String> {
        fn from(x: Jobset) -> Self {
            [x.project.into(), vec![x.name]].concat()
        }
    }
    impl_display!(Evaluation);
    impl From<Evaluation> for Vec<String> {
        fn from(x: Evaluation) -> Self {
            [x.jobset.into(), vec![format!("{}", x.num)]].concat()
        }
    }
    impl_display!(Job);
    impl From<Job> for Vec<String> {
        fn from(x: Job) -> Self {
            [x.evaluation.into(), vec![x.system, x.name]].concat()
        }
    }
    impl_display!(Log);
    impl From<Log> for Vec<String> {
        fn from(x: Log) -> Self {
            use crate::data::{ActionIdentifier, TaskIdentifier};
            [
                x.job.into(),
                vec![match x.identifier {
                    TaskIdentifier::Action(ActionIdentifier::Begin) => "begin",
                    TaskIdentifier::Action(ActionIdentifier::End) => "end",
                    TaskIdentifier::Build => "build",
                }
                .to_string()],
            ]
            .concat()
        }
    }

    use crate::handles as selfmod;
    pub fn project(name: String) -> Project {
        Project { name }
    }
    pub fn jobset((project, name): (String, String)) -> Jobset {
        Jobset {
            project: selfmod::project(project),
            name,
        }
    }
    pub fn evaluation((project, jobset, num): (String, String, i64)) -> Evaluation {
        Evaluation {
            jobset: selfmod::jobset((project, jobset)),
            num,
        }
    }
    pub fn job(
        (project, jobset, evaluation, system, name): (String, String, i64, String, String),
    ) -> Job {
        Job {
            evaluation: selfmod::evaluation((project, jobset, evaluation)),
            system,
            name,
        }
    }
}

pub mod requests {
    use crate::handles;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct EvaluationSearch {
        pub jobset_name: Option<String>,
        pub limit: u8,
        pub offset: u32,
        pub project_name: Option<String>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct ProjectDecl {
        pub flake: bool,
        pub url: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum Project {
        Delete,
        Info,
        Refresh,
        SetDecl(ProjectDecl),
        SetPrivateKey(String),
        UpdateJobsets,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum Jobset {
        Evaluate(bool),
        Info,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum Evaluation {
        Cancel,
        Info,
        Log,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum Job {
        Cancel,
        Info,
        LogBegin,
        LogEnd,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum Request {
        SearchEvaluations(EvaluationSearch), // Rename to EvaluationSearch
        ListProjects,
        CreateProject { name: String, decl: ProjectDecl },
        Project(handles::Project, Project),
        Jobset(handles::Jobset, Jobset),
        Evaluation(handles::Evaluation, Evaluation),
        Job(handles::Job, Job),
        Login(String),
    }

    impl std::fmt::Display for Request {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                Request::SearchEvaluations(_) => write!(f, "Search through evaluations"),
                Request::ListProjects => write!(f, "List projects"),
                Request::CreateProject { name, decl } => {
                    write!(
                        f,
                        "Create{} project {} with url {}",
                        if !decl.flake { " legacy" } else { "" },
                        name,
                        decl.url
                    )
                }
                Request::Project(h, req) => write!(f, "{:?} for project {}", req, h),
                Request::Jobset(h, req) => write!(f, "{:?} for jobset {}", req, h),
                Request::Evaluation(h, req) => write!(f, "{:?} for evaluation {}", req, h),
                Request::Job(h, req) => write!(f, "{:?} for job {}", req, h),
                Request::Login(_) => write!(f, "Log in"),
            }
        }
    }
}

pub mod data {
    use crate::responses;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct JobSystemName {
        pub system: String,
        pub name: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct TimeRange {
        pub start: u64,
        pub end: u64,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum TaskStatusKind {
        Success,
        Pending,
        Error,
        Canceled,
    }

    impl From<TaskStatus> for TaskStatusKind {
        fn from(s: TaskStatus) -> Self {
            match s {
                TaskStatus::Success(..) => Self::Success,
                TaskStatus::Pending { .. } => Self::Pending,
                TaskStatus::Error(..) => Self::Error,
                TaskStatus::Canceled(..) => Self::Canceled,
            }
        }
    }

    impl core::cmp::PartialOrd for TaskStatusKind {
        fn partial_cmp(&self, rhs: &Self) -> Option<core::cmp::Ordering> {
            Some(self.cmp(rhs))
        }
    }
    impl core::cmp::Ord for TaskStatusKind {
        fn cmp(&self, rhs: &Self) -> core::cmp::Ordering {
            use core::cmp::Ordering;
            if self == rhs {
                return Ordering::Equal;
            }
            match (self, rhs) {
                (TaskStatusKind::Error, _) => Ordering::Greater,
                (_, TaskStatusKind::Error) => Ordering::Less,
                (TaskStatusKind::Pending, _) => Ordering::Greater,
                (_, TaskStatusKind::Pending) => Ordering::Less,
                (TaskStatusKind::Canceled, _) => Ordering::Greater,
                (_, TaskStatusKind::Canceled) => Ordering::Less,
                (TaskStatusKind::Success, TaskStatusKind::Success) => Ordering::Greater,
            }
        }
    }

    impl From<responses::JobInfo> for TaskStatusKind {
        fn from(job: responses::JobInfo) -> Self {
            let begin: TaskStatusKind = job.begin.status.into();
            let end: TaskStatusKind = job.end.status.into();
            let build: TaskStatusKind = job.build.status.into();
            begin.max(end).max(build)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum TaskStatus {
        Success(TimeRange),
        Pending { start: Option<u64> },
        Error(TimeRange),
        Canceled(Option<TimeRange>),
    }

    impl TaskStatus {
        pub fn tag(&self) -> &str {
            match self {
                TaskStatus::Success(..) => "success",
                TaskStatus::Pending { .. } => "pending",
                TaskStatus::Error(..) => "error",
                TaskStatus::Canceled(..) => "canceled",
            }
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum ActionIdentifier {
        Begin,
        End,
    }
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum TaskIdentifier {
        Action(ActionIdentifier),
        Build,
    }
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum TaskKind {
        Action { identifier: ActionIdentifier },
        Build { drv: String, out: String },
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct TaskRef {
        pub kind: TaskKind,
        pub status: TaskStatus,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct Build {
        pub drv: String,
        pub out: String,
        pub status: TaskStatus,
    }

    impl From<&TaskRef> for TaskIdentifier {
        fn from(tr: &TaskRef) -> Self {
            match tr.kind {
                TaskKind::Action { identifier } => TaskIdentifier::Action(identifier),
                TaskKind::Build { .. } => TaskIdentifier::Build,
            }
        }
    }
    impl From<Build> for TaskRef {
        fn from(Build { drv, out, status }: Build) -> Self {
            Self {
                kind: TaskKind::Build { drv, out },
                status,
            }
        }
    }

    impl From<Action> for TaskRef {
        fn from(Action { identifier, status }: Action) -> Self {
            Self {
                kind: TaskKind::Action { identifier },
                status,
            }
        }
    }

    impl From<Job> for [TaskRef; 3] {
        fn from(job: Job) -> Self {
            [job.begin.into(), job.build.into(), job.end.into()]
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct Action {
        pub identifier: ActionIdentifier,
        pub status: TaskStatus,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct Job {
        pub begin: Action,
        pub build: Build,
        pub end: Action,
        pub dist: bool,
        pub name: JobSystemName,
        pub time_created: u64,
    }
}

pub mod responses {
    pub use super::data::Job as JobInfo;
    pub use super::data::{TaskStatus, TimeRange};
    use crate::handles;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct ProjectMetadata {
        #[serde(default)]
        pub description: String,
        #[serde(default)]
        pub homepage: String,
        #[serde(default)]
        pub title: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct ProjectInfo {
        pub actions_path: Option<String>,
        pub flake: bool,
        pub jobsets: Vec<String>,
        pub metadata: ProjectMetadata,
        pub public_key: String,
        pub url: String,
        pub url_locked: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct JobsetInfo {
        pub last_evaluation: Option<(handles::Evaluation, EvaluationInfo<()>)>,
        pub evaluations_count: u32,
        pub flake: bool,
        pub url: String,
    }

    pub use crate::data::JobSystemName;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub struct EvaluationInfo<Jobs = Vec<JobSystemName>> {
        pub actions_path: Option<String>,
        pub flake: bool,
        pub jobs: Jobs,
        pub status: TaskStatus,
        pub time_created: i64,
        pub time_finished: Option<i64>,
        pub url: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum Response {
        Ok,
        SearchEvaluations(Vec<(handles::Evaluation, EvaluationInfo<()>)>),
        ListProjects(Vec<(handles::Project, ProjectMetadata)>),
        ProjectInfo(ProjectInfo),
        ProjectUpdateJobsets(Vec<String>),
        JobsetEvaluate(crate::handles::Evaluation),
        JobsetInfo(JobsetInfo),
        EvaluationInfo(EvaluationInfo),
        JobInfo(JobInfo),
        Log(Option<String>),
        Login { token: String },
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    pub enum ResponseError {
        BadRequest(String),
        InternalError,
        ResourceNotFound(String),
    }

    impl std::fmt::Display for ResponseError {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                ResponseError::BadRequest(e) => write!(f, "Bad request: {}", e),
                ResponseError::InternalError => write!(f, "Internal server error"),
                ResponseError::ResourceNotFound(e) => write!(f, "Resource not found: {}", e),
            }
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Event {
    ProjectNew(handles::Project),
    ProjectDeleted(handles::Project),
    ProjectJobsetsUpdated(handles::Project),
    ProjectUpdated(handles::Project),
    EvaluationNew(handles::Evaluation),
    EvaluationFinished(handles::Evaluation),
    JobUpdated(handles::Job),
}
