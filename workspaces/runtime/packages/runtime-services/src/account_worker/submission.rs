//! The concurrent part of a submission. The durable `submitted` commit runs
//! in the agent's own task, concurrently with other agents; the account worker
//! keeps only the provider write. The submitted commit returns before the
//! write starts. With synchronous=NORMAL an unclean stop can lose that commit,
//! so startup must hold the earlier reservation as an unknown outcome.
use super::*;
use crate::provider_session_task::PreparedRequest;
use rusqlite::OptionalExtension;

/// One request to commit, built by the worker and run in the agent's task
/// (or inline for an interrupt).
pub(super) struct Commit {
    pub bridge: StoreBridge,
    pub account: String,
    pub connection_id: String,
    /// Provider reply deadline of the request; unit: milliseconds.
    pub timeout_ms: u64,
    pub control: SessionControl,
    pub id: RequestId,
    pub context: Pending,
    pub params: Value,
    pub native: Option<crate::provider_adapter::NativeRequest>,
    pub interrupt_turn: Option<(String, i64)>,
    pub submission: Option<crate::notification_dispatch::PreparedSubmission>,
    pub guard: Option<preparation::CommitGuard>,
    pub receipt_checked: bool,
    /// The durable effect the submitted commit deletes.
    pub effect: Option<String>,
}

pub(super) enum Committed {
    /// Already submitted, or the durable boundary refused it: nothing to write.
    Nothing,
    /// A never-submitted batch was isolated and released for a fresh claim.
    Retry(String, i64),
    /// The submitted commit returned; only the write remains.
    Ready(Box<Ready>),
}

pub(super) struct Ready {
    pub id: RequestId,
    pub context: Arc<Pending>,
    pub prepared: PreparedRequest,
    pub control: SessionControl,
}

impl Commit {
    pub async fn run(self) -> Result<Committed, WorkerError> {
        if !self.receipt_checked {
            let operation = self.context.operation.clone();
            if self
                .bridge
                .worker_run("was_submitted", move |store| async move {
                    store::was_submitted(&store, &operation).await
                })
                .await?
            {
                return Ok(Committed::Nothing);
            }
        }
        if matches!(
            self.context.method,
            Method::ThreadStart | Method::TurnStart | Method::TurnInterrupt | Method::ThreadResume
        ) {
            let agent_id = self.context.intent.agent_id.clone();
            let unsupported = self.bridge.worker_run("execution_mode", move |store| async move {
                store.read_async("runtime-services/account_worker_execution_mode", move |connection| {
                    Ok::<_, WorkerError>(connection.query_row(
                        "SELECT json_extract(record,'$.executionMode')='vm' FROM runtime_agents WHERE id=?1",
                        [&agent_id], |row| row.get::<_, Option<bool>>(0),
                    ).optional()?.is_none_or(|mode| mode.unwrap_or(false)))
                }).await
            }).await?;
            if unsupported {
                return Ok(Committed::Nothing);
            }
        }
        let control = self.control.clone();
        Box::pin(self.finish(control)).await
    }

    /// Validate the request on `control`'s session, then commit `submitted`.
    pub async fn finish(self, control: SessionControl) -> Result<Committed, WorkerError> {
        let Self {
            bridge,
            account,
            connection_id,
            timeout_ms,
            id,
            context,
            params,
            native,
            interrupt_turn,
            submission,
            guard,
            effect,
            ..
        } = self;
        let method = context.method;
        let mut prepared = match crate::diagnostics::measure_runtime_stage_async(
            "turn_submission.prepare",
            async {
                // internal-sender-witness: workspaces/runtime/packages/runtime-services/src/account_worker/tests/account_transfer.rs#internal_sender_witness_transfer_completes_before_next_turn_on_destination
                let spec = RequestSpec {
                    id: id.clone(),
                    operation_id: OperationId(context.operation.clone()),
                    method,
                    params,
                    thread_id: context.intent.thread_id.clone(),
                    lightweight: method == Method::TurnInterrupt,
                    timeout_ms,
                };
                match native {
                    Some(native) => {
                        let frame = native.frame(spec.id.clone())?;
                        control.prepare_request_frame(spec, frame).await
                    }
                    None => control.prepare(spec).await,
                }
            },
        )
        .await
        {
            Ok(prepared) => prepared,
            Err(error) => return Err(error.into()),
        };
        for event in &context.intent.events {
            if let Some(stamp) = event.metadata.get("timing").and_then(|timing| {
                timing.get(studio_runtime_schema::provider_timing::ACCEPTED_STAMP_FIELD)
            }) {
                prepared.queued_input(
                    serde::Deserialize::deserialize(stamp).map_err(StoreError::from)?,
                );
            }
        }
        let recovery = prepared.recovery_record();
        let context = Arc::new(context);
        let save_context = Arc::clone(&context);
        let authorized = Box::pin(crate::diagnostics::measure_runtime_stage_async(
            "turn_submission.submitted",
            bridge.worker_run("submitted", move |store| async move {
                store::submitted(
                    &store,
                    save_context,
                    &account,
                    recovery,
                    store::SubmissionEvidence {
                        turn: interrupt_turn.map(|(turn, _)| turn),
                        submission,
                        guard,
                        connection_id,
                        effect,
                    },
                )
                .await
            }),
        ))
        .await?;
        if !authorized {
            let intent = context.intent.clone();
            let retry = bridge
                .worker_run("retry_isolated_input", move |store| async move {
                    store
                        .read_async("runtime-services/retry_isolated_input", move |db| {
                            Ok::<_, WorkerError>(
                                AgentRepository::get(db, &AgentId::from(intent.agent_id.as_str()))?
                                    .filter(|row| {
                                        row.epoch == intent.epoch
                                            && row.status
                                                == studio_runtime_store::AgentStatus::Queued
                                            && row.start_attempt.is_none()
                                    }),
                            )
                        })
                        .await
                })
                .await?;
            return Ok(retry.map_or(Committed::Nothing, |row| {
                Committed::Retry(row.id.0, row.epoch)
            }));
        }
        Ok(Committed::Ready(Box::new(Ready {
            id,
            context,
            prepared,
            control,
        })))
    }
}
