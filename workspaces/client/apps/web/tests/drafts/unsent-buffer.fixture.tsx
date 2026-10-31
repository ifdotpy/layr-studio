import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { createDirectDrafts } from "../../src/sync/directDrafts.ts";
import { useUnsentText } from "../../src/unsent/useUnsentText.ts";
import { unsentStore } from "../../src/unsent/store.ts";

const buffer = unsentStore("fixture", "draft");
declare global {
  interface Window {
    unsentFixture: typeof buffer;
    unsentFormFixture: typeof buffer;
  }
}
window.unsentFixture = buffer;
window.unsentFormFixture = unsentStore("local", "form");
const store = createDirectDrafts({
  post: async () => [],
  changed: () => {},
  buffer,
});
function Fixture() {
  const [text, setText] = useState("");
  const [status, setStatus] = useState("loading");
  const [form, saveForm, clearForm] = useUnsentText("form", "review");
  const [submitted, setSubmitted] = useState<string | null>(null);
  const [refused, setRefused] = useState(false);
  useEffect(() => {
    void store.restore().then(() => {
      const row = store.records()[0];
      if (row) setText(JSON.parse(row.payload).text);
      store.snapshot([], true, 1);
      setStatus("ready");
    });
  }, []);
  return (
    <>
      <label>
        Draft
        <textarea
          value={text}
          onChange={(event) => {
            const next = event.currentTarget.value;
            setText(next);
            setStatus("saving");
            store.edit({
              id: "draft",
              payload: JSON.stringify({
                id: "draft",
                session: "chat",
                device: "fixture",
                text: next,
                updated: Date.now(),
              }),
              seq: 0,
              _deleted: false,
            });
            void buffer.loadAll().then(() => setStatus("saved"));
          }}
        />
      </label>
      <button
        onClick={() =>
          void store.flush().then(() => {
            setText("");
            setStatus("confirmed");
          })
        }
      >
        Confirm
      </button>
      <label>
        Form
        <input
          value={form ?? ""}
          onChange={(event) => saveForm(event.currentTarget.value)}
        />
      </label>
      <button onClick={() => setSubmitted(form)}>Begin request</button>
      <button
        onClick={() => {
          if (submitted !== null)
            void clearForm(submitted).then(() => setRefused(true));
        }}
      >
        Refuse
      </button>
      {refused && <span>Refusal processed</span>}
      <output>{status}</output>
    </>
  );
}
const root = document.getElementById("root");
if (root) createRoot(root).render(<Fixture />);
