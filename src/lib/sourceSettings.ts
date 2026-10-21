import type { JobSourceConfig, JobSourceKind } from "../types/job";

export type SourceTuning = Required<Pick<JobSourceConfig, "requestDelayMs" | "maxRetries" | "maxResults" | "pageSize" | "maxCalls" | "hydrationLimit">>;

export const connectorDefaults: Record<JobSourceKind, SourceTuning> = {
  Amazon: { requestDelayMs: 250, maxRetries: 4, maxResults: 1000, pageSize: 100, maxCalls: 10, hydrationLimit: 0 },
  USAJOBS: { requestDelayMs: 100, maxRetries: 3, maxResults: 2500, pageSize: 250, maxCalls: 10, hydrationLimit: 0 },
  SmartRecruiters: { requestDelayMs: 125, maxRetries: 3, maxResults: 2000, pageSize: 100, maxCalls: 20, hydrationLimit: 0 },
  Workday: { requestDelayMs: 250, maxRetries: 3, maxResults: 1000, pageSize: 20, maxCalls: 50, hydrationLimit: 0 },
  Adzuna: { requestDelayMs: 250, maxRetries: 3, maxResults: 1000, pageSize: 20, maxCalls: 50, hydrationLimit: 0 },
  Google: { requestDelayMs: 250, maxRetries: 3, maxResults: 1000, pageSize: 20, maxCalls: 50, hydrationLimit: 0 },
  Greenhouse: { requestDelayMs: 0, maxRetries: 3, maxResults: 5000, pageSize: 100, maxCalls: 1, hydrationLimit: 0 },
  Lever: { requestDelayMs: 0, maxRetries: 3, maxResults: 5000, pageSize: 100, maxCalls: 1, hydrationLimit: 0 },
  Ashby: { requestDelayMs: 0, maxRetries: 3, maxResults: 5000, pageSize: 100, maxCalls: 1, hydrationLimit: 0 },
  Microsoft: { requestDelayMs: 1000, maxRetries: 4, maxResults: 1000, pageSize: 10, maxCalls: 100, hydrationLimit: 50 },
  CareerPage: { requestDelayMs: 0, maxRetries: 3, maxResults: 5000, pageSize: 100, maxCalls: 1, hydrationLimit: 0 },
};

export function effectiveSourceTuning(source: Pick<JobSourceConfig, "kind" | keyof SourceTuning>): SourceTuning {
  const defaults = connectorDefaults[source.kind];
  return Object.fromEntries(Object.entries(defaults).map(([key, value]) => [key, Number(source[key as keyof SourceTuning]) || value])) as unknown as SourceTuning;
}
