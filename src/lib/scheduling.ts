import type { SearchProfile } from "../types/job";

export function isScheduleDue(schedule: SearchProfile["schedule"], lastRunAt: string | undefined, now = new Date()): boolean {
  if (schedule === "Manual") return false;
  const last = lastRunAt ? new Date(lastRunAt.replace(" ", "T") + (lastRunAt.includes("Z") ? "" : "Z")) : undefined;
  if (last && Number.isFinite(last.getTime())) {
    const elapsedHours = (now.getTime() - last.getTime()) / 3_600_000;
    if (schedule === "Weekly") return elapsedHours >= 168;
    if (elapsedHours < 20) return false;
  }
  const day = now.getDay();
  return schedule !== "Weekdays" || (day !== 0 && day !== 6);
}

export function dueScheduledProfiles(profiles: SearchProfile[], lastRunAt: string | undefined, now = new Date()): SearchProfile[] {
  return profiles.filter((profile) => profile.enabled && isScheduleDue(profile.schedule, lastRunAt, now));
}
