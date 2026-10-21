import type { Job } from "../types/job";

export type SourceId = Job["source"];
export interface SourceConfig { source: SourceId; company?: string; boardToken?: string; enabled: boolean; }
export type ImportedJob = Omit<Job, "id" | "matchScore" | "matchReason" | "isNew" | "status">;
export interface JobSource { id: SourceId; fetch(config: SourceConfig, signal?: AbortSignal): Promise<ImportedJob[]>; }

// Each live source adapter will live behind this contract. It keeps source-specific
// API details out of matching, persistence, and UI code.
export const sourceLabels: Record<SourceId, string> = { USAJOBS: "USAJOBS", Greenhouse: "Greenhouse", Lever: "Lever", Ashby: "Ashby", Amazon: "Amazon Jobs", SmartRecruiters: "SmartRecruiters", Workday: "Workday", Microsoft: "Microsoft Careers", Google: "Google Careers", CareerPage: "Structured career page", Adzuna: "Adzuna" };
