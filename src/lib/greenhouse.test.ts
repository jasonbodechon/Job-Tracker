import { describe, expect, it } from "vitest";
import { normalizeGreenhouseJobs } from "./greenhouse";
import { defaultProfile } from "./mockJobs";

const job = { id: 7, title: "Senior Engineering Manager", absolute_url: "https://example.test/apply", updated_at: "2026-08-19T12:00:00Z", content: "<p>Lead with TypeScript, React, and Leadership. Fully remote.</p>", location: { name: "Canada" } };

describe("normalizeGreenhouseJobs", () => {
  it("normalizes a public posting, strips markup, scores it, and preserves the application URL", () => {
    const [result] = normalizeGreenhouseJobs({ jobs: [job] }, "Example Inc", "example", defaultProfile);
    expect(result).toMatchObject({ id: "greenhouse-example-7", externalId: "example:7", company: "Example Inc", source: "Greenhouse", remoteType: "Remote", applyUrl: "https://example.test/apply", matchScore: 100 });
    expect(result.description).not.toContain("<p>");
  });

  it("deduplicates repeated external job IDs", () => {
    expect(normalizeGreenhouseJobs({ jobs: [job, job] }, "", "example", defaultProfile)).toHaveLength(1);
  });

  it("handles an empty board response", () => {
    expect(normalizeGreenhouseJobs({}, "Example", "example", defaultProfile)).toEqual([]);
  });
});
