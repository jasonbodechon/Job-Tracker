/** Rows-per-page choices for the job results table. */
export const PAGE_SIZE_OPTIONS = [20, 50, 100] as const;
export type PageSize = (typeof PAGE_SIZE_OPTIONS)[number];
export const DEFAULT_PAGE_SIZE: PageSize = 20;

const PAGE_SIZE_STORAGE_KEY = "job-tracker.results-page-size";

export interface Page<T> {
  items: T[];
  /** The current page, kept within 1..pageCount. */
  page: number;
  pageCount: number;
  /** 1-based position of the first and last row shown; both 0 when there are no rows. */
  first: number;
  last: number;
  total: number;
}

export function isPageSize(value: unknown): value is PageSize {
  return PAGE_SIZE_OPTIONS.includes(Number(value) as PageSize);
}

/** Returns one page of rows. A page past the end (for example after filtering) shows the last page. */
export function paginate<T>(items: readonly T[], page: number, pageSize: number): Page<T> {
  const total = items.length;
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  const current = Math.min(Math.max(1, Math.floor(page) || 1), pageCount);
  const start = (current - 1) * pageSize;
  const pageItems = items.slice(start, start + pageSize);
  return { items: pageItems, page: current, pageCount, first: total ? start + 1 : 0, last: start + pageItems.length, total };
}

/** The viewer's last rows-per-page choice. Browser storage can be unavailable, so this never throws. */
export function loadPageSize(storage: Pick<Storage, "getItem"> | undefined = globalThis.localStorage): PageSize {
  try {
    const stored = Number(storage?.getItem(PAGE_SIZE_STORAGE_KEY));
    return isPageSize(stored) ? stored : DEFAULT_PAGE_SIZE;
  } catch {
    return DEFAULT_PAGE_SIZE;
  }
}

export function savePageSize(pageSize: PageSize, storage: Pick<Storage, "setItem"> | undefined = globalThis.localStorage): void {
  try { storage?.setItem(PAGE_SIZE_STORAGE_KEY, String(pageSize)); } catch { /* the choice just is not remembered */ }
}
