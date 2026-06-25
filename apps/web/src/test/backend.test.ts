import { describe, expect, it } from "vitest";
import { backendPlaceholder } from "../lib/backend";

describe("backendPlaceholder", () => {
  it("exposes only health endpoints in phase 0", () => {
    expect(backendPlaceholder().endpoints).toEqual(["/healthz", "/readyz"]);
  });
});
