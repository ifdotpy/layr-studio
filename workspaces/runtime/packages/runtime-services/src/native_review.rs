//! Native review children use the Python request-key UUID, never a random child
//! on retry. Workspace/catalog checks stay with the ordinary creation owner.
use crate::native_actions::{NativeError, Result, bounded, checked_agent, decode};
use rusqlite::Transaction;
use studio_json::{Value, json};
use studio_runtime_schema::creation_intent::EffortSelection;
use studio_runtime_schema::review::{
    GitCommit, GitRef, LayrStateId, ReviewAssignment, ReviewTarget,
};
use studio_runtime_store::{AgentRecord, AgentRepository, ContextRepairRepository};

const MAX_PROMPT: usize = 32_000;
const DELIVERY: &str = "The review result wakes you automatically. Finish your turn while waiting.";
/// Refusal shared by service and HTTP review-model validation.
pub const REVIEW_MODEL_REQUIRED: &str = "Select a review model in chat settings";

#[cfg(test)]
mod explicit_review_model_tests {
    use super::*;

    #[test]
    fn missing_review_model_refuses_instead_of_using_the_caller() {
        let mut actor = AgentRecord::test_defaults();
        actor.model = Some("gpt-6-luna".into());
        actor.review_defaults = None;
        let result = execution_settings(
            &actor,
            &actor,
            &studio_runtime_schema::queue_command::Field::Missing,
            &EffortSelection::Inherit,
        );
        assert_eq!(
            result,
            Err(
                crate::managed_child_create::ManagedCreationEnvironmentError::Invalid(
                    REVIEW_MODEL_REQUIRED
                )
            )
        );
    }

    #[test]
    fn requested_review_model_precedes_the_stored_chat_choice() {
        use studio_runtime_schema::queue_command::Field;
        let mut actor = AgentRecord::test_defaults();
        actor.model = Some("gpt-6-luna".into());
        let mut root = actor.clone();
        root.review_defaults = Some(Box::new(
            studio_runtime_schema::agent_entity_remote::ReviewDefaultsDto {
                model: Some("gpt-6-astra".into()),
                effort: None,
                fast_mode: None,
            },
        ));
        for (requested, expected) in [
            (Field::Missing, "gpt-6-astra"),
            (Field::Value("gpt-6.1-sol".into()), "gpt-6.1-sol"),
        ] {
            let resolved =
                execution_settings(&actor, &root, &requested, &EffortSelection::Inherit).unwrap();
            assert_eq!(resolved.0, expected);
        }
    }
}

/// Validate and normalize the complete typed review target.
pub fn target(args: &Value) -> Result<Value> {
    Ok(json!(typed_target(args)?))
}
fn typed_target(args: &Value) -> Result<ReviewTarget> {
    let request: studio_runtime_schema::review::ReviewRequest = decode(args.clone(), "review")?;
    let object = args.as_object().ok_or(NativeError::Invalid("review"))?;
    if let Some(value) = object.get("request_id") {
        let text = value
            .as_str()
            .ok_or(NativeError::Invalid("review.options"))?;
        if text.trim().is_empty()
            || text.chars().count() > 200
            || text.trim() != text
            || text.chars().any(|character| character <= '\u{1f}')
        {
            return Err(NativeError::Invalid("review.options"));
        }
    }
    Ok(request.target)
}
pub fn worker_id(key: &str) -> String {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, key.as_bytes()).to_string()
}

/// Run a bounded, non-interactive Git read without inherited Git overrides.
pub(crate) fn git_read(
    cwd: &std::path::Path,
    args: &[&str],
    limit: usize,
) -> Result<Option<String>> {
    let output = crate::bounded_child::run_without_git_environment(crate::bounded_child::Request {
        program: std::ffi::OsStr::new("git"),
        git_directory: Some(cwd.as_os_str()),
        args,
        environment: &[
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_NO_LAZY_FETCH", "1"),
        ],
        input: None,
        stdout_limit: limit,
        stderr_limit: 4096,
        timeout: std::time::Duration::from_secs(30),
    })
    .map_err(|_| NativeError::Invalid("review.git"))?;
    if output.stdout.truncated {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout.bytes).into_owned(),
    ))
}

/// Model and reasoning fallback for both managed reviewer spawns and native reviews.
/// Explicit effort (including null) wins. A different explicit model uses its
/// native effort; an omitted model uses only the chat's explicit review choice.
pub(crate) fn execution_settings(
    actor: &AgentRecord,
    root: &AgentRecord,
    requested_model: &studio_runtime_schema::queue_command::Field<String>,
    requested_effort: &EffortSelection,
) -> std::result::Result<
    (String, Option<String>, Option<bool>),
    crate::managed_child_create::ManagedCreationEnvironmentError,
