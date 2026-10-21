import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const connector = readFileSync(new URL("../server/src/connectors.rs", import.meta.url), "utf8");
const migration = readFileSync(new URL("../server/migrations/0013_microsoft_detail_hydration.sql", import.meta.url), "utf8");

describe("Microsoft detail hydration", () => {
  it("uses the current Eightfold envelope and detail endpoint", () => {
    expect(connector).toContain('envelope.get("data").unwrap_or(envelope)');
    expect(connector).toContain("/api/pcsx/position_details?position_id=");
    expect(connector).toContain("MICROSOFT_SEARCH_CONTRACT_CHANGED");
    expect(connector).toContain("MICROSOFT_DETAIL_CONTRACT_CHANGED");
  });

  it("types millisecond timestamp fixtures as 64-bit integers", () => {
    expect(connector).toContain('"postedTs":1787000000000i64');
    expect(connector).not.toContain('"postedTs":1787000000000,"');
  });

  it("persists fingerprints and cached detail records", () => {
    expect(migration).toContain("CREATE TABLE connector_detail_cache");
    expect(migration).toContain("summary_fingerprint");
    expect(connector).toContain("stored==&fingerprint");
    expect(connector).toContain("ON CONFLICT(provider,external_id) DO UPDATE");
  });

  it("limits, throttles, and retries hydration requests", () => {
    expect(migration).toContain("hydration_limit");
    expect(connector).toContain("hydration_attempts<tuning.hydration_limit");
    expect(connector).toContain("Duration::from_millis(tuning.request_delay_ms as u64)");
    expect(connector).toContain("let attempts=settings.max_retries+1");
  });
});
