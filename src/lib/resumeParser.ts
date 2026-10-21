export const MAX_RESUME_BYTES = 10 * 1024 * 1024;

export async function parseResumeFile(file: File): Promise<string> {
  if (file.size > MAX_RESUME_BYTES) throw new Error("Resume files must be 10 MB or smaller.");
  const extension = file.name.split(".").pop()?.toLowerCase();
  if (extension === "txt" || extension === "md") return normalizeText(await file.text());
  const data = await file.arrayBuffer();
  if (extension === "docx") {
    const mammoth = (await import("mammoth")).default;
    return normalizeText((await mammoth.extractRawText({ arrayBuffer: data })).value);
  }
  if (extension === "pdf") {
    const pdfjs = await import("pdfjs-dist");
    pdfjs.GlobalWorkerOptions.workerSrc = new URL("pdfjs-dist/build/pdf.worker.min.mjs", import.meta.url).toString();
    const document = await pdfjs.getDocument({ data: new Uint8Array(data) }).promise;
    const pages: string[] = [];
    for (let pageNumber = 1; pageNumber <= document.numPages; pageNumber += 1) {
      const content = await (await document.getPage(pageNumber)).getTextContent();
      pages.push(content.items.map((item) => "str" in item ? item.str : "").join(" "));
    }
    return normalizeText(pages.join("\n"));
  }
  throw new Error("Choose a PDF, DOCX, TXT, or Markdown resume.");
}

export function normalizeText(value: string) { return value.replace(/\u0000/g, "").replace(/[ \t]+/g, " ").replace(/\n{3,}/g, "\n\n").trim(); }
