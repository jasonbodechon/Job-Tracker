import type { Job, SearchProfile } from "../types/job";
import { matchPosting } from "./matching";

type JsonRecord = Record<string, any>;

function text(value: unknown, fallback = ""): string { return typeof value === "string" && value.trim() ? value.trim() : fallback; }
function plain(value: unknown): string { return text(value).replace(/<[^>]+>/g, " ").replace(/&nbsp;|&#160;/gi, " ").replace(/&amp;/gi, "&").replace(/\s+/g, " ").trim(); }
function remoteType(...parts: unknown[]): Job["remoteType"] {
  const corpus = parts.map((part) => text(part)).join(" ").toLowerCase();
  if (/\bremote\b/.test(corpus)) return "Remote";
  if (/\bhybrid\b/.test(corpus)) return "Hybrid";
  if (/on[- ]?site|in[- ]office/.test(corpus)) return "On-site";
  return "Unknown";
}
function enabled(value: unknown): boolean { return value === true || (typeof value === "string" && value.toLowerCase() === "true"); }
function unique<T>(items: T[], key: (item: T) => string): T[] { const seen = new Set<string>(); return items.filter((item) => { const value = key(item); if (!value || seen.has(value)) return false; seen.add(value); return true; }); }

export function normalizeLeverJobs(payload: unknown, company: string, site: string, profile: SearchProfile): Job[] {
  const rows = Array.isArray(payload) ? payload as JsonRecord[] : [];
  return unique(rows, (row) => text(row.id)).map((row) => {
    const description = plain(row.descriptionPlain || row.description || row.additionalPlain);
    const location = text(row.categories?.location, "Location not provided");
    const type = remoteType(row.workplaceType, location, description);
    const id = text(row.id);
    return { ...matchPosting({ title: text(row.text, "Untitled role"), location, description, remoteType: type, postedAt: row.createdAt ? new Date(row.createdAt).toLocaleDateString() : "Not provided", applyUrl: text(row.applyUrl || row.hostedUrl) }, profile), id: `lever-${site}-${id}`, externalId: `${site}:${id}`, source: "Lever", company: company.trim() || site, status: "New" };
  });
}

export function normalizeAshbyJobs(payload: unknown, company: string, board: string, profile: SearchProfile): Job[] {
  const rows = Array.isArray((payload as JsonRecord)?.jobs) ? (payload as JsonRecord).jobs as JsonRecord[] : [];
  return unique(rows, (row) => text(row.id || row.jobUrl)).map((row) => {
    const description = plain(row.descriptionPlain || row.descriptionHtml || row.description);
    const location = text(row.location, "Location not provided");
    const type = remoteType(row.workplaceType, row.isRemote ? "remote" : "", location, description);
    const id = text(row.id || row.jobUrl);
    return { ...matchPosting({ title: text(row.title, "Untitled role"), location, description, remoteType: type, postedAt: row.publishedAt ? new Date(row.publishedAt).toLocaleDateString() : "Not provided", applyUrl: text(row.applyUrl || row.jobUrl) }, profile), id: `ashby-${board}-${id}`, externalId: `${board}:${id}`, source: "Ashby", company: company.trim() || board, status: "New" };
  });
}

export function normalizeUsaJobs(payload: unknown, profile: SearchProfile): Job[] {
  const rows = (payload as JsonRecord)?.SearchResult?.SearchResultItems;
  return unique(Array.isArray(rows) ? rows as JsonRecord[] : [], (row) => text(row.MatchedObjectId)).map((row) => {
    const item = row.MatchedObjectDescriptor ?? {};
    const details = item.UserArea?.Details ?? {};
    const location = Array.isArray(item.PositionLocationDisplay) ? item.PositionLocationDisplay.join("; ") : text(item.PositionLocationDisplay, "Location not provided");
    const description = plain([details.JobSummary, details.MajorDuties?.join?.(" "), details.Requirements, details.Qualifications].filter(Boolean).join(" "));
    const id = text(row.MatchedObjectId || item.PositionID);
    return { ...matchPosting({ title: text(item.PositionTitle, "Untitled role"), location, description, remoteType: remoteType(enabled(details.RemoteIndicator) ? "remote" : "", enabled(details.TeleworkEligible) ? "hybrid" : "", location), postedAt: text(item.PublicationStartDate, "Not provided"), applyUrl: text(item.PositionURI) }, profile), id: `usajobs-${id}`, externalId: id, source: "USAJOBS", company: text(item.OrganizationName || item.DepartmentName, "U.S. Federal Government"), status: "New" };
  });
}
