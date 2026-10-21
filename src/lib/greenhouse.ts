import type { GreenhouseJob, Job, SearchProfile } from "../types/job";
import { matchGreenhouseJob } from "./matching";

export async function importGreenhouseBoard(boardToken: string, company: string, profile: SearchProfile): Promise<Job[]> {
  const token = boardToken.trim();
  if (!/^[a-zA-Z0-9_-]+$/.test(token)) throw new Error("Enter a valid Greenhouse board token.");
  const response = await fetch(`https://boards-api.greenhouse.io/v1/boards/${encodeURIComponent(token)}/jobs?content=true`);
  if (!response.ok) throw new Error(`Greenhouse returned ${response.status}. Check the board token.`);
  return normalizeGreenhouseJobs(await response.json() as { jobs: GreenhouseJob[] }, company, token, profile);
}

export function normalizeGreenhouseJobs(payload: { jobs?: GreenhouseJob[] }, company: string, boardToken: string, profile: SearchProfile): Job[] {
  const seen = new Set<number>();
  return (payload.jobs ?? []).filter((job) => !seen.has(job.id) && (seen.add(job.id), true)).map((job) => ({ ...matchGreenhouseJob(job, profile), id: `greenhouse-${boardToken}-${job.id}`, externalId: `${boardToken}:${job.id}`, source: "Greenhouse", company: company.trim() || boardToken, status: "New" }));
}
