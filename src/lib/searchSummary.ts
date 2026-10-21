import type { ImportResult } from "../types/job";

export function describeSearchResult(result: ImportResult): string {
  const details: string[] = [];
  if (typeof result.fetched === "number") details.push(`${result.fetched} fetched`);
  if (result.truncatedSources?.length) details.push(`source limit reached: ${result.truncatedSources.join(", ")}`);
  if (result.outsideLocation) details.push(`${result.outsideLocation} outside the remote/location settings`);
  if (result.belowMinimumScore) details.push(`${result.belowMinimumScore} below the minimum score`);
  if (result.sourceErrors?.length) details.push(`${result.sourceErrors.length} source error${result.sourceErrors.length === 1 ? "" : "s"}`);
  return details.length ? `${result.imported} matching jobs imported (${details.join("; ")}).` : `${result.imported} active jobs imported, ${result.inactive} older jobs marked inactive.`;
}

export function noResultsGuidance(result: ImportResult): string {
  if (result.sourceErrors?.length) return ` Check Sources for the error details: ${result.sourceErrors.join("; ")}`;
  if (result.outsideLocation) return " Review Distance and work-style eligibility: increase the kilometre range, include the job's work style, or choose Show as location review for broad locations.";
  if (result.belowMinimumScore) return " Try lowering Minimum score in the active profile.";
  return " Check that at least one profile exists and a source is enabled, then try refreshing an individual source from Sources.";
}
