import { ProjectSave } from "../../../app/projects/ProjectSave.ts";
import { localSidebarBackend } from "../../sidebar/support/services.tsx";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { MantineProvider } from "@mantine/core";
import { expect, userEvent, within } from "storybook/test";
import { MoveChatForm } from "../MoveChatForm/MoveChatForm.tsx";
import { ProjectNameForm } from "../ProjectNameForm/ProjectNameForm.tsx";
import type { Project } from "../../../app/projects/ProjectOrganization.tsx";
import ProjectLocationsSettings from "../../settings/ProjectLocationsSettings/ProjectLocationsSettings.tsx";
import { SettingsSection } from "../../ui/PanelHeader/PanelHeader.tsx";
import { theme } from "../../../theme.ts";
import "../ProjectAccount/project-settings.css";

const nameSave = new ProjectSave(
  localSidebarBackend,
  "/api/projects",
  async () => {},
);
const moveSave = new ProjectSave(
  localSidebarBackend,
  "/api/organization",
  async () => {},
);
const project = {
  id: "atlas",
  name: "Atlas API",
  path: "/workspace/atlas/services/api",
  folders: [{ id: "review", name: "Release review" }],
  locations: [
    {
      serverId: "local",
      projectId: "atlas",
      path: "/workspace/atlas/services/api",
    },
    {
      serverId: "build",
      projectId: "atlas-build",
      path: "/workspace/atlas-release/services/api",
    },
  ],
} satisfies Project;

function ProjectFixture({ light = false }: { light?: boolean }) {
  return (
    <MantineProvider theme={theme} forceColorScheme={light ? "light" : "dark"}>
      <div
        className="project-settings"
        style={{
          maxWidth: "var(--studio-form-max)",
          maxHeight: "calc(100dvh - var(--mantine-spacing-xl) * 2)",
          overflow: "auto",
        }}
      >
        <SettingsSection title="Project organization">
          <ProjectNameForm project={project} save={nameSave} />
          <MoveChatForm
            project={project}
            agent={{ id: "lead" }}
            save={moveSave}
          />
        </SettingsSection>
        <SettingsSection
          title="Project folders"
          help="The same project can have a working folder on each server."
        >
          <ProjectLocationsSettings
            project={project}
            servers={[
              { id: "local", label: "Workstation" },
              { id: "build", label: "Build server" },
            ]}
          />
        </SettingsSection>
      </div>
    </MantineProvider>
  );
}

const meta = {
  title: "Settings/Project organization",
  component: ProjectFixture,
  parameters: { layout: "fullscreen" },
  beforeEach: () => {
    const original = window.fetch;
    window.fetch = async (input, init) => {
      const request = new Request(input, init);
      if (new URL(request.url).pathname === "/api/project-locations")
        return new Response(JSON.stringify({ gitHead: "ui/redesign" }), {
          headers: { "content-type": "application/json" },
        });
      return original(input, init);
    };
    return () => {
      window.fetch = original;
    };
  },
} satisfies Meta<typeof ProjectFixture>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Dark: Story = {};
export const Light: Story = { args: { light: true } };
export const AccessibleFields: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("textbox", { name: "Project name" }),
    ).toHaveValue("Atlas API");
    await userEvent.selectOptions(
      canvas.getByRole("combobox", { name: "Move to folder" }),
      "review",
    );
    await expect(
      canvas.getByRole("combobox", { name: "Move to folder" }),
    ).toHaveValue("review");
    await expect(
      canvas.getByRole("button", {
        name: "Options for folder /workspace/atlas/services/api",
      }),
    ).toBeVisible();
  },
};
