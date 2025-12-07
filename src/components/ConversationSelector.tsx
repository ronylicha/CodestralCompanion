import { useState } from "react";
import { Conversation } from "../types";

interface ConversationSelectorProps {
  conversations: Conversation[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string) => void;
  onRename: (id: string, newTitle: string) => void;
}

export function ConversationSelector({
  conversations,
  selectedId,
  onSelect,
  onCreate,
  onDelete,
  onRename,
}: ConversationSelectorProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");

  const selectedConversation = conversations.find((c) => c.id === selectedId);

  const handleRename = (id: string, currentTitle: string) => {
    setEditingId(id);
    setEditTitle(currentTitle);
  };

  const handleSaveRename = () => {
    if (editingId) {
      onRename(editingId, editTitle);
      setEditingId(null);
      setEditTitle("");
    }
  };

  const handleDelete = (id: string) => {
    if (confirm("Êtes-vous sûr de vouloir supprimer cette conversation ?")) {
      onDelete(id);
    }
  };

  return (
    <div className="conversation-selector">
      <div className="selector-header">
        <select
          value={selectedId || ""}
          onChange={(e) => onSelect(e.target.value)}
          className="conversation-dropdown"
        >
          <option value="">Sélectionner une conversation...</option>
          {conversations.map((conv) => (
            <option key={conv.id} value={conv.id}>
              {conv.title}
            </option>
          ))}
        </select>
        <button onClick={onCreate} className="btn-new-conversation">
          + Nouvelle
        </button>
      </div>

      {selectedConversation && (
        <div className="conversation-actions">
          <button
            onClick={() => handleRename(selectedId!, selectedConversation.title)}
            className="btn-action"
          >
            Renommer
          </button>
          <button
            onClick={() => handleDelete(selectedId!)}
            className="btn-action btn-delete"
          >
            Supprimer
          </button>
        </div>
      )}

      {editingId && (
        <div className="rename-dialog">
          <input
            type="text"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSaveRename();
              if (e.key === "Escape") {
                setEditingId(null);
                setEditTitle("");
              }
            }}
            autoFocus
          />
          <button onClick={handleSaveRename}>Enregistrer</button>
          <button onClick={() => {
            setEditingId(null);
            setEditTitle("");
          }}>Annuler</button>
        </div>
      )}
    </div>
  );
}

