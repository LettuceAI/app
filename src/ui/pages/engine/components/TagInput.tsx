import { X } from "lucide-react";
import { useState, type KeyboardEvent } from "react";

type Props = {
  label: string;
  value: string[];
  onChange: (value: string[]) => void;
  placeholder?: string;
};

export function TagInput({ label, value, onChange, placeholder }: Props) {
  const [input, setInput] = useState("");

  const addTag = () => {
    const trimmed = input.trim();
    if (trimmed && !value.includes(trimmed)) {
      onChange([...value, trimmed]);
    }
    setInput("");
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addTag();
    } else if (e.key === "Backspace" && !input && value.length > 0) {
      onChange(value.slice(0, -1));
    }
  };

  const removeTag = (index: number) => {
    onChange(value.filter((_, i) => i !== index));
  };

  return (
    <div>
      <label className="mb-1 block text-[11px] font-medium text-white/70">{label}</label>
      <div className="rounded-lg border border-white/10 bg-black/20 px-2 py-1.5">
        {value.length > 0 && (
          <div className="mb-1.5 flex flex-wrap gap-1">
            {value.map((tag, i) => (
              <span
                key={i}
                className="flex items-center gap-1 rounded-md bg-white/10 px-2 py-0.5 text-[11px] text-white/80"
              >
                {tag}
                <button
                  onClick={() => removeTag(i)}
                  className="rounded p-0.5 hover:bg-white/10"
                >
                  <X className="h-2.5 w-2.5 text-white/50" />
                </button>
              </span>
            ))}
          </div>
        )}
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={addTag}
          placeholder={value.length === 0 ? (placeholder || "Type and press Enter") : "Add more..."}
          className="w-full bg-transparent text-sm text-white placeholder-white/40 focus:outline-none"
        />
      </div>
    </div>
  );
}
