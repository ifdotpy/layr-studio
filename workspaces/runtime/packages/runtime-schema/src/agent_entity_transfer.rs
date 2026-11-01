//! Public account transfer summary projected onto an agent.
use crate::{AgentCounter, ProviderKind, queue_command::Field};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TransferStatus {
    Pending,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TransferScope {
    #[default]
    Team,
    Subagents,
}

// Both blocked and interrupted entries have the same public shape.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountTransferNoticeDto {
    pub id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub name: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub reason: Field<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountTransferLeftOnSourceDto {
    pub id: String,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub name: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub reason: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<ProviderKind>")]
    pub provider: Field<ProviderKind>,
}

fn nullable_counter(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({"anyOf":[{"type":"integer","minimum":0,"maximum":crate::shared_values::MAX_SAFE_INTEGER},{"type":"null"}]})
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountTransferSummaryDto {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub target_account_key: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<TransferStatus>")]
    pub status: Field<TransferStatus>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<TransferScope>")]
    pub scope: Field<TransferScope>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<f64>")]
    pub updated: Field<f64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub finish_history: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "nullable_counter")]
    pub total: Field<AgentCounter>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "nullable_counter")]
    pub completed: Field<AgentCounter>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "nullable_counter")]
    pub moved: Field<AgentCounter>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "nullable_counter")]
    pub native_history_pending: Field<AgentCounter>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "nullable_counter")]
    pub moving_now: Field<AgentCounter>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub can_finish_history: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(schema_with = "nullable_counter")]
    pub waiting_count: Field<AgentCounter>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub needs_attention: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<bool>")]
    pub can_retry: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<AccountTransferNoticeDto>>")]
    pub interrupted: Field<Vec<AccountTransferNoticeDto>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<AccountTransferLeftOnSourceDto>>")]
    pub left_on_source: Field<Vec<AccountTransferLeftOnSourceDto>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<Vec<AccountTransferNoticeDto>>")]
    pub blocked: Field<Vec<AccountTransferNoticeDto>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    #[schemars(with = "Option<String>")]
    pub waiting: Field<String>,
}

fn nullable_ref(
    schema: &mut serde_json::Value,
    name: &str,
    from: &str,
    to: &str,
) -> Result<(), String> {
    use crate::agent_entity_nested::{object, property};
    let prop = property(schema, name)?;
    let variants = object(prop)?
        .get_mut("anyOf")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| format!("{name} must be nullable"))?;
    if variants.len() != 2
        || variants[0]["$ref"] != from
        || variants[1] != serde_json::json!({"type":"null"})
    {
        return Err(format!("{name} must reference its owner and null"));
    }
    variants[0]["$ref"] = to.into();
    object(prop)?.insert("default".into(), serde_json::Value::Null);
    Ok(())
}

fn nullable_list(
    schema: &mut serde_json::Value,
    name: &str,
    label: &str,
    from: &str,
    to: &str,
) -> Result<(), String> {
    use crate::agent_entity_nested::{object, property, title};
    let prop = property(schema, name)?;
    let mut item = prop.clone();
    if item["type"] != serde_json::json!(["array", "null"]) || item["items"]["$ref"] != from {
        return Err(format!("{name} must contain its owner and null"));
    }
    item["type"] = "array".into();
    item["items"]["$ref"] = to.into();
    *prop = serde_json::json!({"anyOf":[item,{"type":"null"}]});
    object(prop)?.insert("default".into(), serde_json::Value::Null);
    title(prop, label)
}

fn notice_schema<T: JsonSchema>(
    generator: &mut schemars::SchemaGenerator,
    name: &str,
) -> Result<serde_json::Value, String> {
    use crate::agent_entity_nested::{nullable, property, title};
    let mut value = T::json_schema(generator).to_value();
    title(&mut value, name)?;
    title(property(&mut value, "id")?, "Id")?;
    nullable(&mut value, "name", "Name", true)?;
    nullable(&mut value, "reason", "Reason", true)?;
    Ok(value)
}

