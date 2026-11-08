import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";
import { preprocessCSS, resolveConfig } from "vite";

// Relational selectors can invalidate the whole chat on each text input.
// Each existing exception is tied to a specific layout behavior.
const allowed: Record<string, Record<string, string>> = {
  "../../sidebar/Sidebar/sidebar-projects.css": {
    '.project-tree-heading:is( :hover, :focus-within, :has(> .project-tree-action[aria-expanded="true"]) ) > .project-tree-action':
      "Keep an open project menu action visible.",
    ".row-copy:has(> .fleet-chat-heading)":
      "Align chat status and metadata in a grid.",
    ".fleet-strip:is( :hover, :focus-within, .selected, :has(.row-actions[data-expanded]) ) > .chat-row":
      "Reserve space for visible row actions.",
    ".sidebar-row:has(.inline-rename) > .chat-row":
      "Make space for the inline rename field.",
    '.project-tree-heading > .project-tree-action, .project-tree-heading:is( :hover, :focus-within, :has(> .project-tree-action[aria-expanded="true"]) ) > .project-tree-action':
      "Keep project actions touch accessible.",
    '.sidebar-header:not(:has(> [aria-label="Close conversations"])) > .brand':
      "Align the sidebar title without a close button.",
  },
  "../../sidebar/chat-row/ChatServerLine/sidebar-status.css": {
    ".chat-row:has(.chat-server-line) > .chat-status, .server-chat:has(.chat-server-line) > .chat-status":
      "Align the status with a server line.",
  },
  "conversation-chrome.css": {
    ".conversation-heading:has(#back-lead)": "Fit the lead navigation control.",
    ".composer-tools:has(.attachment-list)": "Fit attachment controls.",
    ".composer-bar:has(.attachment-list)": "Fit the attachment list.",
    ".composer-bar:has(.attachment-list) .composer-submit-actions":
      "Align actions beside attachments.",
    ".composer-tools-disclosure:not( :has(.composer-tools-popover > :not(.composer-queue-disclosure)) )":
      "Hide an otherwise empty compact disclosure.",
  },
  "empty-chat-settings.css": {
    "#composer:has(.composer-draft-control:not(:empty)) #message":
      "Fit the draft controls.",
    "#composer:has(#stop) #message": "Fit the stop control.",
    "#composer:has(#stop) > .account-transfer-chip":
      "Fit transfer status with stop.",
  },
};

test("sidebar and composer relational selectors cannot expand input style invalidation", async () => {
  for (const [file, exceptions] of Object.entries(allowed)) {
    const selectors = new Set<string>();
    const config = await resolveConfig(
      {
        configFile: false,
        css: {
          postcss: {
            plugins: [
              {
                postcssPlugin: "composer-style-invalidation",
                Once(root) {
                  root.walkRules((rule) => {
                    if (rule.selector.includes(":has("))
                      selectors.add(rule.selector.replace(/\s+/g, " ").trim());
                  });
                },
              },
            ],
          },
        },
      },
      "serve",
    );
    const url = new URL(file, import.meta.url);
    await preprocessCSS(
      await readFile(url, "utf8"),
      fileURLToPath(url),
      config,
    );
    expect([...selectors].sort(), file).toEqual(Object.keys(exceptions).sort());
    expect(Object.values(exceptions).every(Boolean), file).toBe(true);
  }
});
