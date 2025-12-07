import React, { useState, useRef, useEffect } from 'react';
import { Send } from 'lucide-react';

interface Props {
    onSend: (content: string) => void;
    disabled: boolean;
}

const MessageInput: React.FC<Props> = ({ onSend, disabled }) => {
    const [content, setContent] = useState('');
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    const handleSubmit = (e?: React.FormEvent) => {
        e?.preventDefault();
        if (!content.trim() || disabled) return;
        onSend(content);
        setContent('');
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSubmit();
        }
    };

    useEffect(() => {
        if (textareaRef.current) {
            textareaRef.current.style.height = 'auto';
            textareaRef.current.style.height = textareaRef.current.scrollHeight + 'px';
        }
    }, [content]);

    return (
        <div className="border-t p-4 bg-white">
            <form onSubmit={handleSubmit} className="relative flex items-end gap-2 border rounded-xl p-2 bg-white shadow-sm focus-within:ring-2 focus-within:ring-blue-100 focus-within:border-blue-300 transition-all">
                <textarea
                    ref={textareaRef}
                    value={content}
                    onChange={(e) => setContent(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder="Type a message..."
                    disabled={disabled}
                    className="w-full resize-none outline-none max-h-32 bg-transparent text-gray-800 py-2 px-2"
                    rows={1}
                />
                <button
                    type="submit"
                    disabled={disabled || !content.trim()}
                    className={`p-2 rounded-lg transition-colors ${disabled || !content.trim()
                            ? 'text-gray-300 cursor-not-allowed'
                            : 'bg-blue-600 text-white hover:bg-blue-700 shadow-md'
                        }`}
                >
                    <Send size={20} />
                </button>
            </form>
            <div className="text-xs text-gray-400 mt-2 text-center">
                Codestral and Mistral can make mistakes. Check important info.
            </div>
        </div>
    );
};

export default MessageInput;
