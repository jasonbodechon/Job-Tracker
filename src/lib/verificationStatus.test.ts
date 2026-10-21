import { describe, expect, it } from "vitest";
import { verificationDetails, verificationErrorCode, verificationSeverity } from "./verificationStatus";

describe("company verification status", () => {
  it("maps requested status severities", () => {
    expect(verificationSeverity("Verified")).toBe("verified");
    expect(verificationSeverity("Stale")).toBe("stale");
    expect(verificationSeverity("Pending")).toBe("pending");
    expect(verificationSeverity("Redirected")).toBe("error");
    expect(verificationSeverity("Provider change")).toBe("error");
  });

  it("extracts connector error codes and diagnostic fields", () => {
    const error = "Microsoft connector [HTTP_429]: Too Many Requests";
    expect(verificationErrorCode(error)).toBe("HTTP_429");
    expect(verificationDetails({
      id: "microsoft", enabled: true, companyName: "Microsoft", countries: [], atsProvider: "Microsoft",
      connectorClass: "Public site feed", boardIdentifier: "microsoft", careersUrl: "https://example.com",
      currentJobCount: 1, verificationStatus: "Redirected", identifierOrigin: "Manual",
      verificationError: error, detectedProvider: "Microsoft", redirectUrl: "https://redirect.example.com",
    })).toEqual([
      { label: "Error code", value: "HTTP_429" },
      { label: "Details", value: error },
      { label: "Detected provider", value: "Microsoft" },
      { label: "Redirect destination", value: "https://redirect.example.com" },
    ]);
  });
});
