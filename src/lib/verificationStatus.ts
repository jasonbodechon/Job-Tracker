import type { CompanyDirectoryEntry } from "../types/job";

export type VerificationSeverity = "verified" | "stale" | "pending" | "error";

export function verificationSeverity(status: string): VerificationSeverity {
  const normalized = status.trim().toLowerCase();
  if (normalized === "verified") return "verified";
  if (normalized === "stale") return "stale";
  if (normalized === "pending") return "pending";
  return "error";
}

export function verificationErrorCode(error?: string): string | undefined {
  return error?.match(/\[([A-Z][A-Z0-9_]*)\]/)?.[1];
}

export function verificationDetails(entry: CompanyDirectoryEntry): Array<{ label: string; value: string }> {
  const details: Array<{ label: string; value: string }> = [];
  const code = verificationErrorCode(entry.verificationError);
  if (code) details.push({ label: "Error code", value: code });
  if (entry.verificationError) details.push({ label: "Details", value: entry.verificationError });
  if (entry.detectedProvider) details.push({ label: "Detected provider", value: entry.detectedProvider });
  if (entry.redirectUrl) details.push({ label: "Redirect destination", value: entry.redirectUrl });
  return details;
}
