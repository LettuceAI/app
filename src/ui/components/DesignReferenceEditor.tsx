import { useMemo } from "react";
import { ImagePlus, X } from "lucide-react";
import { useImageData } from "../hooks/useImageData";

function ReferenceThumb({
  value,
  index,
  onRemove,
}: {
  value: string;
  index: number;
  onRemove: (index: number) => void;
}) {
  const imageUrl = useImageData(value);

  return (
    <div className="group relative overflow-hidden rounded-lg border border-fg/10 bg-surface-el/20">
      {imageUrl ? (
        <img
          src={imageUrl}
          alt={`Design reference ${index + 1}`}
          className="h-28 w-full object-cover"
        />
      ) : (
        <div className="flex h-28 items-center justify-center text-xs text-fg/40">Loading...</div>
      )}
      <button
        type="button"
        onClick={() => onRemove(index)}
        className="absolute right-2 top-2 flex h-7 w-7 items-center justify-center rounded-md border border-fg/10 bg-surface/90 text-fg/70 opacity-0 transition hover:border-danger/40 hover:bg-danger/20 hover:text-danger group-hover:opacity-100"
        aria-label="Remove design reference"
      >
        <X size={14} />
      </button>
    </div>
  );
}

async function readFilesAsDataUrls(files: FileList | File[]): Promise<string[]> {
  return Promise.all(
    Array.from(files).map(
      (file) =>
        new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result as string);
          reader.onerror = () => reject(new Error(`Failed to read ${file.name}`));
          reader.readAsDataURL(file);
        }),
    ),
  );
}

export function DesignReferenceEditor({
  designDescription,
  onDesignDescriptionChange,
  referenceImages,
  onReferenceImagesChange,
  showHeader = true,
  title = "Design references",
  description = "Upload a few clear reference images plus one canonical visual description.",
  descriptionPlaceholder = "Describe the stable look: face, hair, build, age presentation, outfit cues, accessories, and art/style direction.",
}: {
  designDescription: string;
  onDesignDescriptionChange: (value: string) => void;
  referenceImages: string[];
  onReferenceImagesChange: (value: string[]) => void;
  showHeader?: boolean;
  title?: string;
  description?: string;
  descriptionPlaceholder?: string;
}) {
  const helperText = useMemo(
    () =>
      referenceImages.length > 0
        ? `${referenceImages.length} reference image${referenceImages.length === 1 ? "" : "s"} attached`
        : "No reference images attached yet",
    [referenceImages.length],
  );

  const handleAddImages = async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.multiple = true;
    input.onchange = async (event) => {
      const files = (event.target as HTMLInputElement).files;
      if (!files?.length) return;
      try {
        const nextImages = await readFilesAsDataUrls(files);
        onReferenceImagesChange([...referenceImages, ...nextImages]);
      } catch (error) {
        console.error("Failed to load design reference images:", error);
      }
    };
    input.click();
  };

  const handleRemoveImage = (index: number) => {
    onReferenceImagesChange(referenceImages.filter((_, currentIndex) => currentIndex !== index));
  };

  return (
    <section className="space-y-3">
      {showHeader ? (
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-semibold text-fg">{title}</h3>
            <p className="max-w-2xl text-sm leading-6 text-fg/55">{description}</p>
          </div>
          <button
            type="button"
            onClick={() => void handleAddImages()}
            className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-lg border border-fg/15 bg-fg/[0.04] px-3 py-2 text-sm font-medium text-fg/80 transition hover:border-fg/25 hover:bg-fg/[0.07]"
          >
            <ImagePlus size={14} />
            Add references
          </button>
        </div>
      ) : null}

      <div className="space-y-2">
        <label className="text-sm font-medium text-fg/75">Visual description</label>
        <textarea
          value={designDescription}
          onChange={(event) => onDesignDescriptionChange(event.target.value)}
          rows={4}
          placeholder={descriptionPlaceholder}
          className="min-h-[120px] w-full resize-y rounded-lg border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm leading-6 text-fg placeholder-fg/35 transition focus:border-fg/25 focus:outline-none"
        />
      </div>

      <div className="space-y-2">
        <div className="flex items-center justify-between gap-3 border-b border-fg/10 pb-2">
          <div>
            <div className="text-sm font-medium text-fg/75">Reference images</div>
            <div className="text-xs text-fg/40">{helperText}</div>
          </div>
          <div className="flex items-center gap-3">
            <div className="text-[11px] text-fg/35">Front, 3/4, or full-body</div>
            {!showHeader ? (
              <button
                type="button"
                onClick={() => void handleAddImages()}
                className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-lg border border-fg/15 bg-fg/[0.04] px-3 py-2 text-sm font-medium text-fg/80 transition hover:border-fg/25 hover:bg-fg/[0.07]"
              >
                <ImagePlus size={14} />
                Add references
              </button>
            ) : null}
          </div>
        </div>

        {referenceImages.length > 0 ? (
          <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
            {referenceImages.map((value, index) => (
              <ReferenceThumb
                key={`${value}-${index}`}
                value={value}
                index={index}
                onRemove={handleRemoveImage}
              />
            ))}
          </div>
        ) : (
          <div className="flex h-20 items-center justify-center rounded-lg border border-dashed border-fg/10 bg-fg/[0.02] px-4 text-sm text-fg/35">
            Add a few clear reference shots to lock face, proportions, outfit, and style.
          </div>
        )}
      </div>
    </section>
  );
}
