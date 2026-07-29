publish-view-release = Release preparation
publish-view-configuration = Configuration
publish-validation-valid = Valid Zola build
publish-validation-invalid = Invalid Zola build
publish-validation-error = Validation unavailable
publish-validation-queued = Validation scheduled
publish-validation-running = Validation in progress
publish-validation-none = Build not validated
publish-preflight-updated = The pre-publish check has been updated.
publish-preflight-failed = The check could not be completed: { $error }
publish-eyebrow = Publishing workspace
publish-title = Publish
publish-description = Check, build, and publish in one flow. Sources are saved before the Zola output is published.
publish-state = Publishing status
publish-ready = Ready to build
publish-needs-check = Needs checking
publish-tabs-label = Publishing views
publish-quality-gates = Quality gates
publish-preflight-title = Pre-publish check
publish-checking = Checking…
publish-run-preflight = Run preflight
publish-sources-saved = Sources saved
publish-sources-synced = The project session and disk are synchronized.
publish-unsaved-areas =
    { $count ->
        [one] { $count } area contains unpersisted changes.
       *[other] { $count } areas contain unpersisted changes.
    }
publish-save = Save
publish-project-audit = Project audit
publish-audit-summary = { $errors } errors · { $warnings } warnings
publish-audit-stale = The audit does not match the current revision.
publish-open-audit = Open audit
publish-validation-help = Run preflight to validate the project with Zola.
publish-check = Check
publish-bunny-target = Bunny CDN target
publish-bunny-description = Credentials remain in .env and are used only by the Rust deploy command.
publish-configure = Configure
publish-build-and-release = Build and publish
publish-release-current = Deliver the current version
publish-release-description = Building generates the local output. Publishing sends the configured output to Bunny CDN and never starts automatically.
publish-gates-warning = Resolve the gates before publishing. Actions remain available for controlled checks.
publish-open-log = Open log
publish-config-sources = config.toml · .env · Pană settings
publish-config-title = Build and destination configuration
publish-config-description = Settings are read and written through project commands, without a parallel interface configuration.
