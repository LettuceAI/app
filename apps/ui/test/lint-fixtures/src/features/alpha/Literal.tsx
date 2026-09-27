declare function t(key: string): string;

export function Literal() {
  return (
    <section className="p-4" data-state="open">
      <h1 title="Hardcoded title">{t("alpha.title")}</h1>
      <p>Hardcoded text</p>
    </section>
  );
}
