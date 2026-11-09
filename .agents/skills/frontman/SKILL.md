---
name: frontman
description: Visual frontend development, live DOM component-tree mapping, computed CSS inspection, and hot-reload verification using Frontman dev-server middleware and browser MCP server. Use when making visual UI adjustments, diagnosing styling/layout discrepancies, resolving DOM elements to source components via source maps, and conducting designer/PM in-browser visual reviews.
metadata:
  version: "1.0.0"
  origin: ECC-Frontman
---

# Frontman — Visual Frontend Engineering & Live DOM Editing

## Overview & Core Philosophy

Most AI coding assistants operate **blindly from static source files**: they can read `.tsx`, `.vue`, or `.css` files, but they have zero awareness of how the code actually renders in a browser. They cannot see computed styles, element box boundaries, flex/grid overflow, responsive viewport behavior, or runtime DOM hierarchies.

**Frontman** (`frontman-ai/frontman`) reverses this workflow: **it starts from the running browser application and works backward to the source code.**

```
┌────────────────────────────────────────────────────────────────────────┐
│ Browser (Live Running App)                                             │
│ ┌──────────────────────────────────┐  ┌──────────────────────────────┐ │
│ │ Target UI Element (Clicked)      │  │ Frontman Overlay             │ │
│ │ e.g. <button class="btn-primary">│  │ (Chat, element picker, HMR)  │ │
│ └──────────────────┬───────────────┘  └──────────────┬───────────────┘ │
│                    │                                 │                 │
│ ┌──────────────────▼─────────────────────────────────▼───────────────┐ │
│ │ Browser-side MCP Server                                            │ │
│ │ Live DOM, Computed CSS, Component Tree, Source Maps, Console Logs   │ │
│ └──────────────────────────────────┬─────────────────────────────────┘ │
└────────────────────────────────────┼───────────────────────────────────┘
                                     │
┌────────────────────────────────────┼───────────────────────────────────┐
│ Framework Dev Server Middleware    │ (Vite / Next.js / Astro)          │
│ ┌──────────────────────────────────▼─────────────────────────────────┐ │
│ │ Maps DOM node -> Exact Component File + Line Number                 │ │
│ │ (via @frontman-ai/dom-element-to-component-source)                 │ │
│ │ Hot Module Reload (HMR) Trigger & Compilation Monitor              │ │
│ └──────────────────────────────────┬─────────────────────────────────┘ │
└────────────────────────────────────┼───────────────────────────────────┘
                                     │
┌────────────────────────────────────┼───────────────────────────────────┐
│ Everything Claude Code (ECC) Harness│                                  │
│ ┌──────────────────────────────────▼─────────────────────────────────┐ │
│ │ frontman-engineer Agent / Skill Orchestration                      │ │
│ │ Ingests live DOM + computed CSS -> Generates surgical code edit    │ │
│ │ Writes source file -> Receives instant HMR verification            │ │
│ └────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```

By connecting Frontman's framework middleware and browser MCP tools with the **Everything Claude Code (ECC)** agent ecosystem, AI agents gain human-like visual perception and live interactive debugging capabilities.

---

## Frontman vs. Existing ECC Frontend Tools

Frontman does not replace existing ECC agents; it provides the **missing runtime visual context** that completes the frontend toolchain:

| Tool / Agent | Operational Domain | Primary Capability | When to Use |
|:---|:---|:---|:---|
| **`frontman` (Skill) / `frontman-engineer` (Agent)** | **Live Browser Runtime** | Live DOM selection, computed CSS inspection, source-map component resolution, HMR verification | Fixing visual alignment, spacing, padding, responsive layout bugs, and copy tweaks directly from rendered UI |
| **`react-reviewer` / `vue-reviewer`** | **Static Source Code** | Hook rules, render performance, server/client component boundaries, state mutation | Code review after editing `.tsx` or `.vue` files; pre-commit quality gate |
| **`browser-qa`** | **Headless Browser Automation** | Post-deploy automated smoke tests, visual regression across 3 breakpoints, accessibility audits | Automated CI/CD verification; post-merge regression testing |
| **`e2e-runner`** | **Automated User Journeys** | End-to-end user journeys (Playwright / Agent Browser) | Testing critical flows (checkout, login, CRUD sequences) |
| **`a11y-architect`** | **Accessibility Governance** | WCAG 2.2 AAA/AA compliance, ARIA attribute auditing | Auditing accessibility semantics, keyboard navigation, color contrast |
| **`plan-canvas`** | **Static Markdown/HTML Review** | Reviewing `.plan.md` architecture plans with human annotation | Gating architectural plan sign-off before coding begins |

---

## Key Capabilities

1. **Component Tree & Source-Map Resolution**:
   - Resolves any rendered DOM node back to its exact component file, line number, and column using `@frontman-ai/dom-element-to-component-source` and framework source maps.
   - Eliminates guesswork in large nested React/Vue/Svelte codebases where dozens of components compose a single view.
