import { describe, expect, it } from "vitest";
import { DEFAULT_PAGE_SIZE, loadPageSize, paginate, PAGE_SIZE_OPTIONS, savePageSize } from "./pagination";

const rows = Array.from({ length: 78 }, (_, index) => index + 1);

describe("results pagination", () => {
  it("offers 20, 50 and 100 rows per page, defaulting to 20", () => {
    expect([...PAGE_SIZE_OPTIONS]).toEqual([20, 50, 100]);
    expect(DEFAULT_PAGE_SIZE).toBe(20);
  });

  it("returns the requested page with its row range", () => {
    const second = paginate(rows, 2, 20);
    expect(second.items).toEqual(rows.slice(20, 40));
    expect(second).toMatchObject({ page: 2, pageCount: 4, first: 21, last: 40, total: 78 });
    expect(paginate(rows, 4, 20)).toMatchObject({ first: 61, last: 78, pageCount: 4 });
    expect(paginate(rows, 1, 100)).toMatchObject({ first: 1, last: 78, pageCount: 1 });
  });

  it("keeps the page in range when filters shrink the results", () => {
    expect(paginate(rows, 9, 50)).toMatchObject({ page: 2, first: 51, last: 78 });
    expect(paginate(rows, 0, 20)).toMatchObject({ page: 1, first: 1 });
    expect(paginate([], 3, 20)).toMatchObject({ items: [], page: 1, pageCount: 1, first: 0, last: 0, total: 0 });
  });

  it("remembers the rows-per-page choice and tolerates unavailable storage", () => {
    const values = new Map<string, string>();
    const storage = { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value); } };
    expect(loadPageSize(storage)).toBe(20);
    savePageSize(50, storage);
    expect(loadPageSize(storage)).toBe(50);
    values.set("job-tracker.results-page-size", "37");
    expect(loadPageSize(storage)).toBe(20);
    const broken = { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } };
    expect(loadPageSize(broken)).toBe(20);
    expect(() => savePageSize(100, broken)).not.toThrow();
  });
});
