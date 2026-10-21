import { describe, expect, it } from "vitest";
import { defaultProfile, demoJobs } from "./mockJobs";
import { filterAndSortJobs, formatDistance, formatScoreValue, jobMatchesProfileLocation, postedAtTimestamp } from "./jobResults";

const noFilters = { query: "", sources: [], statuses: [], profiles: [], postedFrom: "", postedTo: "", locationStatuses: [] };

describe("job result filtering and sorting", () => {
  it("formats null database distances without calling toFixed", () => {
    expect(formatDistance(null)).toBe("Distance not applicable");
    expect(formatDistance(undefined)).toBe("Distance not applicable");
    expect(formatDistance(12.345)).toBe("12.3 km");
  });
  it("only displays score decimals when a fractional value exists", () => {
    expect(formatScoreValue(10)).toBe("10");
    expect(formatScoreValue(10.5)).toBe("10.5");
    expect(formatScoreValue(10.25)).toBe("10.25");
  });
  it("sorts every visible data column in both directions", () => {
    for (const column of ["role", "match", "profiles", "source", "posted", "status", "distance"] as const) {
      const ascending = filterAndSortJobs(demoJobs, noFilters, column, "asc", new Date("2026-08-20T12:00:00"));
      const descending = filterAndSortJobs(demoJobs, noFilters, column, "desc", new Date("2026-08-20T12:00:00"));
      expect(ascending.map((job) => job.id)).toEqual(descending.map((job) => job.id).reverse());
    }
  });

  it("allows multiple sources and tracker statuses", () => {
    const jobs = filterAndSortJobs(demoJobs, { ...noFilters, sources: ["Greenhouse", "Ashby"], statuses: ["New", "Applied"] }, "match", "desc");
    expect(jobs.map((job) => job.id)).toEqual(["job-1", "job-3"]);
  });

  it("filters results by one or more matching profile names", () => {
    const jobs = demoJobs.map((job, index) => ({ ...job, profileMatches: [{ profileId: `profile-${index}`, profileName: index < 2 ? "Leadership" : "Developer", matchScore: job.matchScore, matchReason: job.matchReason }] }));
    expect(filterAndSortJobs(jobs, { ...noFilters, profiles: ["Leadership"] }, "profiles", "asc").map((job) => job.id)).toEqual(["job-1", "job-2"]);
  });

  it("filters an inclusive posted date range", () => {
    const jobs = filterAndSortJobs(demoJobs, { ...noFilters, postedFrom: "2026-08-18", postedTo: "2026-08-19" }, "posted", "desc", new Date("2026-08-20T12:00:00"));
    expect(jobs.map((job) => job.id)).toEqual(["job-2", "job-3"]);
    expect(postedAtTimestamp("not a date")).toBeNull();
  });

  it("filters and sorts location eligibility results", () => {
    const jobs = demoJobs.map((job,index)=>({...job,distanceKm:index===0?undefined:index*20,locationStatus:index===0?"Remote eligible":index===1?"Within range":"Broad location"}));
    expect(filterAndSortJobs(jobs,{...noFilters,locationStatuses:["Within range","Remote eligible"]},"distance","asc").map((job)=>job.id)).toEqual(["job-2","job-1"]);
  });
});

describe("profile location filtering", () => {
  it("matches one value per populated location level", () => {
    const profile = { ...defaultProfile, remoteOnly: false, countries: ["Canada"], regions: ["ON", "Ontario"], cities: ["Toronto", "Ottawa"] };
    expect(jobMatchesProfileLocation({ location: "Toronto, ON, Canada" }, profile)).toBe(true);
    expect(jobMatchesProfileLocation({ location: "Vancouver, BC, Canada" }, profile)).toBe(false);
  });

  it("does not apply location lists in remote-only mode", () => {
    const profile = { ...defaultProfile, remoteOnly: true, countries: ["Canada"], cities: ["Toronto"] };
    expect(jobMatchesProfileLocation({ location: "Remote - United States" }, profile)).toBe(true);
  });

  it("recognizes Canadian and US regions when boards omit the country name", () => {
    const canada = { ...defaultProfile, remoteOnly: false, countries: ["Canada"], regions: [], cities: [] };
    const unitedStates = { ...defaultProfile, remoteOnly: false, countries: ["United States"], regions: [], cities: [] };
    expect(jobMatchesProfileLocation({ location: "Kitchener, Ontario" }, canada)).toBe(true);
    expect(jobMatchesProfileLocation({ location: "Toronto, ON" }, canada)).toBe(true);
    expect(jobMatchesProfileLocation({ location: "Boston, MA" }, unitedStates)).toBe(true);
    expect(jobMatchesProfileLocation({ location: "Austin, TX" }, canada)).toBe(false);
  });

  it("does not filter on empty province/state and city lists", () => {
    const profile = { ...defaultProfile, remoteOnly: false, countries: ["Canada"], regions: [], cities: [] };
    expect(jobMatchesProfileLocation({ location: "Vancouver, BC" }, profile)).toBe(true);
  });
});
