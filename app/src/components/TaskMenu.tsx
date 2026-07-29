import { useEffect, useRef, useState } from "react";

interface Props {
  onDelete: () => void;
}

/** タスク行の「…」メニュー。今のところ削除のみを持つ。 */
export default function TaskMenu({ onDelete }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [open]);

  return (
    <div ref={ref} className="relative shrink-0">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label="メニュー"
        className="rounded-md border border-slate-700 px-2 py-1.5 text-sm text-slate-400 hover:bg-slate-800 hover:text-slate-200"
      >
        ⋯
      </button>
      {open && (
        <div className="absolute right-0 z-10 mt-1 w-32 overflow-hidden rounded-md border border-slate-700 bg-slate-900 shadow-lg">
          <button
            onClick={() => {
              setOpen(false);
              onDelete();
            }}
            className="block w-full px-3 py-2 text-left text-sm text-rose-300 hover:bg-rose-500/10"
          >
            削除
          </button>
        </div>
      )}
    </div>
  );
}
