export interface Message {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
}

export interface Conversation {
  id: string;
  title: string;
  messages: Message[];
  created_at: number;
  updated_at: number;
}

export interface ApiProvider {
  provider: "codestral.mistral.ai" | "api.mistral.ai";
  api_key?: string;
  phone_number?: string;
}

export interface Settings {
  api_provider: ApiProvider;
  selected_conversation_id?: string;
}


