import { describe, expect, it } from "vitest";
import { defaultProfile } from "./mockJobs";
import { dueScheduledProfiles, isScheduleDue } from "./scheduling";

describe("search scheduling", () => {
  const monday = new Date("2026-08-17T13:00:00Z");

  it("never schedules manual profiles", () => expect(isScheduleDue("Manual", undefined, monday)).toBe(false));
  it("runs daily after the cooldown and not twice in twenty hours", () => {
    expect(isScheduleDue("Daily", "2026-08-16T12:00:00Z", monday)).toBe(true);
    expect(isScheduleDue("Daily", "2026-08-17T01:00:00Z", monday)).toBe(false);
  });
  it("skips weekday schedules on weekends", () => expect(isScheduleDue("Weekdays", undefined, new Date("2026-08-16T13:00:00Z"))).toBe(false));
  it("requires seven days for weekly schedules", () => expect(isScheduleDue("Weekly", "2026-08-11T13:01:00Z", monday)).toBe(false));
  it("returns only enabled due profiles", () => {
    const due = { ...defaultProfile, id: "due", schedule: "Daily" as const, enabled: true };
    const disabled = { ...defaultProfile, id: "disabled", schedule: "Daily" as const, enabled: false };
    expect(dueScheduledProfiles([due, disabled], undefined, monday).map((profile) => profile.id)).toEqual(["due"]);
  });
});
