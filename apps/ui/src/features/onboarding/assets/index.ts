import art from "./welcome.webp";
import artTall from "./welcome-mobile.webp";

const shots = import.meta.glob<string>("./guides/*.webp", { eager: true, import: "default" });

export function shotUrl(name: string): string {
  return shots[`./guides/${name}.webp`] ?? "";
}

export { art, artTall };
