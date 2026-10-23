import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/features.css", import.meta.url), "utf8");

describe("Best job matches paging", () => {
  it("renders only the current page of results", () => {
    expect(app).toContain("paginate(visibleJobs, resultsPage, resultsPageSize)");
    expect(app).toContain("<tbody>{resultsPageData.items.map");
    expect(app).not.toContain("<tbody>{visibleJobs.map");
  });

  it("offers a rows-per-page selector and previous/next controls", () => {
    expect(app).toContain("PAGE_SIZE_OPTIONS.map((size) => <option key={size} value={size}>{size}</option>)");
    expect(app).toContain('aria-label="Previous page"');
    expect(app).toContain('aria-label="Next page"');
    expect(app).toContain("Showing ${resultsPageData.first}–${resultsPageData.last} of ${resultsPageData.total}");
    expect(css).toContain(".pagination {");
  });

  it("returns to the first page when filters, sorting or page size change", () => {
    expect(app).toContain("useEffect(() => { setResultsPage(1); }, [query, sourceFilters, statusFilters, profileFilters, postedFrom, postedTo, locationStatusFilters, sortColumn, sortDirection, resultsPageSize]);");
  });
});
