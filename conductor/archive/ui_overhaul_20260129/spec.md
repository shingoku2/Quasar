# Track: UI Overhaul: The Shell & Dashboard

## Specification
This track implements the "Phase 1: The Shell" and "Phase 2: Dashboard & Data" from the SysAdmin Nexus design concept. It replaces the current basic layout with a high-fidelity, high-density dark mode UI.

### Scope
- **Design System:** Implement the "SysAdmin Nexus" color palette and typography (Inter/JetBrains Mono).
- **App Shell:**
    - **Sidebar:** Fixed left navigation with icons (Lucide React).
    - **Top Bar:** Global search, breadcrumbs, and window controls.
    - **Main Area:** Scrollable container for active modules.
- **Dashboard Module:**
    - **System Health Widget:** Top banner with cluster status.
    - **Metric Chart Cards:** Reusable components using Recharts for CPU/Memory/Network (Mock data for now).
    - **Alert Feed:** List of recent system alerts.
- **Integration:** Adapt existing `HostList` (Remote) and `AIAssistant` (AI) components to fit within the new shell's navigation structure.

### Technical Stack Updates
- **Icons:** `lucide-react`
- **Charts:** `recharts`
- **Utils:** `clsx`, `tailwind-merge` (for dynamic classes)
- **Fonts:** Ensure `Inter` and `JetBrains Mono` are available (via Google Fonts or local).

### Visual Reference (from PDF)
- **Background:** `#1a1b21` (Zinc-900 equivalent)
- **Card Background:** `#27272a` (Zinc-800)
- **Accent:** `#3b82f6` (Blue-500)
- **Alert Critical:** `#ef4444` (Red-500)
- **Text:** High contrast, legible.

### User Experience
- The app should launch directly into the new Dashboard.
- Navigation between "Dashboard", "Remote", and "AI" should be instant (client-side routing or state-based switching).
