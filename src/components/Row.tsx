import { useRef, useState, useEffect, type ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";

export function Row({ title, children, action }: { title: string; children: ReactNode; action?: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);

  const update = () => {
    const el = ref.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 4);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 4);
  };

  useEffect(() => {
    update();
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [children]);

  const scroll = (dir: 1 | -1) => {
    const el = ref.current;
    if (!el) return;
    el.scrollBy({ left: dir * el.clientWidth * 0.85, behavior: "smooth" });
  };

  return (
    <section className="group/row relative mb-9">
      <div className="mb-3 flex items-center justify-between pr-2">
        <h2 className="text-xl font-bold tracking-tight">{title}</h2>
        {action}
      </div>
      <div className="relative">
        {canLeft && (
          <button
            aria-label="Scroll left"
            title="Scroll left"
            onClick={() => scroll(-1)}
            className="absolute -left-2 top-0 z-10 flex h-[calc(100%-44px)] w-12 items-center justify-center bg-gradient-to-r from-ink to-transparent opacity-0 transition group-hover/row:opacity-100"
          >
            <span className="flex h-11 w-11 items-center justify-center rounded-full bg-black/70 hover:bg-white hover:text-black">
              <ChevronLeft size={26} />
            </span>
          </button>
        )}
        <div ref={ref} onScroll={update} className="no-scrollbar flex gap-3 overflow-x-auto scroll-smooth px-1 py-2">
          {children}
        </div>
        {canRight && (
          <button
            aria-label="Scroll right"
            title="Scroll right"
            onClick={() => scroll(1)}
            className="absolute -right-2 top-0 z-10 flex h-[calc(100%-44px)] w-12 items-center justify-center bg-gradient-to-l from-ink to-transparent opacity-0 transition group-hover/row:opacity-100"
          >
            <span className="flex h-11 w-11 items-center justify-center rounded-full bg-black/70 hover:bg-white hover:text-black">
              <ChevronRight size={26} />
            </span>
          </button>
        )}
      </div>
    </section>
  );
}
