// SSE (Server-Sent Events) parsing utilities

export function extractSseEvents(buffer: string) {
  const events: string[] = [];
  let remaining = buffer;
  let boundary = remaining.match(/\r?\n\r?\n/);
  while (boundary?.index !== undefined) {
    events.push(remaining.slice(0, boundary.index));
    remaining = remaining.slice(boundary.index + boundary[0].length);
    boundary = remaining.match(/\r?\n\r?\n/);
  }
  return { events, remaining };
}

export function sseEventData(event: string) {
  return event
    .split(/\r?\n/)
    .filter((line) => line.startsWith('data:'))
    .map((line) => line.replace(/^data: ?/, ''))
    .join('\n');
}

export function splitAssistantStreamSegments(content: string) {
  const segments = content.match(/(\r?\n|[^\S\r\n]+|[^\s]+)/g);
  return segments && segments.length > 0 ? segments : [content];
}
