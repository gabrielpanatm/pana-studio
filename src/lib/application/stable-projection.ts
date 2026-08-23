type ProjectionReaders = Record<string, () => unknown>;

type StableProjection<Readers extends ProjectionReaders> = {
  readonly [Key in keyof Readers]: ReturnType<Readers[Key]>;
};

/** Keeps a presentation boundary stable while preserving fine-grained rune reads. */
export function stableProjection<Readers extends ProjectionReaders>(
  readers: Readers,
): StableProjection<Readers> {
  const projection = {} as StableProjection<Readers>;
  for (const key of Object.keys(readers) as Array<keyof Readers>) {
    Object.defineProperty(projection, key, {
      enumerable: true,
      get: readers[key],
    });
  }
  return projection;
}
