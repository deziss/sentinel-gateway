use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};

/// Wrapper for reqwest::HeaderMap to implement OpenTelemetry Injector.
/// Used to propagate trace context to upstream backends.
pub struct HeaderInjector<'a>(pub &'a mut reqwest::header::HeaderMap);

impl<'a> Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&value) {
                self.0.insert(name, val);
            }
        }
    }
}

/// Wrapper for axum::http::HeaderMap to implement OpenTelemetry Extractor.
/// Used to extract trace context from incoming requests.
pub struct HeaderExtractor<'a>(pub &'a axum::http::HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Inject the current trace context into outgoing headers.
pub fn inject_trace_context(headers: &mut reqwest::header::HeaderMap) {
    let cx = opentelemetry::Context::current();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut HeaderInjector(headers));
    });
}

/// Extract trace context from incoming request headers.
pub fn extract_trace_context(headers: &axum::http::HeaderMap) -> opentelemetry::Context {
    global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(headers))
    })
}
