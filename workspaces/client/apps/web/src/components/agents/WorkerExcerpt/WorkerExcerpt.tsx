import { Button } from "@mantine/core";
import { FileText, ChevronRight } from "lucide-react";
import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useSyncedVisualState } from "../../../sync/useSyncedVisualState.ts";
import Disclosure from "../../ui/Disclosure/Disclosure.tsx";

export default function WorkerExcerpt({
  agentId,
  label,
  text,
  truncated,
  open,
}: {
  agentId: string;
  label: string;
  text: string;
  truncated?: boolean | undefined;
  open: () => void;
}) {
  const { save, saved } = useServerApi();

  const storageKey = "codex-worker-disclosures";
  const key = `${agentId}:${label}`;
  const [disclosures] = useSyncedVisualState<Record<string, boolean>>(
    storageKey,
    {},
  );
  const expanded = disclosures[key] || false;
  return (
    <Disclosure
      className="worker-excerpt"
      open={expanded}
      onChange={(value) => {
        const prior = saved<Record<string, boolean>>(storageKey, {});
        save(
          storageKey,
          Object.fromEntries([
            ...Object.entries(prior)
              .filter(([id]) => id !== key)
              .slice(-499),
            [key, value],
          ]),
        );
      }}
      summary={
        <span className="worker-excerpt-label">
          <FileText size={11} aria-hidden="true" />
          {label}
          <ChevronRight size={11} aria-hidden="true" />
        </span>
      }
    >
      <div className="worker-excerpt-full">
        <p>{text}</p>
        {truncated && (
          <Button variant="subtle" size="compact-xs" onClick={open}>
            Continue in chat
          </Button>
        )}
      </div>
    </Disclosure>
  );
}
