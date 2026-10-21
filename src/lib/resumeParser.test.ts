import { beforeEach, describe, expect, it, vi } from "vitest";

const { extractRawText, getDocument } = vi.hoisted(() => ({ extractRawText: vi.fn(), getDocument: vi.fn() }));
vi.mock("mammoth", () => ({ default: { extractRawText } }));
vi.mock("pdfjs-dist", () => ({ GlobalWorkerOptions: { workerSrc: "" }, getDocument }));

import { MAX_RESUME_BYTES, normalizeText, parseResumeFile } from "./resumeParser";

describe("resume parsing", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("normalizes imported plain text", async () => {
    const file = new File(["Senior   Engineer\n\n\nTypeScript"], "resume.txt", { type: "text/plain" });
    await expect(parseResumeFile(file)).resolves.toBe("Senior Engineer\n\nTypeScript");
  });

  it("extracts DOCX text locally", async () => {
    extractRawText.mockResolvedValue({ value: "Director   of Engineering" });
    await expect(parseResumeFile(new File(["docx"], "resume.docx"))).resolves.toBe("Director of Engineering");
  });

  it("extracts and joins PDF pages locally", async () => {
    getDocument.mockReturnValue({ promise: Promise.resolve({ numPages: 2, getPage: async (page: number) => ({ getTextContent: async () => ({ items: [{ str: page === 1 ? "Page one" : "Page two" }] }) }) }) });
    await expect(parseResumeFile(new File(["pdf"], "resume.pdf"))).resolves.toBe("Page one\nPage two");
  });

  it("rejects unsupported and oversized files", async () => {
    await expect(parseResumeFile(new File(["data"], "resume.rtf"))).rejects.toThrow("PDF, DOCX, TXT, or Markdown");
    const oversized = { name: "resume.pdf", size: MAX_RESUME_BYTES + 1 } as File;
    await expect(parseResumeFile(oversized)).rejects.toThrow("10 MB or smaller");
  });

  it("removes null characters and excess whitespace", () => { expect(normalizeText("A\u0000  B")).toBe("A B"); });
});
