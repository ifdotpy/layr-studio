// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { dictationLanguageOptions } from "./BrowserDictation.tsx";

it("reuses language names until the device locale changes", () => {
  const displayNames = vi.spyOn(Intl, "DisplayNames");
  try {
    vi.stubGlobal("navigator", { language: "en-US" });
    const english = dictationLanguageOptions();
    dictationLanguageOptions();
    expect(displayNames).toHaveBeenCalledTimes(1);
    expect(
      english.filter((option) => option.props.value === "en-US"),
    ).toHaveLength(1);
    expect(
      english.find((option) => option.props.value === "de-DE")?.props.children,
    ).toBe("German (Germany)");
    vi.stubGlobal("navigator", { language: "fr-FR" });
    const french = dictationLanguageOptions();
    expect(displayNames).toHaveBeenCalledTimes(2);
    expect(
      french.find((option) => option.props.value === "de-DE")?.props.children,
    ).toBe("allemand (Allemagne)");
  } finally {
    displayNames.mockRestore();
    vi.unstubAllGlobals();
  }
});
