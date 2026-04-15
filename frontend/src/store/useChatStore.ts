import { create } from 'zustand';

// FR-14: Citation support for document-based queries
// FR-01: Structure for token-by-token streaming
interface Message {
  role: 'user' | 'assistant';
  content: string;
  sources?: string[]; // To store file names like "Project_Proposal.pdf"
}

// FR-18 & FR-29: Possible phases for the Orchestration Trace
type TracePhase = 'Idle' | 'Routing' | 'RAG' | 'Inference' | 'Complete';

interface SystemResources {
  cpu: number;
  ram: number;
}

interface ChatState {
  messages: Message[];
  isStreaming: boolean;
  currentTrace: TracePhase;
  resources: SystemResources;
  // Actions
  addMessage: (content: string, role: 'user' | 'assistant') => void;
  updateStreamingMessage: (chunk: string) => void;
  setTrace: (phase: TracePhase) => void;
  updateResources: (cpu: number, ram: number) => void;
  addSourcesToLastMessage: (sources: string[]) => void;
  resetChat: () => void; // FR-19: Session management
}

export const useChatStore = create<ChatState>((set) => ({
  messages: [],
  isStreaming: false,
  currentTrace: 'Idle',
  resources: { cpu: 0, ram: 0 },

  // FR-19: Adds a new message to the session context
  addMessage: (content, role) =>
    set((state) => ({
      messages: [...state.messages, { role, content }],
      isStreaming: role === 'assistant',
    })),

  // FR-01: Progressively appends tokens to the last assistant message
  updateStreamingMessage: (chunk) =>
    set((state) => {
      const newMessages = [...state.messages];
      const lastMessage = newMessages[newMessages.length - 1];
      if (lastMessage && lastMessage.role === 'assistant') {
        lastMessage.content += chunk;
      }
      return { messages: newMessages, isStreaming: true };
    }),

  // FR-14: Attaches source references to the generated response
  addSourcesToLastMessage: (sources) =>
    set((state) => {
      const newMessages = [...state.messages];
      const lastMessage = newMessages[newMessages.length - 1];
      if (lastMessage && lastMessage.role === 'assistant') {
        lastMessage.sources = sources;
      }
      return { messages: newMessages };
    }),

  // FR-18 & FR-29: Updates the visual trace of the Rust Engine
  setTrace: (phase) => set({ 
    currentTrace: phase,
    isStreaming: phase === 'Inference' 
  }),

  // FR-05: Updates CPU/RAM status for real-time monitoring
  updateResources: (cpu, ram) => set({ 
    resources: { cpu, ram } 
  }),

  // FR-19: Clears session context
  resetChat: () => set({ 
    messages: [], 
    currentTrace: 'Idle', 
    isStreaming: false 
  }),
}));