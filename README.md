# AEGIS

AEGIS is a local-only AI assistant platform designed to run entirely on the user’s machine without relying on cloud-based inference or external online services. The project combines a
orchestration engine, a web interface, tool integrations via MCP, a command-line interface, a RAG subsystem, and a
local inference backend provide private and modular AI interaction.

Table of Contents:

- [Overview](#overview)
- [Prerequisites](#installation)
- [Installation Guide](#installation)
- [Getting Started](#getting-started)
- [Demo](#demo)

### Project Repository

```
  aegis/
  ├── engine-rust/
  ├── rag-python/
  ├── web-ui/
  ├── cli/
  ├── installer/
  ├── docs/
  ├── scripts/
  └── data/
```

## Overview

<!-- As local models progress and provide better  -->

The system is designed around the following principles:

- **Local-only execution**
- **Privacy-preserving interaction**
- **Clear orchestration of inference, retrieval, and tools**
- **Support for both CLI and web-based usage**

## Prerequisites

Download an Inference provider - Ollama and LMStudio are currently the only providers supported by AEGIS.

- [Ollama's official download page](https://ollama.com/download)
- [LMStudio download page](https://lmstudio.ai/download)

## Installation

Installation steps:

### 1. Download binary file from web page:

```
    cd \AEGIS\AEGIS\landing page
    npm run dev
```

- Run the downloaded `AEGIS-Windows-x64.exe` executable file. All the python dependencies in the requirements.txt file should be downloaded automatically.

### 2. Open a new terminal and run the following command:

```
    aegis install --yes
```

## Getting Started

To launch everything from the terminal, run the following:

```
    aegis open
```

This should start the following:

- RAG service (Python FastAPI on 127.0.0.1:8000).
- Engine (Rust HTTP server on 127.0.0.1:8080).
- Opens the browser to http://localhost:8080.

Once everything is ran, you simpy go to the localhost link and you can begin chatting with AEGIS!

### Getting Started with the CLI

Calling the AEGIS CLi is very easy, you can simply call `aegis` in powershell or your default command line shell and AEGIS CLI should start running.

```
aegis
```

**NOTE**: When using the aegis cli, you should still call the `aegis open` command to run the background services needed for the cli to function.

## Demo
