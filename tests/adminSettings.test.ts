import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const admin = readFileSync(new URL("../src/AdminSettings.tsx", import.meta.url), "utf8");
const api = readFileSync(new URL("../server/src/api.rs", import.meta.url), "utf8");
const directory = readFileSync(new URL("../server/src/directory.rs", import.meta.url), "utf8");
const migration = readFileSync(new URL("../server/migrations/0009_administration.sql", import.meta.url), "utf8");
const scheduleMigration = readFileSync(new URL("../server/migrations/0010_site_schedule.sql", import.meta.url), "utf8");

describe("site administration", () => {
  it("shows one admin-only navigation entry and hides dashboard cards on admin pages", () => {
    expect(app).toContain('user.role === "admin"');
    expect(app).toContain('label: "Admin settings"');
    expect(app).not.toContain('setActive("Settings")');
    expect(app).not.toContain('setActive("Company directory")');
    expect(app).toContain('className={active === "Admin settings" ? "content admin-mode" : "content"}');
  });

  it("provides company, source, schedule, user, and data administration tabs without profiles", () => {
    for (const tab of ["Companies", "Sources", "Schedules", "Users", "Data"]) expect(admin).toContain(`\"${tab}\"`);
    expect(admin).not.toContain('tab === "Profiles"');
    for (const provider of ["SmartRecruiters", "Workday", "Adzuna"]) expect(admin).toContain(`\"${provider}\"`);
  });

  it("shows editable per-source request settings in the administrator Sources tab", () => {
    expect(admin).toContain('tab === "Sources" && sourceManagement');
    expect(app).toContain("Per-source request settings");
    for (const field of ["requestDelayMs", "maxRetries", "pageSize", "maxResults", "maxCalls", "hydrationLimit"]) expect(app).toContain(`name="${field}"`);
  });

  it("renders severity-colored company verification badges with diagnostic tooltips", () => {
    expect(admin).toContain("<VerificationStatus entry={entry}/>");
    expect(admin).toContain('role="tooltip"');
    expect(admin).toContain("verificationDetails(entry)");
  });

  it("removes Dashboard and user source navigation and isolates result cards to Job matches", () => {
    expect(app).not.toContain('label: "Dashboard"');
    expect(app).not.toContain('label: "Sources"');
    expect(app).toContain('useState("Job matches")');
    expect(app).toContain('active === "Job matches" && <section className="stat-grid"');
    expect(app).toContain('active === "Job matches" && <section className="results-card"');
  });

  it("shows the site schedule read-only on user schedule pages", () => {
    expect(app).toContain('className="site-schedule-readonly"');
    expect(app).toContain("Set by an administrator");
    expect(scheduleMigration).toContain("CREATE TABLE site_settings");
    expect(scheduleMigration).toContain("global_schedule");
  });

  it("protects administrator routes and synchronizes directory sources site-wide", () => {
    expect(api).toContain('if user.role!="admin"');
    expect(api).toContain("add_source_for_all");
    expect(api).toContain("remove_sources_for_all");
    expect(directory).toContain("sync_enabled_sources_for_user");
  });

  it("migrates one administrator and tracks globally selected directory sources", () => {
    expect(migration).toContain("ADD COLUMN role");
    expect(migration).toContain("ADD COLUMN source_enabled");
    expect(migration).toContain("SET role='admin'");
  });
});
