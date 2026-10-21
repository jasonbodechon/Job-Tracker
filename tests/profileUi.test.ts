import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const serviceSource = readFileSync(new URL("../server/src/service.rs", import.meta.url), "utf8");
const apiSource = readFileSync(new URL("../server/src/api.rs", import.meta.url), "utf8");
const stylesSource = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const featuresSource = readFileSync(new URL("../src/features.css", import.meta.url), "utf8");

describe("profile search controls", () => {
  it("places the active profile card before the resume and search profile card", () => {
    const activeProfile = appSource.indexOf('{active === "Profiles" && activeProfileCard}');
    const resumeProfiles = appSource.indexOf("<h2>Resume and search profiles</h2>");
    expect(activeProfile).toBeGreaterThan(-1);
    expect(resumeProfiles).toBeGreaterThan(activeProfile);
  });

  it("places distance controls below the resume parser in the profile card", () => {
    const resume = appSource.indexOf('placeholder="Extracted resume text appears here"');
    const distance = appSource.indexOf('className="distance-settings-inline"');
    expect(resume).toBeGreaterThan(-1);
    expect(distance).toBeGreaterThan(resume);
    expect(appSource.match(/Distance and work-style eligibility/g)).toHaveLength(1);
  });

  it("does not expose legacy profile eligibility controls", () => {
    expect(appSource).not.toContain("Remote only");
    expect(appSource).not.toContain("Allowed job locations");
    expect(appSource).not.toContain("Include in searches</label>");
    expect(appSource).not.toContain("Use distance from an active location");
  });

  it("always searches every profile", () => {
    expect(serviceSource).not.toContain("filter(|p|p.enabled)");
    expect(serviceSource).not.toContain("profiles WHERE enabled=true");
    expect(apiSource).toContain("p.enabled=true");
  });

  it("keeps distance eligibility active for loaded and saved profiles", () => {
    expect(appSource).toContain("distanceEnabled:true");
    expect(apiSource).toContain("p.distance_enabled=true");
  });

  it("automatically dismisses status messages after 30 seconds", () => {
    expect(appSource).toContain("MESSAGE_TIMEOUT_MS = 30_000");
    expect(appSource).toContain('window.setTimeout(() => setMessage(""), MESSAGE_TIMEOUT_MS)');
  });

  it("aligns weight inputs independently of wrapped labels", () => {
    expect(appSource).toContain("<span>{label} weight</span>");
    expect(featuresSource).toContain(".weight-grid label > span");
    expect(featuresSource).toContain("min-height: 3em");
  });

  it("uses DM Sans headings and a viewport-fixed application shell", () => {
    expect(stylesSource).not.toContain("Fraunces");
    expect(stylesSource).toContain("h1, h2, h3, h4 { font-family: 'DM Sans', sans-serif; }");
    expect(stylesSource).toContain(".app-shell { display: flex; height: 100vh; overflow: hidden; }");
    expect(stylesSource).toContain(".content { flex: 1; height: 100vh; min-width: 0; overflow-y: auto;");
  });
});
