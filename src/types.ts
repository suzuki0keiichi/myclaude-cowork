export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: string;
}

export interface ActivityItem {
  id: string;
  description: string;
  raw_command: string | null;
  status: "running" | "done" | "error";
  timestamp: string;
}

export interface ApprovalRequest {
  id: string;
  tool_name: string;
  description: string;
  raw_input: string;
  details: string[];
}
