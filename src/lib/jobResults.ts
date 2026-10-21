import type { Job, JobSourceKind, JobStatus, ProfileMatch, SearchProfile } from "../types/job";
import { locationGroupMatches, normalizeLocation } from "./locationMatching";

export type SortColumn = "role" | "match" | "profiles" | "source" | "posted" | "status" | "distance";
export type SortDirection = "asc" | "desc";

export interface JobResultFilters {
  query: string;
  sources: JobSourceKind[];
  statuses: JobStatus[];
  profiles: string[];
  postedFrom: string;
  postedTo: string;
  locationStatuses: string[];
}

function startOfDay(value: Date): number {
  return new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime();
}

export function postedAtTimestamp(postedAt: string, now = new Date()): number | null {
  const normalized = postedAt.trim().toLowerCase();
  const today = startOfDay(now);
  if (normalized === "today") return today;
  if (normalized === "yesterday") return today - 86_400_000;
  const daysAgo = normalized.match(/^(\d+)\s+days?\s+ago$/);
  if (daysAgo) return today - Number(daysAgo[1]) * 86_400_000;
  const parsed = new Date(postedAt);
  return Number.isNaN(parsed.getTime()) ? null : startOfDay(parsed);
}

export function formatDistance(distanceKm: number | null | undefined): string {
  return distanceKm == null ? "Distance not applicable" : `${distanceKm.toFixed(1)} km`;
}

export function formatScoreValue(value: number): string {
  return new Intl.NumberFormat("en-CA", { maximumFractionDigits: 2 }).format(value);
}

export function jobMatchesProfileLocation(job: Pick<Job, "location">, profile: SearchProfile): boolean {
  if (profile.remoteOnly) return true;
  const location = normalizeLocation(job.location);
  return locationGroupMatches(location, profile.countries ?? [], true)
    && locationGroupMatches(location, profile.regions ?? [])
    && locationGroupMatches(location, profile.cities ?? []);
}

export function getJobProfileMatches(job: Pick<Job, "profileMatches" | "matchScore" | "matchReason">): ProfileMatch[] {
  if (job.profileMatches?.length) return job.profileMatches;
  const separator = job.matchReason.indexOf(":");
  const profileName = separator > 0 ? job.matchReason.slice(0, separator).trim() : "Matched profile";
  return [{ profileId: profileName, profileName, matchScore: job.matchScore, matchReason: job.matchReason }];
}

export function filterAndSortJobs(jobs: Job[], filters: JobResultFilters, sortColumn: SortColumn, sortDirection: SortDirection, now = new Date()): Job[] {
  const query = filters.query.trim().toLocaleLowerCase();
  const from = filters.postedFrom ? startOfDay(new Date(`${filters.postedFrom}T00:00:00`)) : null;
  const to = filters.postedTo ? startOfDay(new Date(`${filters.postedTo}T00:00:00`)) : null;
  const filtered = jobs.filter((job) => {
    if (filters.sources.length && !filters.sources.includes(job.source)) return false;
    if (filters.statuses.length && !filters.statuses.includes(job.status)) return false;
    if (filters.profiles.length && !getJobProfileMatches(job).some((match) => filters.profiles.includes(match.profileName))) return false;
    if (filters.locationStatuses.length && !filters.locationStatuses.includes(job.locationStatus ?? "Not evaluated")) return false;
    if (query && !`${job.title} ${job.company} ${job.location} ${job.skills.join(" ")}`.toLocaleLowerCase().includes(query)) return false;
    const posted = postedAtTimestamp(job.postedAt, now);
    if (from !== null && (posted === null || posted < from)) return false;
    if (to !== null && (posted === null || posted > to)) return false;
    return true;
  });
  const value = (job: Job): string | number => {
    if (sortColumn === "role") return `${job.title}\u0000${job.company}\u0000${job.location}`.toLocaleLowerCase();
    if (sortColumn === "match") return job.matchScore;
    if (sortColumn === "profiles") return getJobProfileMatches(job).map((match) => match.profileName).sort().join(", ").toLocaleLowerCase();
    if (sortColumn === "source") return job.source.toLocaleLowerCase();
    if (sortColumn === "status") return job.status.toLocaleLowerCase();
    if (sortColumn === "distance") return job.distanceKm ?? Number.POSITIVE_INFINITY;
    return postedAtTimestamp(job.postedAt, now) ?? Number.NEGATIVE_INFINITY;
  };
  return filtered.map((job, index) => ({ job, index })).sort((left, right) => {
    const a = value(left.job); const b = value(right.job);
    const comparison = typeof a === "number" && typeof b === "number" ? a - b : String(a).localeCompare(String(b));
    return (comparison || left.index - right.index) * (sortDirection === "asc" ? 1 : -1);
  }).map(({ job }) => job);
}
