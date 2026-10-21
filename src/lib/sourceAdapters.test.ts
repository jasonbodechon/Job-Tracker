import { describe, expect, it } from "vitest";
import { defaultProfile } from "./mockJobs";
import { normalizeAshbyJobs, normalizeLeverJobs, normalizeUsaJobs } from "./sourceAdapters";

describe("normalizeLeverJobs", () => {
  it("normalizes, scores, and deduplicates Lever postings", () => {
    const posting = { id: "lv-1", text: "Senior Engineering Manager", categories: { location: "Canada" }, descriptionPlain: "Lead TypeScript, React, and Leadership teams.", hostedUrl: "https://jobs.lever.co/acme/lv-1", applyUrl: "https://jobs.lever.co/acme/lv-1/apply", workplaceType: "remote", createdAt: 1787097600000 };
    const jobs = normalizeLeverJobs([posting, posting], "Acme", "acme", defaultProfile);
    expect(jobs).toHaveLength(1);
    expect(jobs[0]).toMatchObject({ source: "Lever", externalId: "acme:lv-1", company: "Acme", remoteType: "Remote", matchScore: 100, applyUrl: posting.applyUrl });
  });
});

describe("normalizeAshbyJobs", () => {
  it("normalizes an Ashby board payload and strips HTML", () => {
    const payload = { jobs: [{ id: "as-1", title: "Senior Engineering Manager", location: "Toronto", workplaceType: "Hybrid", descriptionHtml: "<p>TypeScript React Leadership</p>", jobUrl: "https://jobs.ashbyhq.com/acme/as-1", applyUrl: "https://jobs.ashbyhq.com/acme/as-1/application", publishedAt: "2026-08-20T10:00:00Z" }] };
    const [job] = normalizeAshbyJobs(payload, "Acme", "acme", defaultProfile);
    expect(job).toMatchObject({ source: "Ashby", externalId: "acme:as-1", remoteType: "Hybrid", applyUrl: payload.jobs[0].applyUrl });
    expect(job.description).toBe("TypeScript React Leadership");
  });
});

describe("normalizeUsaJobs", () => {
  it("maps nested USAJOBS response fields and agency ownership", () => {
    const payload = { SearchResult: { SearchResultItems: [{ MatchedObjectId: "USA-42", MatchedObjectDescriptor: { PositionTitle: "Senior Engineering Manager", OrganizationName: "Department of Example", PositionLocationDisplay: ["Washington, DC", "Remote"], PublicationStartDate: "2026-08-20", PositionURI: "https://www.usajobs.gov/job/42", UserArea: { Details: { JobSummary: "Lead TypeScript and React programs.", MajorDuties: ["Provide Leadership."], RemoteIndicator: true } } } }] } };
    const [job] = normalizeUsaJobs(payload, defaultProfile);
    expect(job).toMatchObject({ id: "usajobs-USA-42", source: "USAJOBS", company: "Department of Example", remoteType: "Remote", matchScore: 100, applyUrl: "https://www.usajobs.gov/job/42" });
  });

  it("handles an empty search response", () => expect(normalizeUsaJobs({}, defaultProfile)).toEqual([]));
});
