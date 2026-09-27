declare function t(key: string): string;
declare function Field(props: { hint: string; label: string }): null;

export function Literal() {
  return (
    <section className="p-4" data-state="open">
      <h1 title="Hardcoded title">{t("alpha.title")}</h1>
      <p>
        {t("alpha.first")}{" "}{t("alpha.second")}
      </p>
      <Field hint="Hardcoded hint" label={t("alpha.label")} />
      <p>Hardcoded text</p>
    </section>
  );
}
