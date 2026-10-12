import { AnimatePresence, motion } from "framer-motion";
import { ArrowLeft, ArrowRight, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Input, LoadingState, icon, motion as m } from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { ConnectNote } from "../components/ConnectNote";
import { ProviderForm } from "../components/ProviderForm";
import { ProviderList } from "../components/ProviderList";
import { ActionButton, ScreenTitle } from "../components/ui";
import { canConnect } from "../data/connect";
import { useDescriptor, useOnboarding } from "../flow/context";

export function ConnectScreen() {
  const { t } = useTranslation("onboarding");
  const { state, dispatch, phone, catalog, connect } = useOnboarding();
  const descriptor = useDescriptor(state.providerKind);
  const count = catalog.data?.length ?? 0;

  const guided = (
    <button
      type="button"
      onClick={() => dispatch({ type: "startGuided" })}
      className="group inline-flex items-center gap-1.5 self-start text-base text-accent underline-offset-4 hover:underline"
    >
      {t("connect.guidedLink")}
      <ArrowRight size={icon.sm} className="transition-transform group-hover:translate-x-0.5" />
    </button>
  );

  const search = (
    <Input
      value={state.search}
      onChange={(event) => dispatch({ type: "patch", patch: { search: event.target.value } })}
      placeholder={t("connect.search", { count })}
      leading={<Search size={icon.sm} />}
    />
  );

  const list = catalog.isPending ? (
    <LoadingState label={t("model.picker.loading")} />
  ) : catalog.isError ? (
    <FailureState error={catalog.error} onRetry={() => void catalog.refetch()} />
  ) : (
    <ProviderList descriptors={catalog.data} />
  );

  if (phone) {
    return (
      <AnimatePresence mode="wait" initial={false} custom={descriptor ? 1 : -1}>
        {descriptor ? (
          <motion.div key="form" initial={{ opacity: 0, x: 40 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: 40 }} transition={m.settle}>
            <ProviderForm />
            <div className="mt-4">
              <ConnectNote />
            </div>
            <button
              type="button"
              onClick={() => dispatch({ type: "chooseProvider", kind: null })}
              className="mt-6 inline-flex items-center gap-1.5 text-base text-accent underline-offset-4 hover:underline"
            >
              <ArrowLeft size={icon.sm} />
              {t("connect.chooseAnother")}
            </button>
          </motion.div>
        ) : (
          <motion.div key="list" initial={{ opacity: 0, x: -40 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -40 }} transition={m.settle}>
            <ScreenTitle title={t("connect.title")} lead={t("connect.lead")} />
            <div className="-mt-4 mb-6 flex">{guided}</div>
            <div className="mb-4">{search}</div>
            {list}
          </motion.div>
        )}
      </AnimatePresence>
    );
  }

  return (
    <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,3fr)_minmax(0,2fr)] gap-10">
      <div className="flex min-h-0 flex-col">
        <h1 className="text-3xl font-bold tracking-[-0.02em] text-fg">{t("connect.title")}</h1>
        <p className="mt-2 max-w-[60ch] text-base leading-relaxed text-fg-2">{t("connect.lead")}</p>
        <div className="mt-2 flex">{guided}</div>
        <div className="mt-5 mb-3">{search}</div>
        <div className="-mx-1 min-h-0 flex-1 overflow-y-auto px-1 pb-4 [mask-image:linear-gradient(to_bottom,black_calc(100%-2.5rem),transparent)] [scrollbar-width:none]">
          {list}
        </div>
      </div>
      <div className="flex min-h-0 flex-col gap-6 pt-[7.5rem]">
        <ProviderForm />
        {descriptor && (
          <>
            <ConnectNote />
            <ActionButton
              label={connect.busy ? t("actions.checking") : t("actions.connect")}
              disabled={connect.busy || !canConnect(descriptor, state)}
              loading={connect.busy}
              block
              onClick={connect.run}
            />
          </>
        )}
      </div>
    </div>
  );
}
