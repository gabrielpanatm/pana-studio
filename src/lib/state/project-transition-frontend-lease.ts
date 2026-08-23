const projectTransitionFrontendLeaseBrand: unique symbol = Symbol(
  "project-transition-frontend-lease",
);

export type ProjectTransitionFrontendLeaseKind =
  | "reattach"
  | "open"
  | "rescan"
  | "close"
  | "reload";

export type ProjectTransitionFrontendLeaseOwner =
  | "project-transition-controller"
  | "native-window-close";

export type ProjectTransitionFrontendLeaseRequest = Readonly<{
  kind: ProjectTransitionFrontendLeaseKind;
  owner: ProjectTransitionFrontendLeaseOwner;
}>;

export type ProjectTransitionFrontendLease = Readonly<{
  [projectTransitionFrontendLeaseBrand]: true;
  id: string;
  generation: number;
  kind: ProjectTransitionFrontendLeaseKind;
  owner: ProjectTransitionFrontendLeaseOwner;
  startedAt: number;
}>;

export type ProjectTransitionFrontendLeaseAuthority = {
  projectTransitionFrontendLease: ProjectTransitionFrontendLease | null;
  projectTransitionFrontendLeaseGeneration: number;
};

export type ProjectTransitionFrontendLeaseHooks = {
  onAcquire?: (lease: ProjectTransitionFrontendLease) => void;
  quiesce: (lease: ProjectTransitionFrontendLease) => Promise<void>;
  onRelease?: (
    lease: ProjectTransitionFrontendLease,
    durationMs: number,
  ) => void;
};

type ProjectTransitionFrontendLeasePublicIdentity = Readonly<{
  id: string;
  generation: number;
  kind: ProjectTransitionFrontendLeaseKind;
  owner: ProjectTransitionFrontendLeaseOwner;
}>;

function monotonicNow() {
  return globalThis.performance?.now() ?? Date.now();
}

function publicIdentity(
  lease: ProjectTransitionFrontendLease,
): ProjectTransitionFrontendLeasePublicIdentity {
  return Object.freeze({
    id: lease.id,
    generation: lease.generation,
    kind: lease.kind,
    owner: lease.owner,
  });
}

export class ProjectTransitionFrontendLeaseBusyError extends Error {
  readonly code = "PROJECT_TRANSITION_FRONTEND_BUSY" as const;
  readonly active: ProjectTransitionFrontendLeasePublicIdentity;
  readonly requested: ProjectTransitionFrontendLeaseRequest;

  constructor(
    active: ProjectTransitionFrontendLease,
    requested: ProjectTransitionFrontendLeaseRequest,
  ) {
    super(
      `Project Transition este ocupat de ${active.kind} (${active.id}); ${requested.kind} a fost refuzat.`,
    );
    this.name = "ProjectTransitionFrontendLeaseBusyError";
    this.active = publicIdentity(active);
    this.requested = Object.freeze({ ...requested });
  }
}

export class ProjectTransitionFrontendLeaseSupersededError extends Error {
  readonly code = "PROJECT_TRANSITION_FRONTEND_SUPERSEDED" as const;

  constructor(lease: ProjectTransitionFrontendLease) {
    super(`Project Transition ${lease.id} nu mai este proprietarul activ.`);
    this.name = "ProjectTransitionFrontendLeaseSupersededError";
  }
}

export function acquireProjectTransitionFrontendLease(
  authority: ProjectTransitionFrontendLeaseAuthority,
  request: ProjectTransitionFrontendLeaseRequest,
): ProjectTransitionFrontendLease {
  const active = authority.projectTransitionFrontendLease;
  if (active) {
    throw new ProjectTransitionFrontendLeaseBusyError(active, request);
  }

  const generation = authority.projectTransitionFrontendLeaseGeneration + 1;
  const lease = Object.freeze({
    [projectTransitionFrontendLeaseBrand]: true as const,
    id: `project-transition-${generation}`,
    generation,
    kind: request.kind,
    owner: request.owner,
    startedAt: monotonicNow(),
  });
  authority.projectTransitionFrontendLeaseGeneration = generation;
  authority.projectTransitionFrontendLease = lease;
  return lease;
}

export function isProjectTransitionFrontendLeaseCurrent(
  authority: Pick<ProjectTransitionFrontendLeaseAuthority, "projectTransitionFrontendLease">,
  lease: ProjectTransitionFrontendLease,
) {
  return authority.projectTransitionFrontendLease === lease;
}

export function requireCurrentProjectTransitionFrontendLease(
  authority: Pick<ProjectTransitionFrontendLeaseAuthority, "projectTransitionFrontendLease">,
  lease: ProjectTransitionFrontendLease,
) {
  if (!isProjectTransitionFrontendLeaseCurrent(authority, lease)) {
    throw new ProjectTransitionFrontendLeaseSupersededError(lease);
  }
}

export function releaseProjectTransitionFrontendLease(
  authority: ProjectTransitionFrontendLeaseAuthority,
  lease: ProjectTransitionFrontendLease,
): boolean {
  if (!isProjectTransitionFrontendLeaseCurrent(authority, lease)) return false;
  authority.projectTransitionFrontendLease = null;
  return true;
}

export async function runWithProjectTransitionFrontendLease<T>(
  authority: ProjectTransitionFrontendLeaseAuthority,
  request: ProjectTransitionFrontendLeaseRequest,
  hooks: ProjectTransitionFrontendLeaseHooks,
  operation: (lease: ProjectTransitionFrontendLease) => Promise<T>,
): Promise<T> {
  const lease = acquireProjectTransitionFrontendLease(authority, request);
  try {
    hooks.onAcquire?.(lease);
    await hooks.quiesce(lease);
    requireCurrentProjectTransitionFrontendLease(authority, lease);
    return await operation(lease);
  } finally {
    const durationMs = Math.max(0, monotonicNow() - lease.startedAt);
    if (releaseProjectTransitionFrontendLease(authority, lease)) {
      hooks.onRelease?.(lease, durationMs);
    }
  }
}
