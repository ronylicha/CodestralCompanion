import React, { useState } from 'react';
import { useChatStore } from '../../stores/useChatStore';
import { Plus, MessageSquare, Trash2, Edit2, X, Check } from 'lucide-react';

interface Props {
    isOpen: boolean;
    onClose: () => void;
}

const Sidebar: React.FC<Props> = ({ isOpen, onClose }) => {
    const { conversations, currentConversationId, createConversation, selectConversation, deleteConversation, renameConversation } = useChatStore();
    const [editingId, setEditingId] = useState<string | null>(null);
    const [editTitle, setEditTitle] = useState('');

    const handleCreate = async () => {
        await createConversation();
        // On mobile automatically close sidebar if needed, but here we just create
    };

    const startEdit = (e: React.MouseEvent, id: string, title: string) => {
        e.stopPropagation();
        setEditingId(id);
        setEditTitle(title);
    };

    const saveEdit = async (e: React.MouseEvent) => {
        e.stopPropagation();
        if (editingId) {
            await renameConversation(editingId, editTitle);
            setEditingId(null);
        }
    };

    const cancelEdit = (e: React.MouseEvent) => {
        e.stopPropagation();
        setEditingId(null);
    }

    const handleDelete = async (e: React.MouseEvent, id: string) => {
        e.stopPropagation();
        if (confirm('Delete this conversation?')) {
            await deleteConversation(id);
        }
    };

    return (
        <>
            {/* Overlay for mobile */}
            {isOpen && (
                <div
                    className="fixed inset-0 bg-black/20 z-20 lg:hidden"
                    onClick={onClose}
                />
            )}

            <div className={`fixed inset-y-0 left-0 z-30 w-64 bg-gray-900 text-gray-100 transform transition-transform duration-300 ease-in-out lg:relative lg:translate-x-0 ${isOpen ? 'translate-x-0' : '-translate-x-full'}`}>
                <div className="flex flex-col h-full">
                    <div className="p-4 border-b border-gray-700">
                        <button
                            onClick={handleCreate}
                            className="w-full flex items-center justify-center gap-2 bg-blue-600 hover:bg-blue-700 text-white py-2 px-4 rounded-lg transition-colors font-medium"
                        >
                            <Plus size={18} />
                            New Chat
                        </button>
                    </div>

                    <div className="flex-1 overflow-y-auto py-2">
                        {conversations.map((conv) => (
                            <div
                                key={conv.id}
                                onClick={() => { selectConversation(conv.id); isOpen && window.innerWidth < 1024 && onClose(); }}
                                className={`group flex items-center gap-3 px-4 py-3 cursor-pointer transition-colors ${currentConversationId === conv.id
                                        ? 'bg-gray-800 border-r-4 border-blue-500'
                                        : 'hover:bg-gray-800/50'
                                    }`}
                            >
                                <MessageSquare size={18} className="text-gray-400 shrink-0" />

                                {editingId === conv.id ? (
                                    <div className="flex-1 flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                                        <input
                                            value={editTitle}
                                            onChange={(e) => setEditTitle(e.target.value)}
                                            className="w-full bg-gray-700 text-white px-1 py-0.5 rounded text-sm outline-none border border-blue-500"
                                            autoFocus
                                        />
                                        <button onClick={saveEdit} className="text-green-400 hover:text-green-300"><Check size={14} /></button>
                                        <button onClick={cancelEdit} className="text-gray-400 hover:text-gray-300"><X size={14} /></button>
                                    </div>
                                ) : (
                                    <span className="flex-1 truncate text-sm text-gray-300 group-hover:text-white">
                                        {conv.title}
                                    </span>
                                )}

                                {!editingId && (
                                    <div className="hidden group-hover:flex items-center gap-1">
                                        <button
                                            onClick={(e) => startEdit(e, conv.id, conv.title)}
                                            className="p-1 text-gray-500 hover:text-blue-400 rounded"
                                        >
                                            <Edit2 size={14} />
                                        </button>
                                        <button
                                            onClick={(e) => handleDelete(e, conv.id)}
                                            className="p-1 text-gray-500 hover:text-red-400 rounded"
                                        >
                                            <Trash2 size={14} />
                                        </button>
                                    </div>
                                )}
                            </div>
                        ))}
                    </div>

                    <div className="p-4 border-t border-gray-700 text-xs text-center text-gray-500">
                        v0.1.0 • Tauri + Mistral
                    </div>
                </div>
            </div>
        </>
    );
};

export default Sidebar;
