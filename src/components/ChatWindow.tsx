import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Conversation } from "../types";
import { MarkdownRenderer } from "./MarkdownRenderer";

interface ChatWindowProps {
  conversation: Conversation | null;
  onNewMessage: () => void;
}

const LARGE_PASTE_THRESHOLD = 500; // Characters threshold for large paste

export function ChatWindow({ conversation, onNewMessage }: ChatWindowProps) {
  const [input, setInput] = useState("");
  const [actualContent, setActualContent] = useState(""); // Full content for sending
  const [isLargePaste, setIsLargePaste] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const chatContainerRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (conversation) {
      setError(null);
      scrollToBottom();
    }
  }, [conversation]);

  useEffect(() => {
    scrollToBottom();
  }, [conversation?.messages]);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current && !isLargePaste) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = Math.min(textareaRef.current.scrollHeight, 200) + "px";
    }
  }, [input, isLargePaste]);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value;
    setActualContent(value);

    // Check if it's a large paste (content significantly larger than before)
    if (value.length > LARGE_PASTE_THRESHOLD && value.length - input.length > 100) {
      setIsLargePaste(true);
      setInput(`📋 ${value.length} caractères collés`);
    } else if (!isLargePaste) {
      setInput(value);
    }
  };

  const handleClearLargePaste = () => {
    setIsLargePaste(false);
    setInput("");
    setActualContent("");
  };

  const handleSend = async () => {
    const contentToSend = isLargePaste ? actualContent : input;
    if (!conversation || !contentToSend.trim() || loading) return;

    const userMessage = contentToSend.trim();
    setInput("");
    setActualContent("");
    setIsLargePaste(false);
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
        {isLargePaste ? (
          <div className="large-paste-indicator">
            <span className="paste-info">{input}</span>
            <button onClick={handleClearLargePaste} className="btn-clear-paste">✕</button>
          </div>
        ) : (
          <textarea
            ref={textareaRef}
            value={input}
            onChange={handleInputChange}
            onKeyDown={handleKeyPress}
            placeholder="Tapez votre message... (Entrée pour envoyer)"
            className="chat-input"
            rows={1}
            disabled={loading}
          />
        )}
        <button
          onClick={handleSend}
          disabled={!(isLargePaste ? actualContent.trim() : input.trim()) || loading}
          className="btn-send"
        >
          Envoyer
        </button>
      </div>
    </div>
  );
}