pub(crate) fn schemas() -> Result<[(&'static str, serde_json::Value); 6], String> {
    use crate::agent_entity_nested::{nullable, object, property, title};
    let mut g = schemars::generate::SchemaSettings::openapi3()
        .for_serialize()
        .with(|s| s.definitions_path = "/components/schemas".into())
        .into_generator();
    let blocked = notice_schema::<AccountTransferNoticeDto>(&mut g, "AccountTransferBlockedDto")?;
    let interrupted =
        notice_schema::<AccountTransferNoticeDto>(&mut g, "AccountTransferInterruptedDto")?;
    let mut left =
        notice_schema::<AccountTransferLeftOnSourceDto>(&mut g, "AccountTransferLeftOnSourceDto")?;
    nullable_ref(
        &mut left,
        "provider",
        "#/components/schemas/ProviderKind",
        "#/components/schemas/AgentProvider",
    )?;
    let mut scope = TransferScope::json_schema(&mut g).to_value();
    title(&mut scope, "AccountTransferScope")?;
    let mut status = TransferStatus::json_schema(&mut g).to_value();
    title(&mut status, "AccountTransferStatus")?;
    let mut summary = AccountTransferSummaryDto::json_schema(&mut g).to_value();
    title(&mut summary, "AccountTransferSummaryDto")?;
    object(&mut summary)?.insert(
        "description".into(),
        "Public summary projected onto an agent by AccountTransfers.save.".into(),
    );
    for (name, label) in [
        ("id", "Id"),
        ("targetAccountKey", "Targetaccountkey"),
        ("updated", "Updated"),
        ("finishHistory", "Finishhistory"),
        ("canFinishHistory", "Canfinishhistory"),
        ("needsAttention", "Needsattention"),
        ("canRetry", "Canretry"),
        ("waiting", "Waiting"),
    ] {
        nullable(&mut summary, name, label, true)?;
    }
    for (name, label) in [
        ("total", "Total"),
        ("completed", "Completed"),
        ("moved", "Moved"),
        ("nativeHistoryPending", "Nativehistorypending"),
        ("movingNow", "Movingnow"),
        ("waitingCount", "Waitingcount"),
    ] {
        let prop = property(&mut summary, name)?;
        let variants = object(prop)?
            .get("anyOf")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{name} must be a nullable counter"))?;
        if variants.len() != 2
            || variants[0]["maximum"] != crate::shared_values::MAX_SAFE_INTEGER
            || variants[1] != serde_json::json!({"type":"null"})
        {
            return Err(format!("{name} must use the safe counter"));
        }
        object(prop)?.insert("default".into(), serde_json::Value::Null);
        title(prop, label)?;
    }
    for (name, from, to) in [
        ("scope", "TransferScope", "AccountTransferScope"),
        ("status", "TransferStatus", "AccountTransferStatus"),
    ] {
        nullable_ref(
            &mut summary,
            name,
            &format!("#/components/schemas/{from}"),
            &format!("#/components/schemas/{to}"),
        )?;
    }
    for (name, label, from, to) in [
        (
            "blocked",
            "Blocked",
            "AccountTransferNoticeDto",
            "AccountTransferBlockedDto",
        ),
        (
            "interrupted",
            "Interrupted",
            "AccountTransferNoticeDto",
            "AccountTransferInterruptedDto",
        ),
        (
            "leftOnSource",
            "Leftonsource",
            "AccountTransferLeftOnSourceDto",
            "AccountTransferLeftOnSourceDto",
        ),
    ] {
        nullable_list(
            &mut summary,
            name,
            label,
            &format!("#/components/schemas/{from}"),
            &format!("#/components/schemas/{to}"),
        )?;
    }
    Ok([
        ("AccountTransferBlockedDto", blocked),
        ("AccountTransferInterruptedDto", interrupted),
        ("AccountTransferLeftOnSourceDto", left),
        ("AccountTransferScope", scope),
        ("AccountTransferStatus", status),
        ("AccountTransferSummaryDto", summary),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn schemas_match_published_components() {
        let document: serde_json::Value =
            serde_json::from_str(include_str!("../../runtime-http/openapi.json")).unwrap();
        for (name, schema) in schemas().unwrap() {
            assert_eq!(document["components"]["schemas"][name], schema, "{name}");
        }
    }
    #[derive(JsonSchema)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct ChangedNotice {
        id: bool,
        name: Option<String>,
        reason: Option<String>,
    }
    #[test]
    fn field_type_change_changes_published_schema() {
        let witness = ChangedNotice {
            id: true,
            name: None,
            reason: None,
        };
        assert!(witness.id && witness.name.is_none() && witness.reason.is_none());
        let mut g = schemars::generate::SchemaSettings::openapi3()
            .for_serialize()
            .into_generator();
        let changed = notice_schema::<ChangedNotice>(&mut g, "AccountTransferBlockedDto").unwrap();
        let published = schemas()
            .unwrap()
            .into_iter()
            .find(|(name, _)| *name == "AccountTransferBlockedDto")
            .unwrap()
            .1;
        assert_ne!(changed["properties"]["id"], published["properties"]["id"]);
    }
    #[test]
    fn summary_preserves_missing_null_and_rejects_invalid_counters() {
        let value =
            json!({"id":null,"blocked":[{"id":"a","name":null}],"status":"pending","total":2});
        let summary: AccountTransferSummaryDto = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(summary).unwrap(), value);
        for bad in [
            json!({"total":-1}),
            json!({"status":"unknown"}),
            json!({"blocked":[{"id":"a","provider":"codex"}]}),
        ] {
            assert!(serde_json::from_value::<AccountTransferSummaryDto>(bad).is_err());
        }
    }
}
