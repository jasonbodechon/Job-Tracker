import type { SearchProfile } from "../types/job";

export interface ProfileListDrafts { titles: string; requiredSkills: string; preferredSkills: string; countries: string; regions: string; cities: string; }

export function profileListDrafts(profile: SearchProfile): ProfileListDrafts {
  return { titles: profile.titles.join(", "), requiredSkills: profile.requiredSkills.join(", "), preferredSkills: profile.preferredSkills.join(", "), countries: (profile.countries ?? []).join(", "), regions: (profile.regions ?? []).join(", "), cities: (profile.cities ?? []).join(", ") };
}

export function parseProfileList(value: string): string[] {
  const seen = new Set<string>();
  return value.split(",").map((item) => item.trim()).filter((item) => {
    const key = item.toLowerCase();
    if (!key || seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
