
• # AEGIS CLI

The `cli` module provides the command-line interface for AEGIS. Its purpose is to give users and
developers a local control interface for installing, configuring, starting, validating, and
interacting with the system from the terminal.

The CLI is not a separate AI system. It is one of the user-facing interfaces of AEGIS and operates on
top of the Rust engine, which remains the central backend and orchestration layer of the project.

## Responsibilities

The CLI is responsible for:

- starting and managing the AEGIS system locally
- providing installation and setup commands
- exposing diagnostic and validation commands
- allowing users to manage local configuration
- supporting direct command-line interaction with the assistant
- reporting status and error information clearly

## Role in the System

The CLI acts as a local control and interaction layer for AEGIS. It allows users to operate the
system without depending solely on the browser-based interface.

The CLI may be used to:

- prepare the environment
- launch the local service
- validate dependencies and configuration
- inspect system status
- interact directly with the assistant through terminal commands

Although it provides user-facing commands, the CLI does not manage inference or retrieval logic
itself. Instead, it passes control to the Rust engine, which handles the actual orchestration of
requests.

## How It Is Used

A typical CLI-based flow is:

1. The user runs a CLI command.
2. The CLI interprets the command and determines the requested operation.
3. If the command is related to startup or runtime interaction, it invokes or communicates with the
   Rust engine.
4. If the command is related to setup or validation, it performs the appropriate local checks and
   preparation steps.
5. If the user sends a prompt through the terminal, the CLI forwards it to the Rust engine.
6. The Rust engine processes the request through the same orchestration flow used by the web
   interface.
7. The result is returned to the CLI and displayed to the user.

This design ensures that the CLI and Web UI share the same backend logic rather than duplicating
application behavior.

## Intended Provided Functions

The CLI is intended to provide a set of local system commands. At a minimum, these include:

- **Install**
    - prepare the local environment and validate required dependencies
- **Setup**
    - perform first-time configuration and initialization tasks
- **Serve**
    - start the local AEGIS backend service
- **Doctor**
    - validate dependencies, local configuration, and runtime health
- **Config**
    - manage local runtime settings
- **Chat**
    - support direct terminal-based interaction with the assistant
- **Status**
    - display useful system and runtime information

These functions define the CLI as both an operational and interactive interface for AEGIS.

## Relationship with Other Components

The CLI operates together with the following components:

- **Rust Engine**
    - receives runtime requests and manages orchestration
- **Installer**
    - prepares the environment and initial setup
- **Web UI**
    - provides an alternative user-facing interface
- **Ollama**
    - local inference backend used indirectly through the Rust engine
- **Python RAG subsystem**
    - used indirectly through the Rust engine when document retrieval is required

## Future Possibilities

Possible future extensions include:

- richer terminal interaction support
- more detailed diagnostics and system inspection
- additional management commands for local tools and data
- stronger integration with installer/update flows
- improved developer-oriented workflows

These are future possibilities and are not all required for the initial MVP.

## Summary

The CLI is the terminal-based control and interaction interface of AEGIS. It allows users to install,
configure, validate, start, and use the system locally while relying on the Rust engine as the
central execution and orchestration backend.
