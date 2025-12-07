import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Conversation } from "../types";
import { MarkdownRenderer } from "./MarkdownRenderer";

interface ChatWindowProps {
  conversation: Conversation | null;
  onNewMessage: () => void;
}

export function ChatWindow({ conversation, onNewMessage }: ChatWindowProps) {
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const chatContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (conversation) {
      setError(null);
      scrollToBottom();
    }
  }, [conversation]);

  useEffect(() => {
    scrollToBottom();
  }, [conversation?.messages]);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  const handleSend = async () => {
    if (!conversation || !input.trim() || loading) return;

    const userMessage = input.trim();
    setInput("");
    setLoading(true);
    setError(null);

    try {
      await invoke<string>("send_message", {
        conversationId: conversation.id,
        content: userMessage,
      });
      onNewMessage();
    } catch (err: any) {
      setError(err || "Une erreur est survenue");
      console.error("Error sending message:", err);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  if (!conversation) {
    return (
      <div className="chat-window-empty">
        <p>Sélectionnez une conversation ou créez-en une nouvelle pour commencer</p>
      </div>
    );
  }

  return (
    <div className="chat-window">
      <div className="chat-header">
        <h3>{conversation.title}</h3>
        <div className="provider-indicator">
          {/* Provider indicator will be added here */}
        </div>
      </div>

      <div className="chat-messages" ref={chatContainerRef}>
        {conversation.messages.map((message) => (
          <div
            key={message.id}
            className={`message message-${message.role}`}
          >
            <div className="message-content">
              {message.role === "assistant" ? (
                <MarkdownRenderer content={message.content} />
              ) : (
                <div className="message-text">{message.content}</div>
              )}
            </div>
            <div className="message-time">
              {new Date(message.timestamp * 1000).toLocaleTimeString()}
            </div>
          </div>
        ))}

        {loading && (
          <div className="message message-assistant">
            <div className="message-content">
              <div className="loading-indicator">...</div>
            </div>
          </div>
        )}

        {error && (
          <div className="message message-error">
            <div className="message-content">
              <div className="error-text">Erreur: {error}</div>
            </div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      <div className="chat-input-container">
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyPress}
          placeholder="Tapez votre message... (Entrée pour envoyer, Shift+Entrée pour nouvelle ligne)"
          className="chat-input"
          rows={3}
          disabled={loading}
        />
        <button
          onClick={handleSend}
          disabled={!input.trim() || loading}
          className="btn-send"
        >
          Envoyer
        </button>
      </div>
    </div>
  );
}

