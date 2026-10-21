import { defaultMatchingWeights, type GreenhouseJob, type Job, type MatchingWeights, type SearchProfile } from "../types/job";

export function normalizeWeights(weights?: MatchingWeights): MatchingWeights {
  const source = weights ?? defaultMatchingWeights;
  const values = [source.title,source.requiredSkills,source.preferredSkills,source.resumeEvidence,source.experience,source.education,source.proximity ?? defaultMatchingWeights.proximity].map((value) => Number.isFinite(value) ? Math.max(0, value) : 0);
  const total = values.reduce((sum, value) => sum + value, 0) || 1;
  return { title: values[0] / total * 100, requiredSkills: values[1] / total * 100, preferredSkills: values[2] / total * 100, resumeEvidence: values[3] / total * 100, experience: values[4] / total * 100, education: values[5] / total * 100, proximity: values[6] / total * 100 };
}

export function scoreJob(job: Pick<Job, "title" | "skills" | "remoteType" | "postedAt">, profile: SearchProfile): number {
  const weights = normalizeWeights(profile.weights);
  const requiredSkills = profile.requiredSkills.filter((skill) => skill.trim());
  const preferredSkills = profile.preferredSkills.filter((skill) => skill.trim());
  const title = job.title.toLowerCase();
  const titleHit = profile.titles.some((candidate) => candidate.trim() && title.includes(candidate.toLowerCase())) ? weights.title : 0;
  const skills = new Set(job.skills.map((skill) => skill.toLowerCase()));
  const requiredMatches = requiredSkills.filter((skill) => skills.has(skill.toLowerCase())).length;
  const preferredMatches = preferredSkills.filter((skill) => skills.has(skill.toLowerCase())).length;
  const components = [{ earned: titleHit, possible: profile.titles.length ? weights.title : 0 }];
  if (requiredSkills.length) components.push({ earned: (requiredMatches / requiredSkills.length) * weights.requiredSkills, possible: weights.requiredSkills });
  if (preferredSkills.length) components.push({ earned: (preferredMatches / preferredSkills.length) * weights.preferredSkills, possible: weights.preferredSkills });
  const resumeTerms = new Set(extractResumeKeywords(profile.resumeText ?? "").map((term) => term.toLowerCase()));
  if (resumeTerms.size && skills.size) components.push({ earned: [...skills].filter((skill) => resumeTerms.has(skill)).length / skills.size * weights.resumeEvidence, possible: weights.resumeEvidence });
  const possible = components.reduce((sum, component) => sum + component.possible, 0) || 1;
  return Math.round(Math.min(100, components.reduce((sum, component) => sum + component.earned, 0) / possible * 100));
}

export interface NormalizedPosting { title: string; location: string; description: string; remoteType: Job["remoteType"]; postedAt: string; applyUrl: string; }

export function matchPosting(job: NormalizedPosting, profile: SearchProfile): Omit<Job, "id" | "externalId" | "source" | "company" | "status"> {
  const corpus = `${job.title} ${job.location} ${job.description}`.toLowerCase();
  const requiredSkills = profile.requiredSkills.filter((skill) => skill.trim());
  const preferredSkills = profile.preferredSkills.filter((skill) => skill.trim());
  const candidates = [...requiredSkills, ...preferredSkills, ...extractResumeKeywords(profile.resumeText ?? "")].filter((candidate) => candidate.trim());
  const skills = [...new Set(candidates.filter((skill) => corpus.includes(skill.toLowerCase())))];
  const required = requiredSkills.filter((skill) => skills.some((found) => found.toLowerCase() === skill.toLowerCase()));
  const preferred = preferredSkills.filter((skill) => skills.some((found) => found.toLowerCase() === skill.toLowerCase()));
  const missing = requiredSkills.filter((skill) => !required.some((found) => found.toLowerCase() === skill.toLowerCase()));
  const matchReason = `${profile.name}: ${required.length}/${requiredSkills.length} required and ${preferred.length}/${preferredSkills.length} preferred skills found${missing.length ? `. Missing required: ${missing.join(", ")}.` : "."}`;
  const matchScore = scoreJob({ ...job, skills }, profile);
  return { ...job, skills, matchScore, matchReason, profileMatches: [{ profileId: profile.id, profileName: profile.name, matchScore, matchReason }], isNew: true };
}

export function matchGreenhouseJob(job: GreenhouseJob, profile: SearchProfile): Omit<Job, "id" | "externalId" | "source" | "company" | "status"> {
  const plainText = job.content.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
  const remoteType: Job["remoteType"] = /remote/.test(`${job.location?.name ?? ""} ${plainText}`.toLowerCase()) ? "Remote" : "Unknown";
  return matchPosting({ title: job.title, location: job.location?.name || "Location not provided", remoteType, postedAt: new Date(job.updated_at).toLocaleDateString(), applyUrl: job.absolute_url, description: plainText }, profile);
}

export function extractResumeKeywords(resumeText: string) {
  return resumeText.split(/[^a-zA-Z0-9+#.-]+/).map((word) => word.replace(/(^\.+|\.+$)/g, "")).filter((word) => word.length >= 3).slice(0, 80);
}

export function selectBestProfileMatches(groups: Job[][]): Job[] {
  const best = new Map<string, Job>();
  for (const jobs of groups) for (const job of jobs) {
    const previous = best.get(job.externalId);
    if (!previous) { best.set(job.externalId, job); continue; }
    const profileMatches = [...(previous.profileMatches ?? []), ...(job.profileMatches ?? [])]
      .filter((match, index, matches) => matches.findIndex((candidate) => candidate.profileId === match.profileId) === index)
      .sort((left, right) => right.matchScore - left.matchScore || left.profileName.localeCompare(right.profileName));
    const winner = job.matchScore > previous.matchScore ? job : previous;
    best.set(job.externalId, { ...winner, profileMatches });
  }
  return [...best.values()];
}
