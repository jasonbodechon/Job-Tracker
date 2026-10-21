import { describe, expect, it } from "vitest";
import { defaultProfile } from "./mockJobs";
import { parseProfileList, profileListDrafts } from "./profileInputs";

describe("profile comma-separated inputs", () => {
  it("keeps the editable value as plain text while a trailing comma is present", () => {
    const draft = "Engineering Manager,";
    expect(draft.endsWith(",")).toBe(true);
    expect(parseProfileList(draft)).toEqual(["Engineering Manager"]);
  });

  it("parses comma-separated values only when the profile is saved", () => {
    expect(parseProfileList("TypeScript, React, Leadership")).toEqual(["TypeScript", "React", "Leadership"]);
  });

  it("removes empty and duplicate values without changing the first spelling", () => {
    expect(parseProfileList("React, , react, TypeScript, ")).toEqual(["React", "TypeScript"]);
  });

  it("creates editable text drafts from a stored profile", () => {
    expect(profileListDrafts(defaultProfile).titles).toBe("Engineering Manager, Senior Engineering Manager, Director of Engineering");
  });
});
