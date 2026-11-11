import { RoomKind as GeneratedApiRoomKind } from "../../../../generated/api.ts";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { MantineProvider, useMantineColorScheme } from "@mantine/core";
import { useEffect, useMemo } from "react";
import { expect, userEvent, within } from "storybook/test";
import TeamChats from "./TeamChats.tsx";
import { createTeamChatsStore } from "./TeamChatsStore.ts";
import { theme } from "../../../../theme.ts";
import {
  observableFixture,
  conversationActionsFixture,
} from "../../integration-support/observableFixture.tsx";
import { ConversationProvider } from "../../../conversation/support/navigation.ts";
import { createRoomMirror } from "../../../../generated/observableResources.ts";
import type { RoomEntityDtoFields } from "../../../../generated/observableFieldTypes.ts";

const rooms: RoomEntityDtoFields[] = [
  {
    id: "broadcast:lead",
    kind: GeneratedApiRoomKind.broadcast,
    name: "Team broadcast",
    rootId: "lead",
    lastMessage: {
      created: 1792706400,
      sender: "lead",
      seq: 1,
      text: "Release checks are complete. Review the API changes.",
    },
  },
  {
    id: "private:worker",
    kind: GeneratedApiRoomKind.private,
    name: "API tests",
    members: ["lead", "worker"],
    lastMessage: {
      created: 1792706400,
      sender: "worker",
      seq: 1,
      text: "The retry test reproduces the missing receipt.",
    },
  },
];
function MessagesFixture({
  light = false,
  hasRooms = false,
  narrow = false,
}: {
  light?: boolean;
  hasRooms?: boolean;
  narrow?: boolean;
}) {
  const models = useMemo(() => {
    const models = observableFixture([
      { id: "lead", name: "Release lead" },
      { id: "worker", rootId: "lead", name: "API tests" },
    ]);
    const state = [...models.mirror.current.agents.values()].map((value) => ({
      collection: "agentSummary" as const,
      id: value.id,
      value,
    }));
    models.apply([
      ...state,
      {
        collection: "workspace",
        id: "workspace",
        value: { stateDir: "storybook-messages" },
      },
      ...(hasRooms
        ? rooms.map((value) => ({
            collection: "room" as const,
            id: value.id,
            value,
          }))
        : []),
    ]);
    const history = createRoomMirror({
      room: { id: "private:worker" },
      messages: [
        {
          id: "message-1",
          room: "private:worker",
          deliveries: {},
          sender: "worker",
          senderName: "API tests",
          created: 1792706400,
          seq: 1,
          text: "The retry test reproduces the missing receipt. I am checking the request identity.",
        },
      ],
    });
    models.store.room = () => ({
      current: { status: "live", data: history.current },
    });
    return {
      ...models,
      chats: createTeamChatsStore(
        models.store,
        {
          saved: (_key, fallback) => fallback,
          save() {},
          syncGet: async () => {
            throw new Error("Unexpected story read");
          },
        },
        "storybook-messages",
        "lead",
      ),
    };
  }, [hasRooms]);
  useEffect(() => () => models.store.dispose(), [models]);
  const { setColorScheme } = useMantineColorScheme();
  useEffect(() => {
    setColorScheme(light ? "light" : "dark");
  }, [light, setColorScheme]);
  return (
    <MantineProvider theme={theme} forceColorScheme={light ? "light" : "dark"}>
      <div
        style={{
          display: "flex",
          height: "80dvh",
          minWidth: 0,
          maxWidth: narrow ? "var(--studio-form-max)" : undefined,
        }}
      >
        <models.Provider>
          <ConversationProvider value={conversationActionsFixture()}>
            <TeamChats leadId="lead" owner={models.chats} />
          </ConversationProvider>
        </models.Provider>
      </div>
    </MantineProvider>
  );
}
const meta = {
  title: "Messages/Room list",
  component: MessagesFixture,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof MessagesFixture>;
export default meta;
type Story = StoryObj<typeof meta>;
export const EmptyDark: Story = {};
export const EmptyLight: Story = { args: { light: true } };
export const RoomsDark: Story = { args: { hasRooms: true } };
export const RoomsLight: Story = { args: { hasRooms: true, light: true } };
export const NarrowDrawer: Story = {
  args: { hasRooms: true, narrow: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("textbox", { name: "Search chats" }),
    ).toBeVisible();
    await userEvent.click(
      canvas.getByRole("button", { name: /API tests .*The retry test/ }),
    );
    await expect(
      await canvas.findByText(
        "The retry test reproduces the missing receipt. I am checking the request identity.",
      ),
    ).toBeVisible();
    await expect(
      canvas.getByRole("heading", { name: "API tests" }),
    ).toHaveFocus();
    await userEvent.click(
      canvas.getByRole("button", { name: "Back to chats" }),
    );
    await expect(
      canvas.getByRole("textbox", { name: "Search chats" }),
    ).toBeVisible();
  },
};
