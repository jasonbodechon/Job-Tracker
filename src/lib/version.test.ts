import { describe, expect, it } from "vitest";
import packageMetadata from "../../package.json";
import { APP_NAME, APP_VERSION, APP_VERSION_LABEL } from "./version";

describe("application version", () => {
  it("uses package.json as the single frontend version source", () => {
    expect(APP_VERSION).toBe(packageMetadata.version);
    expect(APP_VERSION_LABEL).toBe(`v${packageMetadata.version}`);
    expect(APP_NAME).toBe("Job-Tracker");
  });
});
