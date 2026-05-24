import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import {
  ArrowRight,
  Bot,
  CalendarCheck,
  CheckCircle2,
  CircuitBoard,
  Cpu,
  DatabaseZap,
  Download,
  FileArchive,
  FileText,
  GitBranch,
  HardDrive,
  Layers3,
  LockKeyhole,
  Mail,
  MessageSquareText,
  MonitorCog,
  Network,
  ServerCog,
  ShieldCheck,
  TerminalSquare,
  WandSparkles,
} from 'lucide-react';
import heroImage from './assets/hero.png';
import './landing.css';

const DOWNLOAD_PATH = '/downloads/AEGIS-Windows-x64.exe';

const navItems = [
  { label: 'Home', href: '#home' },
  { label: 'Features', href: '#features' },
  { label: 'Docs', href: '#docs' },
  { label: 'Contact Us', href: '#contact' },
];

const featureCards = [
  {
    icon: ShieldCheck,
    title: 'Local-first privacy',
    body: 'Prompts, files, sessions, and memory stay on the machine by design, with Ollama-powered inference and local storage.',
  },
  {
    icon: DatabaseZap,
    title: 'Private semantic memory',
    body: 'AEGIS indexes documents through a Python RAG worker, retrieves relevant context, and keeps citations close to the answer.',
  },
  {
    icon: MonitorCog,
    title: 'Live model control',
    body: 'Switch local models and providers, inspect runtime health, and keep generation visible without leaving the interface.',
  },
  {
    icon: TerminalSquare,
    title: 'CLI and web workflows',
    body: 'Use the focused CLI for terminal work or the React web interface for conversations, files, tools, and settings.',
  },
  {
    icon: CalendarCheck,
    title: 'Local productivity tools',
    body: 'Create Outlook calendar events, import project folders, export PDFs, and keep workflow utilities close to the assistant.',
  },
  {
    icon: CircuitBoard,
    title: 'Rust orchestration engine',
    body: 'The engine routes chat, retrieval, tool calls, sessions, and streaming through a modular Rust backend.',
  },
];

const architectureSteps = [
  {
    icon: MessageSquareText,
    title: 'Request',
    body: 'A user asks AEGIS through the CLI or web UI.',
  },
  {
    icon: ServerCog,
    title: 'Orchestrate',
    body: 'The Rust engine chooses chat, retrieval, tool, or mixed execution paths.',
  },
  {
    icon: Layers3,
    title: 'Ground',
    body: 'Documents, profile notes, sessions, and tool outputs are assembled locally.',
  },
  {
    icon: Bot,
    title: 'Respond',
    body: 'The local model streams a useful answer back to the user.',
  },
];

const docLinks = [
  {
    icon: FileText,
    title: 'Setup Guide',
    body: 'Prepare the local environment, start RAG, and run the engine.',
    path: '/docs/setup.md',
  },
  {
    icon: Network,
    title: 'Engine Flow',
    body: 'Understand how requests move through inference, retrieval, and tools.',
    path: '/docs/engine.md',
  },
  {
    icon: TerminalSquare,
    title: 'CLI Notes',
    body: 'Use the command-line entry point for local assistant workflows.',
    path: '/docs/cli.md',
  },
];

