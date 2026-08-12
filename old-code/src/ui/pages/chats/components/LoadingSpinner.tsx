export function LoadingSpinner() {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="h-8 w-8 animate-spin rounded-full border-3 border-white/10 border-t-white/70" />
    </div>
  );
}