# AEGIS Installer

The `installer` module is responsible for preparing the local environment required to run AEGIS. Its
purpose is to simplify first-time setup and reduce the technical overhead of configuring a local-only
AI system.

The installer is intended to support the user before normal system operation begins. It ensures that
the required components, runtime dependencies, and local configuration are available so that the CLI
and the Rust engine can operate correctly.

## Responsibilities

The installer is responsible for:

- preparing the local environment for AEGIS
- obtaining or validating the correct release/build for the user’s platform
- checking required dependencies and local runtime prerequisites
- verifying the availability of Ollama and at least one compatible local model
- validating any required Python/RAG-related runtime dependencies if applicable
- creating the required local directories, files, and configuration paths
- performing first-time setup and initialization tasks
- guiding the user when dependencies are missing or incorrectly configured

## Role in the System

The installer is not part of the runtime AI workflow. It does not process prompts, communicate with
the model directly during normal use, or control request orchestration.

Its role is to prepare the system so that the other major components can function correctly,
especially:

- the Rust engine
- the CLI
- the Python RAG subsystem
- local runtime dependencies such as Ollama

## Setup Flow

The installer is expected to support a setup flow similar to the following:

1. Obtain or validate the correct AEGIS build for the host platform.
2. Check whether required local dependencies are available.
3. Validate the Ollama installation and model availability.
4. Validate Python/RAG-related runtime dependencies if needed.
5. Create required directories, paths, and local configuration files.
6. Prepare the system for first-time startup.
7. Report setup status clearly to the user.

## Dependency Validation

The installer may check for items such as:

- operating system compatibility
- local architecture compatibility
- Ollama installation
- presence of at least one local model
- Python runtime availability
- required package dependencies
- writable local paths for indexes, logs, or configuration
- connectivity between the expected local components

## User Experience Goals

The installer should aim to:

- reduce manual setup effort
- provide clear feedback when something is missing
- keep the system easy to initialize on a fresh machine
- make local-only deployment practical for users with moderate technical experience

## Future Possibilities

Possible future extensions for the installer include:

- automated dependency installation where appropriate
- release-aware update handling
- guided setup flows for different operating systems
- validation of optional MCP integrations
- more advanced first-run configuration support

These are possible additions and are not all required for the initial version.

## Summary

The installer is the setup and environment preparation component of AEGIS. Its value lies in making
the local-only system easier to initialize, validate, and operate by ensuring that all required
dependencies and runtime prerequisites are in place before the main application is used.
