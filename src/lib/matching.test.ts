import { describe, expect, it } from "vitest";
import { extractResumeKeywords, normalizeWeights, scoreJob, selectBestProfileMatches } from "./matching";
import { defaultProfile, demoJobs } from "./mockJobs";

describe("scoreJob", () => {
  it("rewards matching title, required skills, remote preference, and freshness", () => {
    expect(scoreJob({ title: "Senior Engineering Manager", skills: ["TypeScript", "React", "Leadership"], remoteType: "Remote", postedAt: "Today" }, defaultProfile)).toBe(100);
  });

  it("keeps location/work style outside the fit score", () => {
    expect(scoreJob({ title: "Senior Engineering Manager", skills: ["TypeScript", "React", "Leadership"], remoteType: "On-site", postedAt: "3 days ago" }, defaultProfile)).toBe(100);
  });

  it("awards preferred skills proportionally without overpowering required skills", () => {
    const profile = { ...defaultProfile, preferredSkills: ["Kubernetes", "AWS"] };
    const onePreferred = scoreJob({ title: "Senior Engineering Manager", skills: ["TypeScript", "React", "Leadership", "AWS"], remoteType: "Remote", postedAt: "Yesterday" }, profile);
    const allPreferred = scoreJob({ title: "Senior Engineering Manager", skills: ["TypeScript", "React", "Leadership", "AWS", "Kubernetes"], remoteType: "Remote", postedAt: "Yesterday" }, profile);
    expect(onePreferred).toBe(92);
    expect(allPreferred).toBe(100);
  });

  it("keeps useful technical resume tokens without turning punctuation into skills", () => {
    expect(extractResumeKeywords("TypeScript, C++, CI/CD; led 12 engineers.")).toEqual(["TypeScript", "C++", "led", "engineers"]);
  });

  it("normalizes custom weights to a 100-point scale", () => {
    const weights = normalizeWeights({ title: 1, requiredSkills: 1, preferredSkills: 1, resumeEvidence: 1, experience: 1, education: 1, proximity: 1 });
    expect(Object.values(weights).reduce((sum, value) => sum + value, 0)).toBeCloseTo(100);
    expect(weights.title).toBeCloseTo(100 / 7);
  });

  it("keeps the strongest match when multiple profiles score the same posting", () => {
    const lower = { ...demoJobs[0], matchScore: 61, matchReason: "Profile A", profileMatches: [{ profileId: "a", profileName: "Profile A", matchScore: 61, matchReason: "Profile A" }] };
    const higher = { ...demoJobs[0], matchScore: 88, matchReason: "Profile B", profileMatches: [{ profileId: "b", profileName: "Profile B", matchScore: 88, matchReason: "Profile B" }] };
    const [result] = selectBestProfileMatches([[lower], [higher]]);
    expect(result.matchScore).toBe(88);
    expect(result.matchReason).toBe("Profile B");
    expect(result.profileMatches?.map((match) => match.profileName)).toEqual(["Profile B", "Profile A"]);
  });
});