> {
    let defaults = root.review_defaults.as_deref();
    let default_model = defaults.and_then(|defaults| defaults.model.as_deref());
    let model = requested_model
        .value()
        .map(String::as_str)
        .or(default_model)
        .filter(|model| !model.is_empty())
        .ok_or(
            crate::managed_child_create::ManagedCreationEnvironmentError::Invalid(
                REVIEW_MODEL_REQUIRED,
            ),
        )?
        .to_owned();
    let effort = if !requested_effort.is_inherit() {
        requested_effort.selected().cloned()
    } else if requested_model.is_missing() && default_model.is_some() {
        defaults.and_then(|defaults| defaults.effort.clone())
    } else if Some(model.as_str()) == actor.model.as_deref() {
        actor.effort.value().cloned()
    } else {
        None
    };
    Ok((
        model,
        effort,
        defaults.and_then(|defaults| defaults.fast_mode),
    ))
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ResolvedBase {
    Layr(LayrStateId),
    Git(GitCommit),
}

pub fn pin_assignment(
    assignment: &ReviewAssignment,
    base: &ResolvedBase,
) -> Result<ReviewAssignment> {
    use studio_runtime_schema::review::{LayrBase, WorkspaceMode};
    let (mode, target) = match (assignment.target(), base) {
        (ReviewTarget::LayrState { state, .. }, ResolvedBase::Layr(base)) => (
            WorkspaceMode::LayrNative,
            ReviewTarget::LayrState {
                state: state.clone(),
                base: LayrBase::State {
                    state: base.clone(),
                },
            },
        ),
        (ReviewTarget::GitRange { head, .. }, ResolvedBase::Git(base)) => (
            WorkspaceMode::Worktree,
            ReviewTarget::GitRange {
                base: GitRef::try_from(base.to_string())
                    .map_err(|_| NativeError::Invalid("review.base"))?,
                head: head.clone(),
            },
        ),
        _ => return Err(NativeError::Invalid("review.base")),
    };
    ReviewAssignment::new(mode, target).map_err(|_| NativeError::Invalid("review.base"))
}

pub struct ReviewPlan {
    actor: Value,
    root: Value,
    key: String,
    target: Value,
    assignment: ReviewAssignment,
    resolved_base: Option<ResolvedBase>,
    args: studio_runtime_schema::creation_intent::CreationIntent,
    spec: studio_runtime_schema::creation_intent::CreationIntent,
}
impl ReviewPlan {
    /// Directory has already been resolved and checked as a repository outside
    /// the transaction. Creation rechecks the actor and delegation on reservation.
    pub fn from_records(
        actor: Value,
        root: Value,
        args: Value,
        key: String,
        directory: String,
    ) -> Result<Self> {
        bounded(&actor, "review.actor")?;
        let checked = checked_agent(&actor)?;
        let checked_root = checked_agent(&root)?;
        let mut plan = Self::from_checked_records(&checked, &checked_root, args, key, directory)?;
        // Reservation compares the original missing/null shape at this boundary.
        plan.actor = actor;
        plan.root = root;
        Ok(plan)
    }

    /// The caller validates both persisted records before entering this planner.
    pub fn from_checked_records(
        checked: &AgentRecord,
        checked_root: &AgentRecord,
        args: Value,
        key: String,
        directory: String,
    ) -> Result<Self> {
        let actor = checked.value();
        let root = checked_root.value();
        if checked.root_id != checked_root.id {
            return Err(NativeError::Changed);
        }
        if crate::event_write::truthy(actor.get("nativeReview")) {
            return Err(NativeError::Invalid("review.actor"));
        }
        if actor["executionMode"] == "vm" || crate::event_write::truthy(actor.get("remoteOrigin")) {
            return Err(NativeError::Unavailable("native-review/remote-worker"));
        }
        if key.is_empty() || key.len() > 200 || directory.is_empty() {
            return Err(NativeError::Invalid("review.identity"));
        }
        let typed_target = typed_target(&args)?;
        let options: studio_runtime_schema::review::ReviewRequest = decode(args, "review")?;
        let target = json!(&typed_target);
        let assignment = ReviewAssignment::new(
            checked
                .workspace_mode
                .ok_or(NativeError::Invalid("review.workspace"))?,
            typed_target,
        )
        .map_err(|_| NativeError::Invalid("review.workspace"))?;
        use studio_runtime_schema::queue_command::Field;
        let requested_model = Field::Missing;
        let requested_effort = EffortSelection::Inherit;
        let (model, effort, fast_mode) =
            execution_settings(checked, checked_root, &requested_model, &requested_effort)
                .map_err(|error| match error {
                    crate::managed_child_create::ManagedCreationEnvironmentError::Invalid(
                        message,
                    ) => NativeError::Invalid(message),
                    crate::managed_child_create::ManagedCreationEnvironmentError::Unavailable(
                        name,
                    ) => NativeError::Unavailable(name),
                })?;
        let fast_mode = fast_mode
            .or_else(|| actor.get("fastMode").and_then(Value::as_bool))
            .unwrap_or(false);
        let requested = studio_runtime_schema::creation_intent::CreationIntent {
            model: requested_model,
            effort: requested_effort,
            prompt: options.instructions.map(|v| v.to_string()),
            account_key: options.account.map(|v| v.to_string()),
            ..Default::default()
        };
        let prompt = review_prompt(&assignment, requested.prompt.as_deref());
        let spec = studio_runtime_schema::creation_intent::CreationIntent {
            id: Field::Value(worker_id(&key)),
            name: Some("Review".into()),
            cwd: Some(directory),
            role: Some(studio_runtime_schema::creation_intent::AgentRole::Reviewer),
            prompt: Some(prompt.chars().take(MAX_PROMPT).collect()),
            model: Field::Value(model),
            effort: effort.map_or(EffortSelection::Clear, EffortSelection::Set),
            fast_mode: Field::Value(fast_mode),
            daybreak_enabled: Field::Value(false),
            ..Default::default()
        };
        Ok(Self {
            actor,
            root,
            key,
            target,
            assignment,
            resolved_base: None,
            args: requested,
            spec,
        })
    }
    pub fn assignment(&self) -> &ReviewAssignment {
        &self.assignment
    }
    pub fn set_prepared_assignment(&mut self, assignment: ReviewAssignment, base: ResolvedBase) {
        self.spec.prompt = Some(review_prompt(&assignment, self.args.prompt.as_deref()));
        self.assignment = assignment;
        self.resolved_base = Some(base);
    }
    pub fn spec(&self) -> &studio_runtime_schema::creation_intent::CreationIntent {
        &self.spec
    }
    pub fn account(&self) -> Option<&str> {
        self.args.account_key.as_deref()
    }
    pub fn existing(&self, child: &Value) -> Result<Value> {
        checked_agent(child)?;
        let review = &child["nativeReview"];
        let same_args = if let Some(saved) = review.get("args").filter(|v| !v.is_null()) {
            saved == &json!(self.args)
        } else {
            fn matches(
                field: &studio_runtime_schema::queue_command::Field<String>,
                value: Option<&Value>,
            ) -> bool {
                use studio_runtime_schema::queue_command::Field;
                match field {
                    Field::Missing => true,
                    Field::Null => value.is_some_and(Value::is_null),
                    Field::Value(expected) => {
                        value.and_then(Value::as_str) == Some(expected.as_str())
                    }
                }
            }
            self.args.prompt.is_none()
                && self.args.account_key.is_none()
                && matches(&self.args.model, child.get("model"))
                && match &self.args.effort {
                    EffortSelection::Inherit => true,
                    EffortSelection::Clear => child.get("effort").is_some_and(Value::is_null),
                    EffortSelection::Set(expected) => {
                        child.get("effort").and_then(Value::as_str) == Some(expected.as_str())
                    }
                }
        };
        if child.get("id").and_then(Value::as_str) != self.spec.id.value().map(String::as_str)
            || review["requestId"] != self.key
            || review["actorId"] != self.actor["id"]
            || review["target"] != self.target
            || review
                .get("sourceCwd")
                .and_then(Value::as_str)
                .or_else(|| child.get("cwd").and_then(Value::as_str))
                != self.spec.cwd.as_deref()
            || !same_args
        {
            return Err(NativeError::Conflict);
        }
        let response = &review["response"];
        if response["requestId"] != self.key
            || response.get("agentId").and_then(Value::as_str)
                != self.spec.id.value().map(String::as_str)
            || response["status"] != "queued"
            || !response["reviewModel"].is_string()
            || !response["agents"].is_array()
            || !response["delivery"].is_string()
        {
            return Err(NativeError::Invalid("review.response"));
        }
        review
            .get("response")
            .filter(|v| v.is_object())
            .cloned()
            .ok_or(NativeError::Invalid("review.response"))
    }
}

fn review_prompt(assignment: &ReviewAssignment, instructions: Option<&str>) -> String {
    let mut prompt = crate::managed_child_create::reviewer_brief(assignment);
    if let Some(instructions) = instructions {
        prompt.push_str("\n\nAdditional review instructions:\n");
        prompt.push_str(instructions);
    }
    prompt
}

/// Ordinary creation validates frozen catalog/workspace evidence, actor account
/// connection and cancellation here. It returns the complete unsaved child and
/// persists the tool result in this same transaction; neither method does I/O.
pub trait ReviewCreator {
    fn child(
        &self,
        tx: &Transaction<'_>,
        actor: &Value,
        spec: &studio_runtime_schema::creation_intent::CreationIntent,
    ) -> Result<Value>;
    fn receipt(&self, tx: &Transaction<'_>, key: &str, response: &Value) -> Result<()>;
}

/// The store owns the provider progress fields. They cannot change review
/// authority; every other field remains frozen through preparation.
fn creation_inputs_match(expected: &Value, current: &Value) -> bool {
    use studio_runtime_store::agent_volatile::is_volatile_field;

    let (Some(expected), Some(current)) = (expected.as_object(), current.as_object()) else {
        return false;
    };
    expected
        .iter()
        .filter(|(key, _)| !is_volatile_field(key))
        .all(|(key, value)| current.get(key) == Some(value))
        && current
            .keys()
            .filter(|key| !is_volatile_field(key))
            .all(|key| expected.contains_key(key))
}

pub fn reserve(
    tx: &Transaction<'_>,
    plan: &ReviewPlan,
    creator: &impl ReviewCreator,
) -> Result<Value> {
    let id = plan
        .spec
        .id
        .value()
        .map(String::as_str)
        .ok_or(NativeError::Invalid("review.id"))?;
    if let Some(child) = ContextRepairRepository::agent(tx, id)? {
        return plan.existing(&child);
    }
    if plan.resolved_base.is_none() {
        return Err(NativeError::Invalid("review.base"));
    }
    let actor_id = plan.actor["id"]
        .as_str()
        .ok_or(NativeError::Invalid("review.actor.id"))?;
    let actor = ContextRepairRepository::agent(tx, actor_id)?.ok_or(NativeError::Changed)?;
    if crate::event_write::truthy(actor.get("daybreakEnabled")) {
        return Err(NativeError::Invalid("review.actor"));
    }
    if !creation_inputs_match(&plan.actor, &actor) {
        return Err(NativeError::Changed);
    }
    let root_id = actor["rootId"]
        .as_str()
        .ok_or(NativeError::Invalid("review.rootId"))?;
    let root = ContextRepairRepository::agent(tx, root_id)?.ok_or(NativeError::Changed)?;
    if !creation_inputs_match(&plan.root, &root) {
        return Err(NativeError::Changed);
    }
    crate::event_write::assert_delegation(&checked_agent(&root)?)
        .map_err(|_| NativeError::Changed)?;
    let mut child = creator.child(tx, &actor, &plan.spec)?;
    if child["id"] != id
        || child["parentId"] != actor_id
        || child["rootId"] != actor["rootId"]
        || child.get("cwd").and_then(Value::as_str) != plan.spec.cwd.as_deref()
        || child.get("model").and_then(Value::as_str) != plan.spec.model.value().map(String::as_str)
        || child["status"] != "queued"
    {
        return Err(NativeError::Changed);
    }
    child["permissionMode"] =
        json!(studio_runtime_schema::claude_permissions::PermissionMode::Plan);
    child["reviewAssignment"] = json!(plan.assignment);
    let brief = ["id", "name", "status", "model", "effort", "fastMode"]
        .into_iter()
        .map(|k| (k.to_owned(), child.get(k).cloned().unwrap_or(Value::Null)))
        .collect::<studio_json::Map<_, _>>();
    let response = json!({"requestId":plan.key,"agentId":id,"status":"queued","reviewModel":child["model"],
        "reviewEffort":child.get("nativeEffort").unwrap_or(&child["effort"]),"agents":[brief],"delivery":DELIVERY});
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    enum Status {
        OrdinaryTurn,
    }
    child["nativeReview"] = json!({"target":plan.target,"resolvedBase":plan.resolved_base,"sourceCwd":plan.spec.cwd,"requestId":plan.key,"actorId":actor_id,
        "args":plan.args,"status":Status::OrdinaryTurn,"response":response});
    let created = child["created"].as_f64().unwrap_or(0.0);
    let child = AgentRecord::from_value(child)?;
    AgentRepository::put(tx, &child)?;
    {
        let inserted = crate::event_write::enqueue_event(
            tx,
            &child,
            format!("{id}:initial"),
            "user",
            plan.spec.prompt.clone().unwrap_or_default(),
            created,
        )
        .map_err(|_| NativeError::Invalid("review.initial"))?;
        if !inserted {
            return Err(NativeError::Conflict);
        }
    }
    creator.receipt(tx, &plan.key, &response)?;
    Ok(response)
}
