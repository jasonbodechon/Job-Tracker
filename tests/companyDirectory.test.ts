import { describe, expect, it } from "vitest";
import { directorySourceId, filterCompanyDirectory, findDirectorySource } from "../src/lib/companyDirectory";
import type { CompanyDirectoryEntry, JobSourceConfig } from "../src/types/job";

const entries: CompanyDirectoryEntry[] = [
  { id:"amazon",enabled:true,companyName:"Amazon",countries:["Canada","United States"],atsProvider:"Amazon",connectorClass:"Public site feed",boardIdentifier:"amazon",careersUrl:"https://www.amazon.jobs/en/search",currentJobCount:200,lastVerifiedAt:"2026-08-21T12:00:00Z",verificationStatus:"Verified",identifierOrigin:"Manual" },
  { id:"acme",enabled:true,companyName:"Acme Canada",countries:["Canada"],atsProvider:"SmartRecruiters",connectorClass:"Official API",boardIdentifier:"Acme",careersUrl:"https://jobs.smartrecruiters.com/Acme",currentJobCount:12,verificationStatus:"Pending",identifierOrigin:"Automatic" },
];

describe("company directory filtering", () => {
  it("searches company names without case sensitivity", () => expect(filterCompanyDirectory(entries, "AMAZ", [], [])).toEqual([entries[0]]));
  it("combines country and provider filters", () => expect(filterCompanyDirectory(entries, "", ["Canada"], ["SmartRecruiters"])).toEqual([entries[1]]));
  it("treats multiple countries as OR choices", () => expect(filterCompanyDirectory(entries, "", ["United States"], [])).toEqual([entries[0]]));
  it("creates the stable source identifier used by the API", () => expect(directorySourceId(entries[0])).toBe("directory-amazon"));
  it("recognizes an equivalent source created from another directory record", () => {
    const sources: JobSourceConfig[] = [{ id:"automatic-amazon",kind:"Amazon",company:" amazon ",boardToken:"AMAZON",enabled:true }];
    expect(findDirectorySource(entries[0], sources)?.id).toBe("automatic-amazon");
  });
  it("matches equivalent career-page URLs with an optional trailing slash", () => {
    const workday: CompanyDirectoryEntry = { ...entries[1],id:"workday",companyName:"Example",atsProvider:"Workday",connectorClass:"Public site feed",boardIdentifier:"example",careersUrl:"https://example.wd5.myworkdayjobs.com/Careers/",identifierOrigin:"Manual" };
    const sources: JobSourceConfig[] = [{ id:"saved-workday",kind:"Workday",company:"Example",boardToken:"https://example.wd5.myworkdayjobs.com/Careers",enabled:true }];
    expect(findDirectorySource(workday, sources)?.id).toBe("saved-workday");
  });
  it("returns disabled equivalent sources so directory actions can enable or remove them", () => {
    const sources: JobSourceConfig[] = [{ id:"disabled-amazon",kind:"Amazon",company:"Amazon",boardToken:"amazon",enabled:false }];
    expect(findDirectorySource(entries[0], sources)).toEqual(sources[0]);
  });
});