export function LandingPage() {
  return (
    <div className="landing-page">
      <header className="top-nav">
        <a className="brand-mark" href="#home" aria-label="AEGIS home">
          <ShieldCheck size={24} aria-hidden="true" />
          <span>AEGIS</span>
        </a>

        <nav className="nav-links" aria-label="Primary navigation">
          {navItems.map((item) => (
            <a key={item.href} href={item.href}>
              {item.label}
            </a>
          ))}
        </nav>

        <a className="nav-download" href={DOWNLOAD_PATH} download>
          <Download size={17} aria-hidden="true" />
          <span>Download</span>
        </a>
      </header>

      <main>
        <section className="landing-hero" id="home">
          <img className="hero-asset" src={heroImage} alt="" aria-hidden="true" />
          <div className="hero-shield" aria-hidden="true">
            <div className="shield-screen">
              <div className="screen-bar" />
              <div className="screen-row strong" />
              <div className="screen-row" />
              <div className="screen-row short" />
            </div>
            <div className="shield-metric">
              <LockKeyhole size={18} aria-hidden="true" />
              <span>Local context sealed</span>
            </div>
          </div>

          <div className="page-shell hero-copy">
            <p className="eyebrow">Private local AI orchestration platform</p>
            <h1>AEGIS</h1>
            <p className="hero-lede">
              AEGIS brings local inference, private memory, retrieval, tools, and a web interface
              into one system for users who want capable AI without sending their work to a cloud
              assistant.
            </p>

            <div className="hero-actions" aria-label="Landing page actions">
              <a className="primary-action" href={DOWNLOAD_PATH} download>
                <Download size={19} aria-hidden="true" />
                <span>Download Windows Binary</span>
              </a>
              <a className="secondary-action" href="#docs">
                <FileText size={18} aria-hidden="true" />
                <span>Read Docs</span>
              </a>
            </div>

            <div className="hero-tags" aria-label="AEGIS highlights">
              <span>Ollama-ready</span>
              <span>Rust engine</span>
              <span>Python RAG</span>
              <span>React UI</span>
            </div>
          </div>
        </section>

        <section className="proof-strip" aria-label="AEGIS platform summary">
          <div className="proof-item">
            <span className="proof-value">100%</span>
            <span className="proof-label">local-first runtime</span>
          </div>
          <div className="proof-item">
            <span className="proof-value">4</span>
            <span className="proof-label">major interfaces</span>
          </div>
          <div className="proof-item">
            <span className="proof-value">0</span>
            <span className="proof-label">cloud dependency by default</span>
          </div>
          <div className="proof-item">
            <span className="proof-value">Live</span>
            <span className="proof-label">model and system telemetry</span>
          </div>
        </section>

        <section className="section-shell feature-section" id="features">
          <div className="section-heading">
            <p className="eyebrow">Built around user control</p>
            <h2>Everything AEGIS promotes is part of the product architecture.</h2>
            <p>
              The platform combines a local Rust engine, a private RAG subsystem, a developer CLI,
              and a web UI that gives the user direct control over models, documents, sessions, and
              tools.
            </p>
          </div>

          <div className="feature-grid">
            {featureCards.map((feature) => {
              const Icon = feature.icon;
              return (
                <article className="feature-card" key={feature.title}>
                  <Icon size={24} aria-hidden="true" />
                  <h3>{feature.title}</h3>
                  <p>{feature.body}</p>
                </article>
              );
            })}
          </div>
        </section>

        <section className="split-band private-band">
          <div className="section-shell split-content">
            <div>
              <p className="eyebrow">Privacy and memory</p>
              <h2>Work with documents and context while keeping ownership local.</h2>
              <p>
                AEGIS is shaped for private knowledge work: documents are indexed locally, sessions
                are preserved locally, and profile notes can tune responses without outsourcing the
                user's context.
              </p>
            </div>

            <div className="capability-list">
              <div>
                <CheckCircle2 size={19} aria-hidden="true" />
                <span>Local conversations and project snapshots</span>
              </div>
              <div>
                <CheckCircle2 size={19} aria-hidden="true" />
                <span>Semantic retrieval from uploaded documents</span>
              </div>
              <div>
                <CheckCircle2 size={19} aria-hidden="true" />
                <span>Personal notes for stable assistant preferences</span>
              </div>
            </div>
          </div>
        </section>

        <section className="section-shell architecture-section">
          <div className="section-heading compact">
            <p className="eyebrow">System architecture</p>
            <h2>One local loop from prompt to grounded answer.</h2>
          </div>

          <div className="architecture-flow">
            {architectureSteps.map((step, index) => {
              const Icon = step.icon;
              return (
                <article className="flow-step" key={step.title}>
                  <div className="flow-index">{String(index + 1).padStart(2, '0')}</div>
                  <Icon size={23} aria-hidden="true" />
                  <h3>{step.title}</h3>
                  <p>{step.body}</p>
                </article>
              );
            })}
          </div>

          <div className="system-panel" aria-label="AEGIS architecture components">
            <div>
              <Cpu size={22} aria-hidden="true" />
              <span>Rust Orchestration Engine</span>
            </div>
            <div>
              <HardDrive size={22} aria-hidden="true" />
              <span>Local Sessions and Memory</span>
            </div>
            <div>
              <FileArchive size={22} aria-hidden="true" />
              <span>Python RAG Worker</span>
            </div>
            <div>
              <WandSparkles size={22} aria-hidden="true" />
              <span>Tools and Exports</span>
            </div>
          </div>
        </section>

        <section className="download-section" id="download">
          <div className="section-shell download-layout">
            <div>
              <p className="eyebrow">Install AEGIS</p>
              <h2>Download the Windows executable and start from the local setup flow.</h2>
              <p>
                The landing page serves the current Windows x64 binary from the website's public
                downloads directory. Future installer builds can replace the same file without
                changing the page.
              </p>
            </div>

            <div className="download-card">
              <div className="download-card-top">
                <Download size={28} aria-hidden="true" />
                <div>
                  <h3>AEGIS Windows x64</h3>
                  <p>Executable binary</p>
                </div>
              </div>
              <a className="primary-action full" href={DOWNLOAD_PATH} download>
                <span>Download AEGIS-Windows-x64.exe</span>
                <ArrowRight size={18} aria-hidden="true" />
              </a>
              <div className="download-meta">
                <span>Local-first</span>
                <span>Ollama compatible</span>
                <span>Private sessions</span>
              </div>
            </div>
          </div>
        </section>

        <section className="section-shell docs-section" id="docs">
          <div className="section-heading compact">
            <p className="eyebrow">Documentation</p>
            <h2>Read the project notes behind the product.</h2>
            <p>
              The first landing-page pass points visitors to the core docs already kept in the
              repository.
            </p>
          </div>

          <div className="docs-grid">
            {docLinks.map((doc) => {
              const Icon = doc.icon;
              return (
                <a className="doc-card" href={doc.path} key={doc.title}>
                  <Icon size={22} aria-hidden="true" />
                  <h3>{doc.title}</h3>
                  <p>{doc.body}</p>
                  <span>{doc.path}</span>
                </a>
              );
            })}
          </div>
        </section>

        <section className="contact-section" id="contact">
          <div className="section-shell contact-layout">
            <div>
              <p className="eyebrow">Contact Us</p>
              <h2>Have questions about local deployment or the roadmap?</h2>
              <p>
                Reach out to the AEGIS team for setup questions, distribution planning, or feature
                discussions.
              </p>
            </div>

            <div className="contact-actions">
              <a className="secondary-action solid" href="mailto:aegis-team@example.com">
                <Mail size={18} aria-hidden="true" />
                <span>Email the team</span>
              </a>
              <a className="secondary-action solid" href="https://github.com/" rel="noreferrer" target="_blank">
                <GitBranch size={18} aria-hidden="true" />
                <span>Project repository</span>
              </a>
            </div>
          </div>
        </section>
      </main>

      <footer className="site-footer">
        <span>AEGIS</span>
        <span>Private local inference, RAG, tools, and model control.</span>
      </footer>
    </div>
  );
}

createRoot(document.getElementById('landing-root')!).render(
  <StrictMode>
    <LandingPage />
  </StrictMode>,
);
