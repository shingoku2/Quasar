# Implementation Plan - UI Overhaul

## Phase 1: Foundation & Dependencies
- [x] Task: Install UI Dependencies
    - [x] Install `lucide-react`, `recharts`, `clsx`, `tailwind-merge`.
    - [x] Configure `tailwind.config.js` (via App.css for v4) with the new color palette.
- [ ] Task: Conductor - User Manual Verification 'Foundation' (Protocol in workflow.md)

## Phase 2: App Shell Implementation [checkpoint: ad4f9ec]
- [x] Task: Implement Navigation Components [45fe87b]
    - [x] Create `src/components/Sidebar.tsx` with Lucide icons.
    - [x] Create `src/components/TopBar.tsx` with search and breadcrumbs.
    - [x] Update `src/components/Layout.tsx` to use the new shell structure (Sidebar + TopBar + Main Content).
- [x] Task: Integrate Existing Modules [45fe87b]
    - [x] Adapt `HostList` (via `RemoteManager`) to be the "Remote" view.
    - [x] Adapt `AIAssistant` to be the "AI" view.
    - [x] Ensure state-based navigation works between views.
- [x] Task: Conductor - User Manual Verification 'App Shell' (Protocol in workflow.md) [manual]

## Phase 3: Dashboard Module [checkpoint: d57e435]
- [x] Task: Implement Dashboard Widgets [6a5e21a]
    - [x] Create `src/components/dashboard/SystemHealthWidget.tsx`.
    - [x] Create `src/components/dashboard/MetricChartCard.tsx` using Recharts.
    - [x] Create `src/components/dashboard/AlertFeed.tsx`.
- [x] Task: Compose Dashboard View [6a5e21a]
    - [x] Create `src/components/dashboard/DashboardView.tsx` implementing the grid layout.
    - [x] Hook up mock data for charts and alerts.
- [x] Task: Conductor - User Manual Verification 'Dashboard' (Protocol in workflow.md) [manual]
