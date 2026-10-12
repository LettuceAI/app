import { compactNumber } from "@lettuceai/crisp";

const priceFormats = new Map<string, Intl.NumberFormat>();

export function formatPrice(perMillion: number, locale: string): string {
  if (perMillion === 0) return "$0";
  let format = priceFormats.get(locale);
  if (!format) {
    format = new Intl.NumberFormat(locale, {
      style: "currency",
      currency: "USD",
      minimumFractionDigits: 2,
      maximumFractionDigits: 4,
    });
    priceFormats.set(locale, format);
  }
  return format.format(perMillion);
}

export function formatContext(tokens: number, locale: string): string {
  return compactNumber(tokens, locale);
}
