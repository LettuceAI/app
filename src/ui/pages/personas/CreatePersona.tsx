import { TopNav } from "../../components/App";
import { CreatePersonaForm } from "./components/CreatePersonaForm";
import { useCreatePersonaController } from "./hooks/createPersonaReducer";

export function CreatePersonaPage() {
    const { state, dispatch, canSave, handleImport, handleSave, topNavPath } = useCreatePersonaController();

    return (
        <div className="flex min-h-screen flex-col bg-surface text-fg">
            <TopNav currentPath={topNavPath} />

            <main className="flex-1 overflow-y-auto px-4 pb-20 pt-[calc(72px+env(safe-area-inset-top))]">
                <CreatePersonaForm
                    state={state}
                    dispatch={dispatch}
                    onImport={handleImport}
                    onSave={handleSave}
                    canSave={canSave}
                />
            </main>
        </div>
    );
}
