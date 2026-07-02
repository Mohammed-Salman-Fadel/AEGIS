# AEGIS Command Line Interface

## Usage

To run the AEGIS CLI, you simply

## Commands

Table of Contents for all types of AEGIS commands:

- [Usage Instruction](#usage)
- [Chat Commands](#chat-chat)
- [Session Commands](#sessions-session)
- [Configuration Commands](#configuration)
  - [Provider](#provider)
  - [Model](#model)
  - [Personalization](#personalization-options)
- [Coding Capabilities](#coding-capabilities)

### Chat `chat`

- `chat "[prompt]"` - Generates a one-time call to the LLM.

### Sessions `session`

- `session new` - create new session.
- `session delete [session_id]` - delete session given session ID.
- `session list` - list all existing sessions.
- `session continue [session_id]` - continues from a previous session.

### Configuration

#### `Provider`

- `provider list` - list all inference providers.
- `provider switch`

#### `Model`

- `model switch [model_name]` - switches between available models.
- `model list` - list out all available models.

#### Personalization Options

The model saves user preferences and builds an internal user profile in a structured manner, therefore you can manually ask to add, delete, or change a preference that was set.

- `save [your_info]` - adds whatever you write into the info field as part of user profile.

## Coding Capabilities

It is important to awknowledge the hardware limitations of these models and their
limited capacity for reasoning. That said, we do support functionality for coding
that aims to maximize each models capacity regardless of their size.

### `Projects`

Projects are simply coding project directories, users can import them and the contents
of these projects will automatically be compacted into the model's context window.
