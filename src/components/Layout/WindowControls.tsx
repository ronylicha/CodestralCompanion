import { getCurrentWindow } from '@tauri-apps/api/window';
import { Minus, Square, X } from 'lucide-react';

const WindowControls = () => {
    const handleMinimize = async () => {
        try {
            const win = getCurrentWindow();
            await win.minimize();
        } catch (e) {
            console.error('Failed to minimize:', e);
        }
    };

    const handleToggleMaximize = async () => {
        try {
            const win = getCurrentWindow();
            const isMaximized = await win.isMaximized();
            if (isMaximized) {
                await win.unmaximize();
            } else {
                await win.maximize();
            }
        } catch (e) {
            console.error('Failed to toggle maximize:', e);
        }
    };

    const handleClose = async () => {
        try {
            const win = getCurrentWindow();
            await win.hide();
        } catch (e) {
            console.error('Failed to hide:', e);
        }
    };

    return (
        <div className="flex items-center gap-0.5">
            <button
                type="button"
                onClick={handleMinimize}
                className="w-8 h-8 flex items-center justify-center hover:bg-gray-200 rounded text-gray-600 transition-colors"
            >
                <Minus size={16} />
            </button>
            <button
                type="button"
                onClick={handleToggleMaximize}
                className="w-8 h-8 flex items-center justify-center hover:bg-gray-200 rounded text-gray-600 transition-colors"
            >
                <Square size={12} />
            </button>
            <button
                type="button"
                onClick={handleClose}
                className="w-8 h-8 flex items-center justify-center hover:bg-red-500 hover:text-white rounded text-gray-600 transition-colors"
            >
                <X size={16} />
            </button>
        </div>
    );
};

export default WindowControls;
