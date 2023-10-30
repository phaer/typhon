use serde::{Deserialize, Serialize};
use typhon_types::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub api_url: String,
}

impl Settings {
    pub fn load() -> Self {
        fn try_load() -> Option<Settings> {
            serde_json::from_str::<Option<Settings>>(
                &gloo_utils::document()
                    .query_selector("script[id='settings']")
                    .ok()??
                    .inner_html(),
            )
            .ok()?
        }
        try_load().unwrap_or_default()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_url: "http://127.0.0.1:8000/api".into(),
        }
    }
}

pub fn get_token() -> Option<String> {
    // use gloo_storage::Storage;
    // gloo_storage::LocalStorage::get("typhon_token").ok()
    Some("password".into())
}

pub async fn handle_request(
    request: &requests::Request,
) -> Result<responses::Response, responses::ResponseError> {
    use gloo_net::http;
    let settings = Settings::load();
    let token = get_token();
    let mut req = http::RequestBuilder::new(&settings.api_url).method(http::Method::POST);
    if let Some(token) = token {
        req = req.header("token", &token)
    }
    req.json(request)
        .unwrap()
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
