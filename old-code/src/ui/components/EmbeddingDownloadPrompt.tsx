import { Download } from "lucide-react";
import { BottomMenu } from "../../ui/components/BottomMenu";
import { useNavigate } from "react-router-dom";
import { useI18n } from "../../core/i18n/context";

interface EmbeddingDownloadPromptProps {
    isOpen: boolean;
    onClose: () => void;
    returnTo?: string;
}

export function EmbeddingDownloadPrompt({ isOpen, onClose, returnTo }: EmbeddingDownloadPromptProps) {
    const { t } = useI18n();
    const navigate = useNavigate();

    const handleDownload = () => {
        onClose();
        if (returnTo) {
            navigate(`/settings/embedding-download?returnTo=${encodeURIComponent(returnTo)}`);
        } else {
            navigate("/settings/embedding-download");
        }
    };

    const handleCancel = () => {
        onClose();
    };

    return (
        <BottomMenu
            isOpen={isOpen}
            onClose={onClose}
            title={t("components.embeddingDownload.downloadRequired")}
            includeExitIcon={false}
        >
            <div className="space-y-4">
                {/* Icon and Message */}
                <div className="text-center">
                    <div className="mb-4 flex justify-center">
                        <div className="flex h-16 w-16 items-center justify-center rounded-full border border-blue-400/20 bg-blue-500/10">
                            <Download className="h-8 w-8 text-blue-400" />
                        </div>
                    </div>
                    <h3 className="text-lg font-semibold text-white">{t("components.embeddingDownload.modelRequired")}</h3>
                    <p className="mt-2 text-sm text-white/60">
                        {t("components.embeddingDownload.description")}
                    </p>
                </div>

                {/* Info Box */}
                <div className="rounded-xl border border-blue-400/20 bg-blue-400/5 p-3">
                    <ul className="space-y-1 text-xs text-blue-200/80">
                        <li>{t("components.embeddingDownload.localStorage")}</li>
                        <li>{t("components.embeddingDownload.downloadSize")}</li>
                        <li>{t("components.embeddingDownload.summarization")}</li>
                    </ul>
                </div>

                {/* Action Buttons */}
                <div className="flex gap-3">
                    <button
                        onClick={handleCancel}
                        className="flex-1 rounded-full border border-white/15 bg-white/5 px-6 py-3 text-sm font-medium text-white transition hover:border-white/25 hover:bg-white/10"
                    >
                        {t("common.buttons.cancel")}
                    </button>
                    <button
                        onClick={handleDownload}
                        className="flex-1 rounded-full border border-blue-400/50 bg-blue-500/20 px-6 py-3 text-sm font-medium text-blue-100 transition hover:border-blue-300 hover:bg-blue-500/30"
                    >
                        {t("common.buttons.download")}
                    </button>
                </div>
            </div>
        </BottomMenu>
    );
}
