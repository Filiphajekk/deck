import { describe, expect, it } from "vitest";
import { translate } from "../i18n";
import { errorText } from "./errorText";

describe("errorText", () => {
  const t = (k: string) => translate("cs", k);

  it("translates i18n error keys from the backend", () => {
    expect(errorText(t, "i18n:plugins.protected")).toContain("Elgato");
    expect(errorText(t, "Error: i18n:plugins.protected")).toContain("Elgato");
  });

  it("leaves other messages alone", () => {
    expect(errorText(t, "cannot read file")).toBe("cannot read file");
  });
});
