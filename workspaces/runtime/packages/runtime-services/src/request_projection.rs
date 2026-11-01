//! Typed renderer projection for request cards.
use serde::Serialize;
use studio_runtime_schema::queue_command::Field;
use studio_runtime_store::RequestRecord;

use studio_runtime_schema::request_projection::{
    Command, RequestParams, RequestPreview, RequestSchemaProperty,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestProjection {
    id: String,
    agent: String,
    method: studio_runtime_schema::request_record::RequestMethod,
    status: studio_runtime_schema::persistence::RequestCardStatus,
    created_at: f64,
    params: RequestParams,
    #[serde(skip_serializing_if = "Option::is_none")]
    connection_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    epoch: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<RequestPreview>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deferred: Option<bool>,
}

use crate::sync_views::bound_sync_text as text;
fn bound(value: &mut Field<String>, field: &str) {
    if let Field::Value(value) = value {
        *value = text(value, field);
    }
}
fn bound_union(value: &mut Field<Command>, field: &str) {
    if let Field::Value(value) = value {
        match value {
            Command::Text(s) => *s = text(s, field),
            Command::Arguments(items) => {
                items.truncate(crate::sync_views::SYNC_ARRAY_ITEMS);
                for s in items {
                    *s = text(s, "");
                }
            }
        }
    }
}
fn bound_schema_property(property: &mut RequestSchemaProperty) {
    bound(&mut property.format, "format");
    bound(&mut property.title, "title");
    bound_union(&mut property.kind, "type");
    if let Field::Value(variants) = &mut property.variants {
        variants.truncate(crate::sync_views::SYNC_ARRAY_ITEMS);
    }
    if let Field::Value(items) = &mut property.items {
        bound_schema_property(items);
    }
}
fn bound_params(params: &mut RequestParams) {
    bound_union(&mut params.command, "command");
    for (value, field) in [
        (&mut params.client_id, "clientId"),
        (&mut params.cwd, "cwd"),
        (&mut params.host, "host"),
        (&mut params.linux_user, "linuxUser"),
        (&mut params.message, "message"),
        (&mut params.phase, "phase"),
        (&mut params.reason, "reason"),
        (&mut params.scope, "scope"),
        (&mut params.source, "source"),
        (&mut params.thread_id, "threadId"),
        (&mut params.turn_id, "turnId"),
        (&mut params.url, "url"),
        (&mut params.user_code, "userCode"),
    ] {
        bound(value, field);
    }
    if let Field::Value(items) = &mut params.questions {
        items.truncate(crate::sync_views::SYNC_ARRAY_ITEMS);
        for q in items {
            q.id = text(&q.id, "id");
            q.question = text(&q.question, "question");
            if let Field::Value(options) = &mut q.options {
                options.truncate(crate::sync_views::SYNC_ARRAY_ITEMS);
                for option in options {
                    bound(&mut option.description, "description");
                    bound(&mut option.label, "label");
                }
            }
        }
    }
    if let Field::Value(schema) = &mut params.requested_schema {
        for property in schema.properties.values_mut() {
            bound_schema_property(property);
        }
        if let Field::Value(required) = &mut schema.required {
            required.truncate(crate::sync_views::SYNC_ARRAY_ITEMS);
            for s in required {
                *s = text(s, "");
            }
        }
    }
}
fn bound_preview(preview: &mut RequestPreview) {
    bound_union(&mut preview.command, "command");
    bound(&mut preview.cwd, "cwd");
    bound(&mut preview.url, "url");
}

pub fn project_request_record(
    record: &RequestRecord,
) -> Result<RequestProjection, serde_json::Error> {
    if record.id.is_empty() {
        return Err(serde::de::Error::custom("request id is empty"));
    }
    let mut params: RequestParams = serde_json::from_str(record.params.get())?;
    let mut preview: Option<RequestPreview> = record
        .preview
        .as_ref()
        .map(|v| serde_json::from_str(v.get()))
        .transpose()?;
    bound_params(&mut params);
    if let Some(value) = &mut preview {
        bound_preview(value);
    }
    Ok(RequestProjection {
        id: text(&record.id, "id"),
        agent: text(&record.agent, "agent"),
        method: record.method,
        status: record.status,
        created_at: record.created_at,
        params,
        connection_id: record
            .connection_id
            .as_deref()
            .map(|v| text(v, "connectionId")),
        account_key: record.account_key.as_deref().map(|v| text(v, "accountKey")),
        epoch: record.epoch,
        turn_id: record.turn_id.as_deref().map(|v| text(v, "turnId")),
        preview,
        deferred: record.deferred,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_projection_matches_existing_projection() {
        let source = r#"{"id":"r","agent":"a","method":"item/tool/requestUserInput","status":"pending","createdAt":1.0,"params":{"threadId":"t","command":["echo","ok"],"questions":[{"id":"q","question":"Continue?","header":"Confirm","options":[{"label":"Yes","description":"Proceed","metadata":{"n":1}}]}],"requestedSchema":{"properties":{"thing":{"type":"string","description":"Help"}},"required":["thing"]},"private":"hidden"},"preview":{"command":"echo ok","cwd":"/tmp","private":true},"accountKey":"account"}"#;
        let record: RequestRecord = serde_json::from_str(source).unwrap();
        let encoded = serde_json::to_vec(&project_request_record(&record).unwrap()).unwrap();
        let actual: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        // Snapshot of the generic projection before removing its request branch.
        let expected = json!({"id":"r","agent":"a","method":"item/tool/requestUserInput","status":"pending","createdAt":1.0,"params":{"threadId":"t","command":["echo","ok"],"questions":[{"id":"q","question":"Continue?","header":"Confirm","options":[{"label":"Yes","description":"Proceed","metadata":{"n":1}}]}],"requestedSchema":{"properties":{"thing":{"type":"string","description":"Help"}},"required":["thing"]}},"preview":{"command":"echo ok","cwd":"/tmp"},"accountKey":"account"});
        assert_eq!(actual, expected);
    }

    #[test]
    fn request_projection_refuses_invalid_known_fields() {
        let source = r#"{"id":"r","agent":"a","method":"item/tool/requestUserInput","status":"pending","createdAt":1.0,"params":{"command":42}}"#;
        assert!(serde_json::from_str::<RequestRecord>(source).is_err());
    }
}
