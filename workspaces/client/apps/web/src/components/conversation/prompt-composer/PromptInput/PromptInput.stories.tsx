import { useCallback, useState, useRef } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, within } from "storybook/test";
import PromptInput from "./PromptInput.tsx";
import PromptComposer, {
  type DraftWriter,
} from "../PromptComposer/PromptComposer.tsx";
import type { DraftReader } from "../PromptComposer/PromptComposer.tsx";

type PromptInputStoryProps = {
  initialValue?: string;
  managed?: boolean;
  mobile?: boolean;
  canSend?: boolean;
  draftTooLong?: boolean;
  hasAttachments?: boolean;
  onSend: () => void;
  onQueue: () => void;
};

function PromptInputStory(p: PromptInputStoryProps) {
  const initialValue =
    p.initialValue ?? (p.draftTooLong ? "x".repeat(12001) : "");
  const [draft, setDraftValue] = useState(initialValue);
  const getDraft = useCallback<DraftReader>(() => draft, [draft]);
  const setDraft = useCallback<DraftWriter>(
    (value) => setDraftValue(value),
    [],
  );
  const session = "storybook";
  const input = useRef<HTMLTextAreaElement>(null);
  return (
    <PromptComposer
      session={session}
      getDraft={getDraft}
      onSubmit={(event) => {
        event.preventDefault();
        p.onSend();
      }}
    >
      {(value) => (
        <div style={{ width: "min(680px, 85vw)" }}>
          <PromptInput
            value={value}
            session={session}
            getDraft={getDraft}
            setDraft={setDraft}
            input={input}
            mobile={!!p.mobile}
            managed={!!p.managed}
            canSend={p.canSend !== false}
            blocked={false}
            sending={false}
            uploading={false}
            draftTooLong={!!p.draftTooLong}
            modelCommand={false}
            hasAttachments={!!p.hasAttachments}
            onChange={(next) => setDraft(next, session)}
            onPasteFiles={() => undefined}
            onSend={p.onSend}
            onQueue={p.onQueue}
            onRecallKeyDown={() => false}
            skillCatalog={{
              enabled: false,
              agentId: "",
              workspace: "storybook",
              account: "default",
              cwd: "",
              provider: "codex",
            }}
          />
        </div>
      )}
    </PromptComposer>
  );
}

const meta = {
  title: "Composer/PromptInput",
  component: PromptInputStory,
  args: {
    onSend: fn(),
    onQueue: fn(),
  },
  parameters: {
    layout: "centered",
  },
} satisfies Meta<typeof PromptInputStory>;

export default meta;
type Story = StoryObj<typeof meta>;

export const SendAndNewline: Story = {
  play: async ({ args, canvasElement, userEvent: storyUserEvent }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("combobox", { name: "Message" });
    await storyUserEvent.type(input, "Draft a note");
    await expect(input).toHaveValue("Draft a note");
    await storyUserEvent.keyboard("{Shift>}{Enter}{/Shift}");
    await expect(input).toHaveValue("Draft a note\n");
    await storyUserEvent.keyboard("{Enter}");
    await expect(args.onSend).toHaveBeenCalledTimes(1);
  },
};

export const QueueAfterTurn: Story = {
  args: {
    initialValue: "Continue after this turn",
    managed: true,
  },
  play: async ({ args, canvasElement, userEvent: storyUserEvent }) => {
    const canvas = within(canvasElement);
    const input = canvas.getByRole("combobox", { name: "Message" });
    input.focus();
    await storyUserEvent.keyboard("{Tab}");
    await expect(args.onQueue).toHaveBeenCalledTimes(1);
    await expect(input).toHaveValue("Continue after this turn");
  },
};

export const Unavailable: Story = {
  args: { canSend: false },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("combobox", { name: "Message" }),
    ).toBeDisabled();
  },
};

export const AttachmentsOnlyQueue: Story = {
  args: { managed: true, hasAttachments: true },
  play: async ({ args, canvasElement, userEvent }) => {
    const input = within(canvasElement).getByRole("combobox", {
      name: "Message",
    });
    input.focus();
    await userEvent.keyboard("{Tab}");
    await expect(args.onQueue).toHaveBeenCalledTimes(1);
  },
};

export const LongDraft: Story = {
  args: { draftTooLong: true },
  play: async ({ canvasElement }) => {
    const input = within(canvasElement).getByRole("combobox", {
      name: "Message",
    });
    await expect(input).toHaveAttribute("aria-invalid", "true");
    await expect(input).toHaveValue("x".repeat(12001));
  },
};

export const ComposingText: Story = {
  args: { initialValue: "Draft while composing" },
  play: async ({ args, canvasElement, userEvent }) => {
    const input = within(canvasElement).getByRole("combobox", {
      name: "Message",
    });
    await expect(input).toHaveValue("Draft while composing");
    input.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Enter",
        bubbles: true,
        isComposing: true,
      }),
    );
    await expect(args.onSend).not.toHaveBeenCalled();
    input.focus();
    await userEvent.keyboard("{Enter}");
    await expect(args.onSend).toHaveBeenCalledTimes(1);
  },
};

export const MobileNewline: Story = {
  args: { mobile: true },
  play: async ({ args, canvasElement, userEvent }) => {
    const input = within(canvasElement).getByRole("combobox", {
      name: "Message",
    });
    input.focus();
    await userEvent.type(input, "Keep drafting");
    await userEvent.keyboard("{Enter}");
    await expect(args.onSend).not.toHaveBeenCalled();
    await expect(input).toHaveValue("Keep drafting\n");
  },
};
