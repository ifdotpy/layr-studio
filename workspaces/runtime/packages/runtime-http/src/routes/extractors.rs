use axum::extract::{FromRequest, FromRequestParts};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

use crate::error::ApiError;
use crate::validation::ValidatedRequest;

fn serde_location(section: &str, path: &serde_path_to_error::Path) -> Vec<Value> {
    let mut location = vec![Value::String(section.to_owned())];
    location.extend(path.iter().filter_map(|segment| match segment {
        serde_path_to_error::Segment::Seq { index } => Some(Value::String(index.to_string())),
        serde_path_to_error::Segment::Map { key } => Some(Value::String(key.clone())),
        serde_path_to_error::Segment::Enum { variant } => Some(Value::String(variant.clone())),
        serde_path_to_error::Segment::Unknown => None,
    }));
    location
}

/// The authenticated caller, set by the boundary for every API request.
#[allow(dead_code)] // Handlers that need the caller extract it; the edge already checked the class.
pub(crate) struct Caller(pub studio_runtime_services::principal::Principal);

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<studio_runtime_services::principal::Principal>()
            .cloned()
            .map(Self)
            .ok_or_else(ApiError::internal)
    }
}

/// JSON request bodies decoded directly from the wire bytes.
pub(crate) struct ApiJson<T>(pub T);

/// Strict, typed query parameters decoded directly from the request URI.
pub(crate) struct TypedQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for TypedQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Query::<T>::try_from_uri(&parts.uri)
            .map(|axum::extract::Query(value)| Self(value))
            .map_err(|_| ApiError::validation(vec![Value::String("query".into())]))
    }
}

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(
        request: axum::extract::Request,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let limit = crate::http_body_limits::request_limit(request.uri().path());
        let (parts, body) = request.into_parts();
        let declared = parts
            .headers
            .get(axum::http::header::CONTENT_LENGTH)
            .map(|value| {
                value
                    .to_str()
                    .ok()
                    .and_then(|text| text.parse::<usize>().ok())
                    .ok_or_else(|| {
                        ApiError::new(axum::http::StatusCode::BAD_REQUEST, "Invalid request size")
                    })
            })
            .transpose()?;
        if declared.is_some_and(|size| size > limit) {
            return Err(ApiError::new(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "Invalid request size",
            ));
        }
        let bytes = tokio::time::timeout(
            std::time::Duration::from_secs(
                crate::http_body_limits::REQUEST_BODY_READ_TIMEOUT_SECONDS,
            ),
            axum::body::to_bytes(body, limit),
        )
        .await
        .map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::REQUEST_TIMEOUT,
                "Request body timed out",
            )
        })?
        .map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                "Invalid request size",
            )
        })?;
        if declared.is_some_and(|size| size != bytes.len()) {
            return Err(ApiError::new(
                axum::http::StatusCode::BAD_REQUEST,
                "Incomplete request",
            ));
        }
        if !bytes.is_empty()
            && !parts
                .headers
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
        {
            return Err(ApiError::json_transport(
                axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "JSON required",
            ));
        }
        let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
        let studio_runtime_schema::http_ingress::Object(value) =
            serde_path_to_error::deserialize(&mut deserializer)
                .map_err(|error| ApiError::validation(serde_location("body", error.path())))?;
        deserializer
            .end()
            .map_err(|_| ApiError::validation(vec![Value::String("body".into())]))?;
        Ok(Self(value))
    }
}

/// Request headers, available without middleware-provided values.
pub(crate) struct ApiHeaders(pub axum::http::HeaderMap);

impl ApiHeaders {
    pub(crate) fn one(&self, name: &'static str) -> Result<Option<&str>, ApiError> {
        let mut values = self.0.get_all(name).iter();
        let first = values.next();
        if values.next().is_some() {
            return Err(ApiError::validation(vec![
                Value::String("header".into()),
                Value::String(name.into()),
            ]));
        }
        first
            .map(|value| {
                value.to_str().map_err(|_| {
                    ApiError::validation(vec![
                        Value::String("header".into()),
                        Value::String(name.into()),
                    ])
                })
            })
            .transpose()
    }
}

impl<S> FromRequestParts<S> for ApiHeaders
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(parts.headers.clone()))
    }
}

/// Raw query pairs plus values validated and typed from the registered schema.
pub(crate) struct QueryParams {
    typed: Arc<serde_json::Map<String, Value>>,
}

impl QueryParams {
    pub(crate) fn typed_value(&self, name: &str) -> Option<&Value> {
        self.typed.get(name)
    }

    pub(crate) fn nonempty_str(&self, name: &str) -> Option<&str> {
        self.typed_value(name)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn into_typed(self) -> Arc<serde_json::Map<String, Value>> {
        self.typed
    }
}

impl<S> FromRequestParts<S> for QueryParams
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let typed = parts
            .extensions
            .get::<ValidatedRequest>()
            .ok_or_else(ApiError::internal)?
            .query
            .clone();
        Ok(Self { typed })
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use axum::middleware::{Next, from_fn};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use serde::Deserialize;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::validation::ValidatedRequest;

