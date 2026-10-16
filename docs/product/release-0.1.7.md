# FolderSync 0.1.7 — remove decorative assurance copy

Removed the general page footer containing “Verified transfers · protected originals” in both languages, including its icon and unused styles. The application version remains available in Settings → Version. Replaced the local-data privacy slogan with a factual explanation of the storage location. Operational instructions, validation messages and confirmation details remain.

Fresh verification: all 32 frontend tests passed, ESLint passed, and the TypeScript/production frontend build passed. No synchronization engine behavior changed.

The progress panel no longer mounts inside the scrolling page or uses sticky positioning over task actions. It now occupies a persistent 150 px status row below the independently scrolling main content. Its idle, discovering and copying states keep the same footprint; long paths are ellipsized. The header is a separate nonshrinking row. Status updates therefore cannot push the heading or obscure the action toolbar. The frontend progress regression verifies the persistent status region outside main through start/end events and the native Cancel command. The hardcoded version assertion was updated for this release.

Headless Chrome layout checks using the actual stylesheet passed at widths 800, 1100 and 1920 px (height 812 px). Idle → progress → idle preserved main/header/status geometry and the scrolled main offset, with no outer overflow or header/content/status overlap. These are CSS fixture measurements, not native application acceptance: [raw layout results](evidence/layout-0.1.7.json).

Computer Use inspected the installed application before attempting an upgrade and observed the user's active “Lukas Test” synchronization. The running application was deliberately left untouched. This release is packaged for later installation and is not claimed to be installed or GUI-verified in production.
