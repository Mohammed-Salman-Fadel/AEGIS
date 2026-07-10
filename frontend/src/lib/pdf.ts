// PDF generation and export utilities
import type { Message, IndexedDocument } from '../types/index.js';

function escapePdfText(text: string) {
  return text
    .replace(/\\/g, '\\\\')
    .replace(/\(/g, '\\(')
    .replace(/\)/g, '\\)');
}

function wrapPdfLine(line: string, maxLength: number) {
  const words = line.replace(/\r/g, '').split(/\s+/);
  const wrapped: string[] = [];
  let current = '';
  for (const word of words) {
    if (!word) continue;
    if (word.length > maxLength) {
      if (current) { wrapped.push(current); current = ''; }
      for (let i = 0; i < word.length; i += maxLength) wrapped.push(word.slice(i, i + maxLength));
      continue;
    }
    const next = current ? `${current} ${word}` : word;
    if (next.length > maxLength) { wrapped.push(current); current = word; }
    else { current = next; }
  }
  if (current) wrapped.push(current);
  return wrapped.length > 0 ? wrapped : [''];
}

function formatExportTimestamp(timestamp?: string) {
  if (!timestamp) return 'time not recorded';
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return timestamp;
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric', month: 'short', day: '2-digit',
    hour: '2-digit', minute: '2-digit', second: '2-digit',
  }).format(date);
}

function speakerLabel(role: 'user' | 'assistant') {
  return role === 'user' ? 'User' : 'AEGIS';
}

export function createConversationPdf(options: {
  title: string;
  sessionId?: string | null;
  messages: Message[];
  indexedDocuments: IndexedDocument[];
}) {
  const pageWidth = 595;
  const pageHeight = 842;
  const margin = 48;
  const lineHeight = 15;
  const maxLinesPerPage = Math.floor((pageHeight - margin * 2) / lineHeight);
  const maxCharsPerLine = 88;
  const pages: string[][] = [[]];
  const exportedAt = new Date().toISOString();

  function addLine(line: string) {
    const page = pages[pages.length - 1];
    if (page.length >= maxLinesPerPage) pages.push([]);
    pages[pages.length - 1].push(line);
  }

  addLine('AEGIS Chat Transcript');
  addLine('');
  addLine(`Conversation: ${options.title}`);
  if (options.sessionId) addLine(`Session ID: ${options.sessionId}`);
  addLine(`Exported: ${formatExportTimestamp(exportedAt)}`);
  addLine(`Messages: ${options.messages.length}`);
  if (options.indexedDocuments.length > 0) {
    addLine('');
    addLine('Documents Added');
    options.indexedDocuments.forEach((doc) => {
      const chunkLabel = doc.chunks_added === 1 ? 'chunk' : 'chunks';
      addLine(`- User added document: ${doc.file_name} (${doc.chunks_added} ${chunkLabel})`);
    });
  }
  addLine('Format: speaker label, timestamp, message body');
  addLine('------------------------------------------------------------');
  addLine('');

  options.messages.forEach((msg) => {
    const label = `${speakerLabel(msg.role)} | ${formatExportTimestamp(msg.timestamp)}${msg.edited ? ' | edited' : ''}`;
    addLine(label);
    msg.content.split('\n').forEach((line: string) => {
      wrapPdfLine(line, maxCharsPerLine).forEach((wrappedLine) => addLine(`  ${wrappedLine}`));
    });
    addLine('');
  });

  const objects: string[] = [''];
  const fontObjectNumber = 3 + pages.length * 2;
  const kids: string[] = [];
  objects[1] = '<< /Type /Catalog /Pages 2 0 R >>';

  pages.forEach((pageLines, pageIndex) => {
    const pageObjectNumber = 3 + pageIndex * 2;
    const contentObjectNumber = pageObjectNumber + 1;
    kids.push(`${pageObjectNumber} 0 R`);
    const stream = pageLines
      .map((line, lineIndex) => {
        const y = pageHeight - margin - lineIndex * lineHeight;
        return `BT /F1 10 Tf 1 0 0 1 ${margin} ${y} Tm (${escapePdfText(line)}) Tj ET`;
      })
      .join('\n');
    objects[pageObjectNumber] = `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${pageWidth} ${pageHeight}] /Resources << /Font << /F1 ${fontObjectNumber} 0 R >> >> /Contents ${contentObjectNumber} 0 R >>`;
    objects[contentObjectNumber] = `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
  });

  objects[2] = `<< /Type /Pages /Kids [${kids.join(' ')}] /Count ${pages.length} >>`;
  objects[fontObjectNumber] = '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>';

  let pdf = '%PDF-1.4\n';
  const offsets = [0];
  for (let i = 1; i < objects.length; i += 1) {
    offsets[i] = pdf.length;
    pdf += `${i} 0 obj\n${objects[i]}\nendobj\n`;
  }

  const xrefOffset = pdf.length;
  pdf += `xref\n0 ${objects.length}\n0000000000 65535 f \n`;
  for (let i = 1; i < objects.length; i += 1) {
    pdf += `${offsets[i].toString().padStart(10, '0')} 00000 n \n`;
  }
  pdf += `trailer\n<< /Size ${objects.length} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF`;

  return new Blob([pdf], { type: 'application/pdf' });
}

export function safeExportFileName(title: string) {
  return title.trim().replace(/[\\/:*?"<>|]+/g, '-').replace(/\s+/g, '-').toLowerCase();
}

export function downloadConversationPdf(options: {
  title: string;
  sessionId?: string | null;
  messages: Message[];
  indexedDocuments: IndexedDocument[];
}) {
  const blob = createConversationPdf(options);
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  const sessionFileName = options.sessionId?.trim();
  const safeTitle = safeExportFileName(options.title);
  anchor.href = url;
  anchor.download = `${sessionFileName || safeTitle || 'aegis-chat'}.pdf`;
  anchor.click();
  URL.revokeObjectURL(url);
}