    use super::{ApiJson, QueryParams};

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StrictQuery {
        limit: Option<studio_runtime_schema::bounded_integer::BoundedInteger<0, 10>>,
        enabled: Option<bool>,
    }

    async fn strict_query_handler(
        super::TypedQuery(query): super::TypedQuery<StrictQuery>,
    ) -> Json<Value> {
        Json(json!({"limit": query.limit.map(|v| v.get()), "enabled": query.enabled}))
    }

    async fn header_handler(
        headers: super::ApiHeaders,
    ) -> Result<Json<Value>, crate::error::ApiError> {
        Ok(Json(json!({"range": headers.one("range")?})))
    }

    #[tokio::test]
    async fn headers_are_read_without_middleware_and_duplicates_are_refused() {
        let app = Router::new().route("/items", get(header_handler));
        let valid = Request::builder()
            .uri("/items")
            .header("range", "bytes=0-2")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            response(app.clone(), valid).await,
            (StatusCode::OK, json!({"range":"bytes=0-2"}))
        );
        let duplicate = Request::builder()
            .uri("/items")
            .header("range", "bytes=0-2")
            .header("range", "bytes=3-4")
            .body(Body::empty())
            .unwrap();
        assert_eq!(response(app, duplicate).await.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn typed_query_decodes_without_middleware_and_rejects_invalid_values() {
        let app = Router::new().route("/items", get(strict_query_handler));
        assert_eq!(
            response(
                app.clone(),
                request("/items?limit=4&enabled=true", Body::empty())
            )
            .await,
            (StatusCode::OK, json!({"limit":4,"enabled":true}))
        );
        for query in [
            "limit=11",
            "limit=1.5",
            "enabled=maybe",
            "extra=x",
            "limit=1&limit=2",
        ] {
            assert_eq!(
                response(
                    app.clone(),
                    request(&format!("/items?{query}"), Body::empty())
                )
                .await
                .0,
                StatusCode::BAD_REQUEST,
                "{query}"
            );
        }
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct CountBody {
        count: u32,
    }

    async fn json_handler(ApiJson(body): ApiJson<CountBody>) -> Json<Value> {
        Json(json!({"count": body.count}))
    }

    async fn query_handler(params: QueryParams) -> Json<Value> {
        Json(json!({"typed": params.typed_value("value")}))
    }

    async fn response(router: Router, request: Request<Body>) -> (StatusCode, Value) {
        let response = router.oneshot(request).await.expect("router responds");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body reads");
        (
            status,
            serde_json::from_slice(&bytes).expect("response is JSON"),
        )
    }

    fn request(uri: &str, body: Body) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .body(body)
            .expect("request builds")
    }

    fn with_validated(router: Router, validated: ValidatedRequest) -> Router {
        router.layer(from_fn(move |mut request: Request<Body>, next: Next| {
            let validated = validated.clone();
            async move {
                request.extensions_mut().insert(validated);
                next.run(request).await
            }
        }))
    }

    #[tokio::test]
    async fn query_params_expose_only_the_validated_value() {
        let app = with_validated(
            Router::new().route("/items", get(query_handler)),
            ValidatedRequest {
                query: Arc::new(serde_json::Map::from_iter([("value".to_owned(), json!(1))])),
                ..ValidatedRequest::default()
            },
        );
        let (status, value) = response(app, request("/items?value=999", Body::empty())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(value, json!({"typed":1}));

        let app = with_validated(
            Router::new().route("/items", get(query_handler)),
            ValidatedRequest::default(),
        );
        let (status, value) = response(app, request("/items?value=999", Body::empty())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value, json!({"typed":null}));
    }

    #[tokio::test]
    async fn json_handler_decodes_wire_bytes_without_middleware() {
        let app = Router::new().route("/items", post(json_handler));
        let request = Request::builder()
            .method("POST")
            .uri("/items")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"count":2}"#))
            .expect("request builds");
        assert_eq!(
            response(app, request).await,
            (StatusCode::OK, json!({"count":2}))
        );
    }

    #[tokio::test]
    async fn wrongly_typed_json_uses_one_located_error() {
        let app = Router::new().route("/items", post(json_handler));
        let request = Request::builder()
            .method("POST")
            .uri("/items")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"count":"wrong"}"#))
            .expect("request builds");
        let (status, value) = response(app, request).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(value["error"], "Invalid request");
        assert_eq!(value["location"], json!(["body", "count"]));
    }

    #[tokio::test]
    async fn json_handler_rejects_positional_struct_arrays() {
        let app = Router::new().route("/items", post(json_handler));
        let request = Request::builder()
            .method("POST")
            .uri("/items")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("[2]"))
            .expect("request builds");
        let (status, value) = response(app, request).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(value["location"], json!(["body"]));
    }
}
inventory::submit! {
    crate::routes::manifest::RouteModuleRegistration {
        name: "extractors",
        routes: None,
        dependencies: &[],
    }
}
