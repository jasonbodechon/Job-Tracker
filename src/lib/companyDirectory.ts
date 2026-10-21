import type { CompanyDirectoryEntry, JobSourceConfig, JobSourceKind } from "../types/job";

export function filterCompanyDirectory(entries: CompanyDirectoryEntry[], query: string, countries: string[], providers: JobSourceKind[]) {
  const needle = query.trim().toLocaleLowerCase();
  return entries.filter((entry) => {
    const companyMatches = !needle || entry.companyName.toLocaleLowerCase().includes(needle);
    const countryMatches = countries.length === 0 || countries.some((country) => entry.countries.includes(country));
    const providerMatches = providers.length === 0 || providers.includes(entry.atsProvider);
    return companyMatches && countryMatches && providerMatches;
  });
}

export const directorySourceId = (entry: Pick<CompanyDirectoryEntry, "id">) => `directory-${entry.id}`;

const normalizedIdentity = (value: string) => value.trim().replace(/\/+$/, "").toLocaleLowerCase();

export function directorySourceIdentity(entry: CompanyDirectoryEntry) {
  const kind = entry.identifierOrigin === "Automatic" && entry.detectedProvider ? entry.detectedProvider : entry.atsProvider;
  const boardToken = kind === "Workday" || kind === "CareerPage" ? entry.careersUrl : entry.boardIdentifier;
  return { company: normalizedIdentity(entry.companyName), kind, boardToken: normalizedIdentity(boardToken) };
}

export function findDirectorySource(entry: CompanyDirectoryEntry, sources: JobSourceConfig[]) {
  const identity = directorySourceIdentity(entry);
  return sources.find((source) => normalizedIdentity(source.company) === identity.company && source.kind === identity.kind && normalizedIdentity(source.boardToken) === identity.boardToken)
    ?? sources.find((source) => source.id === directorySourceId(entry));
}
