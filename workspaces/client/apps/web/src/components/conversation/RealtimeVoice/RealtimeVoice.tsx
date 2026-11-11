import { whileObserved } from "../../../observed.ts";
import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { agentId as resourceAgentId } from "../../../generated/observableResources.ts";
import { VoiceRecordKind as GeneratedApiVoiceRecordKind } from "../../../generated/api.ts";
import {
  useServerApi,
  useServerNet,
  useServerStore,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { hasNativeClient } from "../../../native/client.ts";
import { useServerAudio } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { nativeVoice } from "../../../native/voice.ts";
import { makeAutoObservable, reaction, flow, action } from "mobx";
import { ActionIcon, Popover } from "@mantine/core";
import { AudioLines, X } from "lucide-react";
import { errorText, type PostResult } from "../../../api.ts";

import { decodeVoiceEvent } from "./voiceEvent.ts";

function* toGenerator<T>(promise: Promise<T>): Generator<Promise<T>, T, T> {
  return yield promise;
}

type Session = PostResult<"/api/voice/start">;
type Peer = {
  id: string;
  connection?: RTCPeerConnection;
  native?: ReturnType<typeof nativeVoice>;
  connectionState?: string;
  stream?: MediaStream;
  audio?: HTMLAudioElement;
  channel?: RTCDataChannel;
  remoteSet: boolean;
  ready: boolean;
  closed: boolean;
  submitted: boolean;
  registered: boolean;
  deadline: ReturnType<typeof setTimeout>;
};

type VoiceOptions = {
  agentId: string;
  notify: (text: string) => void;
  store: ReturnType<typeof useServerStore>;
  api: ReturnType<typeof useServerApi>;
  audio: ReturnType<typeof useServerAudio>;
  connection: ReturnType<typeof useServerNet>["status"];
};

export function createRealtimeVoice({
  agentId,
  notify,
  store,
  api,
  audio,
  connection,
}: VoiceOptions) {
  const { post } = api;
  const { dictationBridge, nativeVoice } = audio;
  const state = makeAutoObservable(
    {
      opened: false,
      active: false,
      muted: false,
      status: "",
      error: "",
      blockedAudio: false,
      partial: "",
      get rows() {
        const current =
          this.opened || this.active
            ? store.voice(resourceAgentId(agentId)).current
            : undefined;
        return current ? (resourceData(current)?.records ?? []) : [];
      },
    },
    {},
    { autoBind: true },
  );
  const update = action((patch: Partial<typeof state>) =>
    Object.assign(state, patch),
  );
  const setOpened = (opened: boolean) => update({ opened });
  let peer: Peer | null = null;
  let generation: number = 0;
  let muteWanted: boolean = false;
  let mounted: boolean = true;
  let starting: boolean = false;
  let pendingSession: Session | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  const endSession = (session_id: string) =>
    post(
      "/api/voice/end",
      { agent: agentId, session_id },
      { timeoutMs: 15000 },
    );
  const end = (p: Peer) => {
    p.closed = true;
    clearTimeout(p.deadline);
    p.channel?.close();
    p.connection?.close();
    void p.native?.close();
    p.stream?.getTracks().forEach((t) => t.stop());
    p.audio?.pause();
    if (p.audio) p.audio.srcObject = null;
    if (p.submitted)
      void endSession(p.id).catch((e) => {
        if (mounted && peer === null)
          update({
            error: state.error || `Microphone stopped. ${errorText(e)}`,
          });
      });
  };
  const close = () => {
    clearTimeout(reconnectTimer);
    ++generation;
    starting = false;
    muteWanted = false;
    const p = peer;
    peer = null;
    pendingSession = null;
    if (p) end(p);
    update({ active: false, muted: false, partial: "", status: "" });
  };
  const failure = (e: unknown) => {
    close();
    if (mounted) update({ error: errorText(e) });
  };
  const apply = flow(function* (p: Peer, state: Session) {
    if (peer !== p || p.closed) return;
    if (state.error || state.ended) {
      if (state.error) failure(new Error(state.error));
      else close();
      return;
    }
    if (state.sdp && !p.remoteSet) {
      p.remoteSet = true;
      if (p.native) yield* toGenerator(p.native.answer(state.sdp));
      else
        yield* toGenerator(
          p.connection!.setRemoteDescription({
            type: "answer",
            sdp: state.sdp,
          }),
        );
    }
  });
  let stopConnection: (() => void) | undefined,
    stopData: (() => void) | undefined;
  const start = () => {
    if (stopData) return;
    mounted = true;
    stopConnection = reaction(
      () => connection.connection,
      (health) => {
        if (health !== "live" && (starting || (peer && !peer.closed))) {
          failure(
            new Error(
              "Studio connection lost. The microphone is off. Start voice again after the connection returns.",
            ),
          );
          update({ opened: true });
        }
      },
    );
    stopData = reaction(
      () =>
        state.opened || state.active
          ? store.voice(resourceAgentId(agentId)).current
          : undefined,
      (voice) => {
        const data = voice ? resourceData(voice) : undefined;
        const p = peer;
        const failureError =
          voice && "error" in voice ? voice.error : undefined;
        if (failureError) {
          if (p) {
            failure(
              new Error(
                `Studio connection lost. The microphone is off. Start voice again after the connection returns. ${errorText(failureError)}`,
              ),
            );
            update({ opened: true });
          } else update({ error: state.error || errorText(failureError) });
          return;
        }
        if (p && data?.session?.session_id === p.id) {
          if (p.registered) void apply(p, data.session).catch(failure);
          else pendingSession = data.session;
        }
      },
      { fireImmediately: true },
    );
  };
  const dispose = () => {
    stopData?.();
    stopData = undefined;
    stopConnection?.();
    stopConnection = undefined;
    const hadVoice = !!peer;
    mounted = false;
    close();
    if (hadVoice) notify("Voice ended because you left the chat.");
  };
  whileObserved(state, "rows", () => {
    start();
    return dispose;
  });
  const connectionChanged = (current: Peer, health: string) => {
    current.connectionState = health;
    if (current.closed) return;
    if (health === "failed")
      failure(new Error("Voice disconnected. The saved transcript remains."));
    if (health === "disconnected") {
      update({ status: "Reconnecting. Microphone paused…" });
      void current.native?.mute(true).catch(failure);
      current.stream?.getAudioTracks().forEach((t) => {
        t.enabled = false;
      });
      clearTimeout(reconnectTimer);
      reconnectTimer = setTimeout(() => {
        if (peer === current && !current.closed) {
          failure(
            new Error(
              "Voice could not reconnect. The microphone is off. Start voice again.",
            ),
          );
          update({ opened: true });
        }
      }, 12000);
    }
    if (health === "connected") {
      clearTimeout(reconnectTimer);
      void current.native?.mute(muteWanted).catch(failure);
      current.stream?.getAudioTracks().forEach((t) => {
        t.enabled = !muteWanted;
      });
      update({ status: current.ready ? "Listening" : "Connecting…" });
    }
  };
  const messageReceived = (current: Peer, data: string) => {
    if (current.closed) return;
    let value: unknown;
    try {
      value = JSON.parse(data);
    } catch {
      return;
    }
    const event = decodeVoiceEvent(value);
    if (event.type === "error") {
      failure(new Error(event.message || "Voice failed"));
      return;
    }
    if (event.type === "session.started") {
      clearTimeout(current.deadline);
      current.ready = true;
      update({ status: "Listening" });
    }
    if (event.type === "input_transcript.added") {
      update({ status: "Listening", partial: event.text });
    }
    if (event.type === "output_transcript.added") {
      update({ status: "Speaking", partial: event.text });
    }
    if (event.type === "delegation.created") {
      update({ status: "Working", partial: "" });
    }
    if (event.type === "turn.done") {
      update({ partial: "", status: "Listening" });
    }
    // Native Core owns delegation and transcript persistence. Do not mirror either.
  };
  const begin = flow(function* () {
    if (peer) return;
    const ticket = ++generation;
    starting = true;
    update({
      error: "",
      blockedAudio: false,
      status: "Connecting…",
      active: true,
    });
    let p: Peer | null = null;
    try {
      if (connection.connection !== "live")
        throw new Error(
          "Studio connection is unavailable. Try voice again when connected.",
        );
      // Capture authorization while Start is still the user's native event.
      const permission = hasNativeClient()
        ? dictationBridge()!
            .requestMicrophone()
            .then(
              () => ({ error: undefined }),
              (error) => ({ error }),
            )
        : undefined;
      yield* toGenerator(
        post("/api/voice/status", { agent: agentId }, { timeoutMs: 15000 }),
      );
      if (ticket !== generation) return;
      p = {
        id: randomUUID(),
        remoteSet: false,
        ready: false,
        closed: false,
        submitted: false,
        registered: false,
        deadline: setTimeout(() => {
          if (ticket === generation)
            failure(new Error("Voice did not connect. Start again."));
        }, 90000),
      };
      peer = p;
      const current = p;
      let sdp: string;
      if (permission) {
        const authorization = yield* toGenerator(permission);
        if (authorization.error) throw authorization.error;
        if (current.closed || ticket !== generation) return;
        current.native = nativeVoice(current.id, (event) => {
          if (current.closed) return;
          if (event.type === "message" && event.data)
            messageReceived(current, event.data);
          else if (event.type === "state" && event.state)
            connectionChanged(current, event.state);
          else if (
            event.type === "error" ||
            (event.type === "microphone" && event.state === "ended") ||
            (event.type === "channel" && event.state === "closed")
          )
            failure(
              new Error(
                (event.type === "error" && event.message) ||
                  "Voice disconnected. The saved transcript remains.",
              ),
            );
        });
        sdp = yield* toGenerator(current.native.offer());
        if (current.closed || ticket !== generation) {
          void current.native.close();
          return;
        }
        if (muteWanted) yield* toGenerator(current.native.mute(true));
      } else {
        if (!window.isSecureContext || !navigator.mediaDevices)
          throw new Error("Voice needs HTTPS or the desktop app");
        const connection = new RTCPeerConnection();
        const audio = new Audio();
        audio.autoplay = true;
        const channel = connection.createDataChannel("oai-events");
        current.connection = connection;
        current.audio = audio;
        current.channel = channel;
        connection.ontrack = (e) => {
          if (current.closed) return;
          audio.srcObject = e.streams[0] || new MediaStream([e.track]);
          void audio.play().catch(() => {
            if (!current.closed) update({ blockedAudio: true });
          });
        };
        connection.onconnectionstatechange = () =>
          connectionChanged(current, connection.connectionState);
        channel.onmessage = (message) => messageReceived(current, message.data);
        if (window.codexDesktop && !window.codexDesktop.requestMicrophone)
          throw new Error("Update the desktop app to enable microphone access");
        if (window.codexDesktop?.requestMicrophone)
          yield* toGenerator(window.codexDesktop.requestMicrophone());
        if (current.closed || ticket !== generation) return;
        const stream = yield* toGenerator(
          navigator.mediaDevices.getUserMedia({
            audio: { echoCancellation: true, noiseSuppression: true },
          }),
        );
        if (current.closed || ticket !== generation) {
          stream.getTracks().forEach((t) => t.stop());
          return;
        }
        current.stream = stream;
        stream.getTracks().forEach((t) => {
          t.enabled = !muteWanted;
          t.onended = () => {
            if (!current.closed) failure(new Error("Microphone disconnected"));
          };
          connection.addTrack(t, stream);
        });
        const offer = yield* toGenerator(connection.createOffer());
        yield* toGenerator(connection.setLocalDescription(offer));
        if (current.closed) return;
        sdp = offer.sdp || "";
        if (!sdp) throw new Error("Voice offer has no session description.");
      }
      if (!sdp) throw new Error("Voice offer has no session description.");
      current.submitted = true;
      const result = yield* toGenerator(
        post(
          "/api/voice/start",
          {
            agent: agentId,
            session_id: current.id,
            sdp,
          },
          { timeoutMs: 15000 },
        ),
      );
      current.registered = true;
      if (current.closed) {
        end(current);
        return;
      }
      const notifiedSession = pendingSession;
      pendingSession = null;
      yield* toGenerator(
        apply(
          current,
          notifiedSession?.session_id === current.id ? notifiedSession : result,
        ),
      );
    } catch (e) {
      if (ticket === generation) {
        failure(e);
        notify(errorText(e));
      } else if (p && !p.closed) end(p);
    } finally {
      starting = false;
    }
  });
  const toggleMute = () => {
    const next = !state.muted;
    muteWanted = next;
    peer?.stream?.getAudioTracks().forEach((t) => {
      t.enabled = !next && peer?.connectionState !== "disconnected";
    });
    void peer?.native
      ?.mute(next || peer.connectionState === "disconnected")
      .catch(failure);
    update({ muted: next });
  };
  return Object.assign(state, {
    start,
    dispose,
    setOpened,
    begin,
    close,
    toggleMute,
    resumeAudio() {
      void peer?.audio
        ?.play()
        .then(() => update({ blockedAudio: false }))
        .catch((e) => update({ error: errorText(e) }));
    },
  });
}

type VoiceProps = { voiceStore: ReturnType<typeof createRealtimeVoice> };
export default observer(function RealtimeVoice({ voiceStore }: VoiceProps) {
  const local = voiceStore;
  const {
    opened,
    active,
    muted,
    status,
    error,
    blockedAudio,
    partial,
    rows,
    setOpened,
    close,
    toggleMute,
  } = local;
  return (
    <>
      <Popover
        opened={opened}
        onChange={setOpened}
        position="top-start"
        width={360}
        withinPortal
      >
        <Popover.Target>
          <ActionIcon
            type="button"
            size="lg"
            variant="subtle"
            color={active ? "blue" : "gray"}
            aria-label="Voice conversation"
            title="Voice conversation"
            aria-pressed={active}
            onClick={() => setOpened(!opened)}
          >
            <AudioLines size={18} />
          </ActionIcon>
        </Popover.Target>
        <Popover.Dropdown className="realtime-voice">
          <header className="realtime-voice-heading">
            <strong>Voice</strong>
            <ActionIcon
              type="button"
              variant="subtle"
              color="gray"
              size="sm"
              aria-label="Close voice panel"
              onClick={() => setOpened(false)}
            >
              <X size={16} />
            </ActionIcon>
          </header>
          {!active && !error && (
            <p className="realtime-voice-note">
              Talk directly to this chat’s orchestrator.
            </p>
          )}
          <div className="realtime-voice-actions">
            {!active && (
              <button type="button" onClick={() => void local.begin()}>
                Start voice
              </button>
            )}
            {active && opened && (
              <>
                <button type="button" aria-pressed={muted} onClick={toggleMute}>
                  {muted ? "Unmute" : "Mute"}
                </button>
                <button type="button" onClick={close}>
                  End voice
                </button>
              </>
            )}
            {active && blockedAudio && (
              <button type="button" onClick={local.resumeAudio}>
                Resume audio
              </button>
            )}
          </div>
          {active && opened && (
            <p role="status">{muted ? "Microphone muted" : status}</p>
          )}
          {error && <p role="alert">{error}</p>}
          {partial && <p className="realtime-voice-note">{partial}</p>}
          {active && (
            <small>
              Voice ends when you leave this chat. The task keeps running.
            </small>
          )}
          {!!rows.length && (
            <details>
              <summary>Transcript</summary>
              <div className="realtime-voice-transcript">
                {rows
                  .filter(
                    (r) =>
                      r.text &&
                      ["user", "courier", "assistant", "orchestrator"].includes(
                        r.kind,
                      ),
                  )
                  .map((r) => (
                    <div key={r.id}>
                      <strong>
                        {r.kind === GeneratedApiVoiceRecordKind.user
                          ? "You"
                          : r.kind === GeneratedApiVoiceRecordKind.courier
                            ? "Previous voice assistant"
                            : "Codex"}
                      </strong>
                      <p>{r.text}</p>
                    </div>
                  ))}
              </div>
            </details>
          )}
        </Popover.Dropdown>
      </Popover>
      {active && !opened && (
        <div
          className="voice-active-bar"
          role="group"
          aria-label="Active voice conversation"
        >
          <span role="status">{muted ? "Microphone muted" : status}</span>
          <button type="button" aria-pressed={muted} onClick={toggleMute}>
            {muted ? "Unmute" : "Mute"}
          </button>
          <button type="button" onClick={close}>
            End voice
          </button>
        </div>
      )}
    </>
  );
});
