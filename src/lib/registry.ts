/**
 * Svelte attachments that keep DOM references in plain arrays, which the
 * lyric views read imperatively every frame. Each one clears its slot on
 * unmount unless another element has taken it already.
 */

/** Stores the element in `list[i]` while it is mounted. */
export function registerAt<T extends HTMLElement>(list: (T | undefined)[], i: number) {
  return (el: T) => {
    list[i] = el;
    return () => {
      if (list[i] === el) list[i] = undefined;
    };
  };
}

/** Stores the element in `grid[i][j]` (line i, word j) while it is mounted. */
export function registerIn<T extends HTMLElement>(grid: T[][], i: number, j: number) {
  return (el: T) => {
    (grid[i] ??= [])[j] = el;
    return () => {
      if (grid[i]?.[j] === el) delete grid[i][j];
    };
  };
}
