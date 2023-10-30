use futures::future::FutureExt;
use futures_core::stream::Stream;
use futures_util::stream::StreamExt;
use gloo_net::http;
use wasm_bindgen::JsCast;
use wasm_streams::readable::*;

pub fn fetch_as_stream(req: http::Request) -> impl Stream<Item = String> {
    async move {
        let res = req
            .send()
            .await
            .map_err(|e| gloo_console::log!(format!("network error {:?}", e)))
            .unwrap();
        let body = res.body();
        let readable_stream: web_sys::ReadableStream = body.unwrap();
        let readable_stream: sys::ReadableStream = readable_stream.unchecked_into();
        let readable_stream: ReadableStream = ReadableStream::from_raw(readable_stream);
        readable_stream.into_stream().map(|item| {
            let text_decoder = web_sys::TextDecoder::new().unwrap();
            let item = text_decoder
                .decode_with_buffer_source(&item.unwrap().into())
                .unwrap();
            item.strip_suffix("\n")
                .map(|s| s.to_owned())
                .unwrap_or(item)
        })
    }
    .into_stream()
    .flatten()
}
pub fn fetch_as_signal(req: http::Request) -> leptos::ReadSignal<Option<String>> {
    leptos::create_signal_from_stream(Box::pin(fetch_as_stream(req)))
}
