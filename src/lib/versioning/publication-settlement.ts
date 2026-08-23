export type VersioningPublicationSettlement =
  | Readonly<{ ok: true }>
  | Readonly<{ ok: false; error: unknown }>;

/** Executes one frontend projection exactly once and preserves its rejection. */
export async function settleVersioningPublication(
  projection: () => void | Promise<void>,
): Promise<VersioningPublicationSettlement> {
  try {
    await projection();
    return { ok: true };
  } catch (error) {
    return { ok: false, error };
  }
}
