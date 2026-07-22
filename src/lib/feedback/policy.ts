export const FEEDBACK_CHANNELS = Object.freeze({
  statusBar: {
    role: "passive-transient",
    owns: ["save", "validation", "preview", "ai-authority", "current-source"] as const,
  },
  detailsPanel: {
    role: "durable-details",
    owns: ["problems", "operational-log", "terminal", "timeline"] as const,
  },
  notification: {
    role: "persistent-action-required",
    owns: ["conflict", "recovery", "operator-decision"] as const,
  },
  contextualCallout: {
    role: "local-mode-or-safety",
    owns: ["template-context", "safe-editing", "interactive-preview"] as const,
  },
});

export type FeedbackChannel = keyof typeof FEEDBACK_CHANNELS;