2. **Computed CSS & Real Layout Inspection**:
   - Reads the true runtime box model: computed margins, padding, box-sizing, border, flexbox/grid dimensions, and inherited styles.
   - Diagnoses why an element is misaligned or overflowing rather than guessing based on static CSS class names.
3. **Interactive Visual Feedback Loop**:
   - Developers, designers, or product managers click an element in the browser overlay at `http://localhost:<port>/frontman`.
   - The agent receives the exact DOM selector, computed styles, and component path, makes the surgical edit, and observes hot module reload (HMR) without restarting the server.
4. **Zero Production Footprint**:
   - Framework integrations (`@frontman-ai/vite`, `@frontman-ai/nextjs`, `@frontman-ai/astro`) operate **strictly in development mode** (`NODE_ENV=development`).
   - Production builds (`npm run build` / `vite build`) strip Frontman completely, ensuring zero bytes added to deployed bundles.

---

## Installation & Setup

### 1. Vite Projects (React, Vue, Svelte) — e.g. OpenTally
In Vite-powered projects:

```bash
# Automated installer
npx @frontman-ai/vite install

# Or manual installation
npm install --save-dev @frontman-ai/vite
```

In `vite.config.ts`:
```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import frontman from '@frontman-ai/vite';

export default defineConfig({
  plugins: [
    react(),
    // Enabled in dev mode only; automatically no-ops in production build
    process.env.NODE_ENV !== 'production' && frontman(),
  ].filter(Boolean),
});
```

Run dev server:
```bash
npm run dev
# Frontman overlay is available at: http://localhost:5173/frontman
```

### 2. Next.js Projects
```bash
npx @frontman-ai/nextjs install
npm run dev
# Frontman overlay is available at: http://localhost:3000/frontman
```
*Supports App Router and Pages Router with Turbopack compatibility.*

### 3. Astro Projects
```bash
npx astro add @frontman-ai/astro
npm run dev
# Frontman overlay is available at: http://localhost:4321/frontman
```

---

## Standard Operating Procedure (SOP) for Visual Fixes

When invoked for visual bug fixes or UI refinements, follow this 6-phase protocol:

### Phase 1: Launch & Context Acquisition
1. Ensure the application dev server is running with the Frontman plugin active (`npm run dev`).
2. Query the Frontman endpoint or browser MCP server:
   - Identify the current route and viewport dimensions.
   - Fetch the DOM tree of the target view.

### Phase 2: Element Targeting & Source Mapping
1. Identify the target element via user prompt, click selection from the overlay, or CSS selector.
2. Resolve the DOM node to its component file and line number:
   ```json
   {
     "element": "button.submit-btn",
     "component": "src/components/vouchers/VoucherEntryView.tsx",
     "line": 142,
     "framework": "React"
   }
   ```

### Phase 3: Computed Style Diagnostics
1. Inspect the computed CSS of the element and its parent container:
   - Check `display` (flex, grid, block, inline-block).
   - Check `box-sizing`, `padding`, `margin`, `gap`, `width`, `max-width`.
   - Check typography: `font-size`, `line-height`, `font-family`, `color`.
2. Compare computed runtime values against the design specification or user requirement.

### Phase 4: Surgical Source Code Modification
1. Open the identified source file at the exact line number.
2. Apply minimal, surgical edits:
   - Prefer updating semantic CSS classes (e.g. Tailwind classes or scoped CSS modules).
   - Preserve component immutability and clean component boundaries.
   - Avoid `!important` overrides or inline style hacks unless specifically required.

### Phase 5: Hot Reload & Visual Verification
1. Save the modified file.
2. Monitor framework dev server logs to confirm clean recompilation and Hot Module Replacement (HMR).
3. Verify that the updated element rendered cleanly and resolved the visual discrepancy.

### Phase 6: Code Review & Quality Hand-off
1. Hand off the modified file to **`react-reviewer`** or **`vue-reviewer`** for hook/render checks.
2. If accessibility was affected, trigger **`a11y-architect`** to verify WCAG compliance.
3. Run project test suites (`npm test` / `vitest run`).

---

## Clean-Room & Production Hygiene Guardrails

1. **Development-Only Scope**: Frontman middleware must never be bundled into production artifacts. Verify `npm run build` output to guarantee zero Frontman scripts exist in `dist/`.
2. **Zero Sensitive Data Exposure**: Frontman server logs and overlay data are restricted to loopback/local interfaces (`localhost` / `127.0.0.1`). Never expose Frontman endpoints on public networks without authentication.
3. **Clean-Room Policy Compliance**: In clean-room environments like OpenTally, Frontman is utilized solely for visual frontend styling, layout adjustments, and component diagnostics. It must never be used to ingest unlicensed third-party proprietary assets.

