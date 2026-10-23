export type JobStatus = "New" | "Saved" | "Applied" | "Interviewing" | "Offer" | "Rejected" | "Archived";
export type RemoteType = "Remote" | "Hybrid" | "On-site" | "Unknown";

export interface ScoreComponent { key: string; label: string; earned: number; possible: number; details: string; }
export interface ProfileMatch { profileId: string; profileName: string; matchScore: number; matchReason: string; scoreBreakdown?: ScoreComponent[]; matchedTerms?: string[]; missingTerms?: string[]; resumeYears?: number | null; requiredYears?: number | null; algorithmVersion?: string; distanceKm?: number | null; locationStatus?: string; locationDetails?: string; }

export interface Job {
  id: string;
  source: "USAJOBS" | "Greenhouse" | "Lever" | "Ashby" | "Amazon" | "SmartRecruiters" | "Workday" | "Microsoft" | "Google" | "CareerPage" | "Adzuna";
  externalId: string;
  title: string;
  company: string;
  location: string;
  remoteType: RemoteType;
  postedAt: string;
  applyUrl: string;
  description: string;
  skills: string[];
  matchScore: number;
  matchReason: string;
  profileMatches?: ProfileMatch[];
  isNew: boolean;
  status: JobStatus;
  distanceKm?: number | null;
  locationStatus?: string;
}

export interface MatchingWeights { title: number; requiredSkills: number; preferredSkills: number; resumeEvidence: number; experience: number; education: number; proximity: number; }
export const defaultMatchingWeights: MatchingWeights = { title: 20, requiredSkills: 30, preferredSkills: 10, resumeEvidence: 20, experience: 15, education: 5, proximity: 10 };
export interface SearchProfile { id: string; name: string; titles: string[]; requiredSkills: string[]; preferredSkills: string[]; remoteOnly: boolean; countries: string[]; regions: string[]; cities: string[]; minScore: number; enabled: boolean; schedule: "Manual" | "Daily" | "Weekdays" | "Weekly"; resumeText: string; weights: MatchingWeights; distanceEnabled: boolean; activeLocation: string; activeLatitude?: number; activeLongitude?: number; maximumDistanceKm: number; includeRemote: boolean; includeHybrid: boolean; includeOnsite: boolean; broadLocationPolicy: "review" | "exclude"; }

export type JobSourceKind = Job["source"];
export interface JobSourceConfig { id: string; kind: JobSourceKind; company: string; boardToken: string; location?: string; enabled: boolean; lastRefreshedAt?: string; lastError?: string; requestDelayMs?: number; maxRetries?: number; maxResults?: number; pageSize?: number; maxCalls?: number; hydrationLimit?: number; }
export type ConnectorClass = "Official API" | "Public site feed" | "Public page extraction";
export interface CompanyDirectoryEntry {
  id: string;
  enabled: boolean;
  companyName: string;
  countries: string[];
  atsProvider: JobSourceKind;
  connectorClass: ConnectorClass;
  boardIdentifier: string;
  careersUrl: string;
  currentJobCount: number;
  lastVerifiedAt?: string;
  verificationStatus: "Pending" | "Verified" | "Partial" | "Stale" | "Redirected" | "Provider change";
  verificationError?: string;
  identifierOrigin: "Manual" | "Automatic";
  detectedProvider?: JobSourceKind;
  redirectUrl?: string;
}
export interface ImportResult {
  imported: number;
  inactive: number;
  fetched?: number;
  available?: number;
  outsideLocation?: number;
  belowMinimumScore?: number;
  sourceErrors?: string[];
  truncatedSources?: string[];
}
export type SearchRunStatus = "Running" | "Completed" | "Failed" | "Abandoned";
export interface SearchRun { id: string; startedAt: string; finishedAt?: string; triggerType: string; sourcesChecked: number; jobsImported: number; errors: string[]; status: SearchRunStatus; progress: string; result?: ImportResult | null; }
/** Searches run in the background on the server; starting one returns the run to follow. */
export interface StartedSearch { runId: string; }
export interface TrackerDetails { jobId: string; status: JobStatus; notes: string; contactName: string; contactEmail: string; applicationDate?: string; interviewDate?: string; followUpAt?: string; }
export interface Diagnostics { databasePath: string; databaseSizeBytes: number; activeJobs: number; inactiveJobs: number; profiles: number; enabledSources: number; searchRuns: number; sourceErrors: number; dueFollowUps: number; }

export interface GreenhouseJob {
  id: number; title: string; absolute_url: string; updated_at: string; content: string;
  location?: { name?: string }; departments?: Array<{ name?: string }>;
}

export interface RawSourcePayload { kind: JobSourceKind; payload: unknown; }
