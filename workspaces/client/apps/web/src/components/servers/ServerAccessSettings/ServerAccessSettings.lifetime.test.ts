// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { outboxRequests } from "../../../browserPersistence.ts";
import { createServerServices } from "../../../servers/serverServices.ts";
import { ServerServicesProvider } from "../ServerServicesProvider/ServerServicesProvider.tsx";
import ServerAccessSettings from "./ServerAccessSettings.tsx";

it.each([
  ["other", "second"],
  ["same", "second"],
  ["same", "first"],
])(
  "uses the replacement API and pending storage for server %s / credential %s",
  async (id, credentialId) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    const first = createServerServices({
      id: "same",
      origin: "https://one.test",
      label: "First",
      credentialId: "first",
    });
    const second = createServerServices({
      id,
      origin: "https://one.test",
      label: "Second",
      credentialId,
    });
    const firstStorage = new Storage();
    const secondStorage = new Storage();
    Object.defineProperty(first, "storage", { value: firstStorage });
    Object.defineProperty(second, "storage", { value: secondStorage });
    const firstSend = vi
      .spyOn(first.api, "serverAccess")
      .mockRejectedValue(new Error("First outcome unknown"));
    const secondSend = vi
      .spyOn(second.api, "serverAccess")
      .mockRejectedValue(new Error("Second outcome unknown"));
    vi.spyOn(first.api, "refreshSession").mockResolvedValue({ token: "first" });
    vi.spyOn(second.api, "refreshSession").mockResolvedValue({
      token: "second",
    });
    const saved = {
      action: "create_invite",
      label: "Saved second",
      grant: "observe",
      requestId: "second-request",
    } as const;
    outboxRequests(secondStorage).set(
      "serverAccess",
      "",
      JSON.stringify(saved),
    );
    const host = document.body.appendChild(document.createElement("div"));
    const root = createRoot(host);
    const render = (services: typeof first) =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(ServerServicesProvider, {
            services,
            children: createElement(ServerAccessSettings, { active: true }),
          }),
        ),
      );
    try {
      await act(async () => render(first));
      await act(async () => render(second));
      const retry = [...host.querySelectorAll("button")].find(
        (button) => button.textContent === "Retry saved request",
      );
      expect(retry).toBeDefined();
      await act(async () => retry?.click());
      expect(firstSend).not.toHaveBeenCalled();
      expect(secondSend).toHaveBeenCalledWith("POST", saved);
      expect(outboxRequests(firstStorage).get("serverAccess", "")).toBeNull();
      expect(
        JSON.parse(
          outboxRequests(secondStorage).get("serverAccess", "") || "null",
        ),
      ).toEqual(saved);
    } finally {
      await act(async () => root.unmount());
      host.remove();
      first.close();
      second.close();
      vi.unstubAllGlobals();
    }
  },
);

it.each([false, true])(
  "hides another service's invitation after replacement (deferred=%s)",
  async (deferred) => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
    const fonts = Object.getOwnPropertyDescriptor(document, "fonts");
    Object.defineProperty(document, "fonts", {
      value: new EventTarget(),
      configurable: true,
    });
    const owner = {
      id: "same",
      origin: "https://one.test",
      label: "First",
      credentialId: "first",
    };
    const first = createServerServices(owner),
      second = createServerServices({ ...owner, label: "Second" });
    Object.defineProperty(first, "storage", { value: new Storage() });
    Object.defineProperty(second, "storage", { value: new Storage() });
    const response =
      Promise.withResolvers<
        Awaited<ReturnType<typeof first.api.serverAccess>>
      >();
    vi.spyOn(first.api, "refreshSession").mockResolvedValue({ token: "first" });
    vi.spyOn(first.api, "serverAccess").mockReturnValue(response.promise);
    const invitation = {
      protocol: 1 as const,
      expires: 9999999999,
      inviteId: "first-invite",
      label: "First",
      origin: owner.origin,
      publicKey: "public",
      serverId: "same",
      tailscaleUser: "user",
      token: "first-token",
    };
    const host = document.body.appendChild(document.createElement("div")),
      root = createRoot(host);
    const render = (services: typeof first) =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(ServerServicesProvider, {
            services,
            children: createElement(ServerAccessSettings, { active: true }),
          }),
        ),
      );
    try {
      await act(async () => render(first));
      await act(async () =>
        [...host.querySelectorAll("button")]
          .find((button) => button.textContent === "Create pairing invitation")
          ?.click(),
      );
      if (!deferred) {
        await act(async () =>
          response.resolve({ expires: invitation.expires, invitation }),
        );
        expect(host.querySelector("textarea")?.value).toContain("first-token");
      }
      await act(async () => render(second));
      if (deferred)
        await act(async () =>
          response.resolve({ expires: invitation.expires, invitation }),
        );
      expect(host.querySelector("textarea")).toBeNull();
    } finally {
      await act(async () => {
        response.resolve({ expires: invitation.expires, invitation });
        root.unmount();
      });
      host.remove();
      first.close();
      second.close();
      vi.unstubAllGlobals();
      if (fonts) Object.defineProperty(document, "fonts", fonts);
      else Reflect.deleteProperty(document, "fonts");
    }
  },
);
