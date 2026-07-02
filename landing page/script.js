const navbar = document.getElementById('navbar');
const progressBar = document.querySelector('.scroll-progress');
const reveals = document.querySelectorAll('.reveal');
const tiltCard = document.querySelector('[data-tilt]');
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
const docsNav = document.getElementById('docsNav');
const docsTitle = document.getElementById('docsTitle');
const docsPath = document.getElementById('docsPath');
const docsContent = document.getElementById('docsContent');

const DOCS = {
  setup: {
    title: 'Setup Guide',
    path: '../docs/setup.md',
  },
  cli: {
    title: 'CLI Reference',
    path: '../docs/cli.md',
  },
  engine: {
    title: 'Engine Flow',
    path: '../docs/engine.md',
  },
  project: {
    title: 'Project Layout',
    path: '../docs/project.md',
  },
};

function escapeHtml(value) {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function renderInlineMarkdown(value) {
  return escapeHtml(value)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
}

function renderMarkdownTable(lines, startIndex) {
  const rows = [];
  let index = startIndex;

  while (index < lines.length && /^\|.+\|$/.test(lines[index].trim())) {
    rows.push(lines[index].trim());
    index += 1;
  }

  if (rows.length < 2) {
    return null;
  }

  const cells = rows.map((row) =>
    row
      .slice(1, -1)
      .split('|')
      .map((cell) => cell.trim()),
  );
  const hasDivider = cells[1]?.every((cell) => /^:?-{3,}:?$/.test(cell));

  if (!hasDivider) {
    return null;
  }

  const header = cells[0];
  const bodyRows = cells.slice(2);
  const headHtml = header.map((cell) => `<th>${renderInlineMarkdown(cell)}</th>`).join('');
  const bodyHtml = bodyRows
    .map((row) => `<tr>${row.map((cell) => `<td>${renderInlineMarkdown(cell)}</td>`).join('')}</tr>`)
    .join('');

  return {
    html: `<div class="docs-table-wrap"><table><thead><tr>${headHtml}</tr></thead><tbody>${bodyHtml}</tbody></table></div>`,
    nextIndex: index,
  };
}

function markdownToHtml(markdown) {
  const lines = markdown.replace(/\r\n/g, '\n').split('\n');
  const html = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];
    const trimmed = line.trim();

    if (!trimmed) {
      index += 1;
      continue;
    }

    const fenceMatch = trimmed.match(/^```([A-Za-z0-9_+.#-]*)/);
    if (fenceMatch) {
      const language = fenceMatch[1] || 'text';
      const codeLines = [];
      index += 1;

      while (index < lines.length && !lines[index].trim().startsWith('```')) {
        codeLines.push(lines[index]);
        index += 1;
      }

      html.push(
        `<figure class="docs-code"><figcaption>${escapeHtml(language)}</figcaption><pre><code>${escapeHtml(codeLines.join('\n'))}</code></pre></figure>`,
      );
      index += 1;
      continue;
    }

    const table = renderMarkdownTable(lines, index);
    if (table) {
      html.push(table.html);
      index = table.nextIndex;
      continue;
    }

    const heading = trimmed.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      const level = Math.min(heading[1].length + 1, 5);
      html.push(`<h${level}>${renderInlineMarkdown(heading[2])}</h${level}>`);
      index += 1;
      continue;
    }

    if (/^---+$/.test(trimmed)) {
      html.push('<hr />');
      index += 1;
      continue;
    }

    if (trimmed.startsWith('>')) {
      const quoteLines = [];
      let alertType = null;
      while (index < lines.length && lines[index].trim().startsWith('>')) {
        const content = lines[index].trim().replace(/^>\s?/, '');
        const alertMatch = content.match(/^\[!(\w+)\]/);
        if (alertMatch) {
          alertType = alertMatch[1];
          quoteLines.push(content.replace(/^\[\!\w+\]\s*/, ''));
        } else {
          quoteLines.push(content);
        }
        index += 1;
      }
      const cls = alertType ? ` class="alert alert-${alertType.toLowerCase()}"` : '';
      html.push(`<blockquote${cls}>${quoteLines.map(renderInlineMarkdown).join('<br />')}</blockquote>`);
      continue;
    }

    const orderedMatch = trimmed.match(/^\d+\.\s+(.+)$/);
    if (orderedMatch) {
      const items = [];
      while (index < lines.length) {
        const item = lines[index].trim().match(/^\d+\.\s+(.+)$/);
        if (!item) {
          break;
        }
        items.push(`<li>${renderInlineMarkdown(item[1])}</li>`);
        index += 1;
      }
      html.push(`<ol>${items.join('')}</ol>`);
      continue;
    }

    const unorderedMatch = trimmed.match(/^[-*]\s+(.+)$/);
    if (unorderedMatch) {
      const items = [];
      while (index < lines.length) {
        const item = lines[index].trim().match(/^[-*]\s+(.+)$/);
        if (!item) {
          break;
        }
        items.push(`<li>${renderInlineMarkdown(item[1])}</li>`);
        index += 1;
      }
      html.push(`<ul>${items.join('')}</ul>`);
      continue;
    }

    const paragraph = [];
    while (index < lines.length) {
      const current = lines[index].trim();
      if (
        !current ||
        /^#{1,4}\s+/.test(current) ||
        /^[-*]\s+/.test(current) ||
        /^\d+\.\s+/.test(current) ||
        /^```/.test(current) ||
        /^\|.+\|$/.test(current) ||
        current.startsWith('>')
      ) {
        break;
      }
      paragraph.push(current);
      index += 1;
    }
    html.push(`<p>${renderInlineMarkdown(paragraph.join(' '))}</p>`);
  }

  return html.join('');
}

async function loadDocumentation(docKey) {
  const doc = DOCS[docKey] ?? DOCS.setup;

  if (docsTitle) {
    docsTitle.textContent = doc.title;
  }
  if (docsPath) {
    docsPath.textContent = doc.path;
  }
  if (docsContent) {
    docsContent.innerHTML = '<p>Loading Markdown from the docs folder...</p>';
  }

  document
    .querySelectorAll('.docs-tab, .docs-rail-item, .docs-nav-item, .docs-overview-card')
    .forEach((item) => {
      item.classList.toggle('active', item.dataset.doc === docKey);
    });

  try {
    const response = await fetch(doc.path);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }

    const markdown = await response.text();
    if (docsContent) {
      docsContent.innerHTML = markdownToHtml(markdown);
    }
  } catch (error) {
    if (docsContent) {
      docsContent.innerHTML = `
        <div class="docs-error">
          <strong>Could not load ${escapeHtml(doc.path)}</strong>
          <p>The docs viewer reads Markdown files from the repository docs folder. If you opened this page directly through <code>file://</code>, serve the repository with a small static server so the browser can fetch local Markdown files.</p>
        </div>
      `;
    }
  }
}

function updateChrome() {
  const scrollable = document.documentElement.scrollHeight - window.innerHeight;
  const progress = scrollable > 0 ? window.scrollY / scrollable : 0;

  navbar?.classList.toggle('scrolled', window.scrollY > 36);
  if (progressBar) {
    progressBar.style.transform = `scaleX(${Math.min(Math.max(progress, 0), 1)})`;
  }
}

const revealObserver = new IntersectionObserver(
  (entries) => {
    entries.forEach((entry) => {
      if (!entry.isIntersecting) {
        return;
      }

      entry.target.classList.add('visible');
      revealObserver.unobserve(entry.target);
    });
  },
  {
    threshold: 0.12,
    rootMargin: '0px 0px -44px 0px',
  },
);

reveals.forEach((element, index) => {
  element.style.transitionDelay = `${Math.min(index % 6, 4) * 55}ms`;
  revealObserver.observe(element);
});

document.querySelectorAll('.features-grid, .usecases-grid').forEach((grid) => {
  grid.querySelectorAll('.feature-card, .usecase-card').forEach((card, index) => {
    card.style.transitionDelay = `${index * 55}ms`;
  });
});

docsNav?.addEventListener('click', (event) => {
  const target =
    event.target instanceof Element ? event.target.closest('[data-doc]') : null;
  if (!(target instanceof HTMLButtonElement)) {
    return;
  }

  loadDocumentation(target.dataset.doc);
});

document.querySelectorAll('.docs-overview-card[data-doc]').forEach((card) => {
  card.addEventListener('click', () => {
    loadDocumentation(card.dataset.doc);
  });
});

if (docsContent) {
  loadDocumentation('setup');
}

window.addEventListener('scroll', updateChrome, { passive: true });
window.addEventListener('resize', updateChrome);
updateChrome();

window.addEventListener('pointermove', (event) => {
  if (tiltCard && window.innerWidth < 1080) {
    tiltCard.style.transform = '';
  }

  if (!tiltCard || reducedMotion.matches || window.innerWidth < 1080) {
    return;
  }

  const rect = tiltCard.getBoundingClientRect();
  const x = (event.clientX - rect.left) / rect.width - 0.5;
  const y = (event.clientY - rect.top) / rect.height - 0.5;

  if (Math.abs(x) > 1 || Math.abs(y) > 1) {
    tiltCard.style.transform = 'translateY(-50%)';
    return;
  }

  tiltCard.style.transform = `translateY(-50%) rotateX(${-y * 7}deg) rotateY(${x * 7}deg)`;
});

window.addEventListener('pointerleave', () => {
  if (tiltCard) {
    tiltCard.style.transform = '';
  }
});

const docsApp = document.querySelector('.docs-app');
const handle = document.querySelector('.sidebar-resize-handle');
const STORAGE_KEY = 'aegis-docs-sidebar-width';
const MIN_WIDTH = 180;
const MAX_WIDTH = 500;

function setSidebarWidth(w) {
  w = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, w));
  docsApp.style.gridTemplateColumns = w + 'px 64px 1fr';
  return w;
}

if (docsApp && handle) {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved) {
    const w = parseInt(saved, 10);
    if (w >= MIN_WIDTH && w <= MAX_WIDTH) {
      setSidebarWidth(w);
    }
  }

  let dragging = false;

  function startDrag(e) {
    dragging = true;
    handle.classList.add('dragging');
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
    e.preventDefault();
  }

  function moveDrag(e) {
    if (!dragging) return;
    const rect = docsApp.getBoundingClientRect();
    const x = e.touches ? e.touches[0].clientX : e.clientX;
    setSidebarWidth(x - rect.left);
  }

  function endDrag() {
    if (!dragging) return;
    dragging = false;
    handle.classList.remove('dragging');
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    const match = docsApp.style.gridTemplateColumns.match(/^(\d+)px/);
    if (match) {
      const w = parseInt(match[1], 10);
      if (w >= MIN_WIDTH && w <= MAX_WIDTH) {
        localStorage.setItem(STORAGE_KEY, w);
      }
    }
  }

  handle.addEventListener('mousedown', startDrag);
  document.addEventListener('mousemove', moveDrag);
  document.addEventListener('mouseup', endDrag);
  handle.addEventListener('touchstart', startDrag, { passive: false });
  document.addEventListener('touchmove', moveDrag, { passive: false });
  document.addEventListener('touchend', endDrag);
}
