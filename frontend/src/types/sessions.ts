// Session types

export interface EngineSessionSummary {
  session_id: string;
  title: string;
  turn_count: number;
  updated_at: string;
}

export interface EngineSessionsResponse {
  sessions: EngineSessionSummary[];
}

export interface EngineTurn {
  query: string;
  response: string;
  created_at?: string;
  edited?: boolean;
}

export interface EngineSession {
  session_id: string;
  title: string;
  history: {
    turns: EngineTurn[];
  };
}
