# Refrain UI Style Guide

## 1. Purpose

This document defines the visual system and interaction rules for Refrain's desktop UI.

It is intended to be detailed enough that another designer or AI can generate new Refrain screens without visually drifting away from the approved screens.

The design target is a **clean, bold, content-first cross-platform desktop application** for macOS, Windows, and Linux. It should feel purpose-built, quiet, precise, and native to the desktop without visually fragmenting into three different products. It should not look like a generic web dashboard, admin template, SaaS landing page, or "vibecoded" interface.

The approved concepts have a strong macOS-influenced level of restraint and polish, but that influence is a **visual quality reference, not a requirement to imitate macOS chrome or Apple-specific controls on every platform**.

The approved visual direction is represented by these screen families:

- Local Library
- Spotify Liked Songs
- Spotify Albums grid
- Spotify Album detail
- Spotify Playlists grid
- Spotify Playlist detail
- Settings
- Issues / Ambiguous Match Review

New screens must feel like they belong to the exact same product.

---

# 2. Core Design Principles

## 2.1 Content first

The interface exists to help users understand and manage their music library.

Prioritize:

1. Track and collection content
2. Current state
3. Primary action
4. Search and filtering
5. Supporting metadata
6. Secondary actions

Do not prioritize decoration.

The UI should remain visually quiet even when a screen contains a large amount of information.

## 2.2 Desktop application, not website

Refrain is a desktop app.

Use patterns that feel natural for a modern desktop application on macOS, Windows, and Linux:

- persistent sidebar navigation
- dense but readable tables
- split views
- compact toolbars
- restrained card use
- contextual side panels
- native-feeling segmented controls
- native-feeling search fields
- compact icon buttons
- clear keyboard-friendly hierarchy

Avoid common website patterns such as:

- oversized hero sections
- huge marketing headlines
- excessive card grids
- floating CTA banners
- full-page gradient backgrounds
- excessive rounded containers around every element
- mobile-first spacing
- oversized touch targets
- landing-page style copy blocks

## 2.3 Bold through hierarchy, not decoration

"Bold" means:

- strong page titles
- confident spacing
- clear selection states
- strong alignment
- decisive primary actions
- clear data hierarchy

It does **not** mean:

- bright colors everywhere
- large gradients
- thick shadows
- oversized cards
- excessive iconography
- decorative illustrations

## 2.4 Quiet semantic color

Most of the interface should be neutral.

Color is reserved for meaning:

- blue = selected / primary action / interactive emphasis
- green = healthy / local / complete / tracked
- orange = attention / warning / needs review
- red = failure / invalid / destructive
- gray = neutral / inactive / unavailable / secondary

Do not color every data point.

## 2.5 Dense, not cramped

Refrain is a library management tool, so density is intentional.

Use:

- compact row heights
- aligned metadata
- restrained vertical padding
- clear column structure
- subtle dividers
- small secondary text

Do not reduce text below practical readability just to fit more information.

---

# 3. Overall Visual Character

The approved visual character is:

- bright
- neutral
- lightly translucent
- native-feeling across macOS, Windows, and Linux
- structured
- understated
- modern
- precise
- spacious around major sections
- compact inside data-heavy sections

The application should feel closer to a refined native desktop utility than a web dashboard.

Useful references in spirit:

- Finder, Windows File Explorer, and modern Linux file managers for desktop information hierarchy
- Apple Music and other desktop media libraries for content density
- Things for restraint and hierarchy
- Linear for desktop layout discipline
- Raycast for visual restraint
- modern native preference windows across macOS, Windows, and Linux

Do not directly copy any of those products. Borrow principles such as density, hierarchy, restraint, and predictability rather than platform-specific ornament.



## 3.1 Cross-Platform Foundation

Refrain should have **one product identity** across macOS, Windows, and Linux.

The following remain consistent on every platform:

- information architecture
- sidebar structure and width
- page hierarchy
- spacing scale
- typography scale
- application colors
- component proportions
- tables and grid density
- search and filter behavior
- status vocabulary
- inspector structure
- button hierarchy
- artwork treatment
- issues workflow
- primary navigation
- Refrain branding

The following may adapt to the host operating system:

- window controls and title-bar chrome
- system font
- switch styling
- context menus
- native file and folder pickers
- keyboard modifier labels
- menu-bar / notification-area conventions
- scrollbars
- focus treatment
- system dialogs
- path syntax
- platform-specific integration terminology

### Platform rule

**Keep Refrain's content surface consistent; let the operating system own the parts users already expect the operating system to own.**

Do not fake one operating system inside another.

Examples:

- Do not show macOS traffic lights on Windows.
- Do not show Windows caption buttons on macOS.
- Do not render a custom fake Finder dialog on Linux.
- Do not label `⌘` shortcuts on Windows or Linux.
- Do not force SF Pro as a bundled application font on Windows or Linux.
- Do not use `/Users/...` paths as generic examples on Windows.

### Platform adaptation matrix

| Surface | macOS | Windows | Linux |
|---|---|---|---|
| Window chrome | native macOS controls | native Windows caption controls | native compositor/window-manager controls |
| UI font | system SF family | Segoe UI / system UI | desktop-environment system UI font |
| Primary modifier | `⌘` | `Ctrl` | `Ctrl` |
| Preferences naming | Settings or Preferences, follow product convention | Settings | Settings |
| Background app location | menu bar when applicable | notification area/system tray | status notifier/system tray when supported |
| Folder selection | native macOS picker | native Windows picker | native desktop/portal picker |
| Paths | `/Users/name/...` | `C:\Users\name\...` | `/home/name/...` |
| Context menus | native-feeling macOS menu behavior | native-feeling Windows menu behavior | native-feeling desktop-environment menu behavior |

### Cross-platform implementation principle

When the frontend framework allows it, prefer platform primitives and OS integrations for:

- window controls
- file pickers
- credential storage
- notifications
- context menus
- drag and drop
- keyboard accelerators
- reveal-in-file-manager actions
- opening URLs and external applications

The custom Refrain UI should begin at the product layer, not reimplement operating-system chrome.

---

# 4. Application Window

## 4.1 Default composition

Design mockups around a wide desktop window.

Recommended reference canvas:

- width: 1480 to 1540 px
- height: 900 to 960 px
- aspect ratio: approximately 16:10
- minimum useful content width: about 1180 px

The current approved concepts use a large desktop window rather than a compact floating utility. The same content geometry should be viable on macOS, Windows, and mainstream Linux desktop environments.

## 4.2 Window frame and platform chrome

The approved mockups use macOS traffic-light controls because they were generated on a macOS presentation canvas. **Those controls are not part of the Refrain design system.**

Production behavior:

- use native or platform-appropriate window controls
- preserve expected close / minimize / maximize behavior
- respect the host window manager
- do not reproduce another operating system's caption controls
- avoid custom title-bar chrome unless it provides a concrete product benefit

If Refrain uses a custom title bar, the product content, spacing, and visual treatment may remain consistent while the caption controls adapt per platform.

Application window treatment:

- rounded outer corners when supported by the OS/window manager
- subtle native-feeling shadow when supported
- light translucency only where platform support makes it reliable
- no heavy decorative border
- no mandatory colored title bar
- do not depend on transparency for readability or hierarchy

For generated concept images, macOS, Windows, or Linux window chrome may be used depending on the target platform. The underlying Refrain content layout must remain the same.

## 4.3 Background

The app itself should stay nearly white.

For presentation mockups, a softly blurred desktop wallpaper appropriate to the target platform can appear behind the window. This presentation background is not part of the application UI.

Do not place decorative gradients inside the product surface.

---

# 5. Layout System

## 5.1 Primary shell

The standard shell is:

```text
┌──────────────────────────────────────────────────────────────┐
│ Sidebar │ Main content                                      │
│         │                                                   │
│         │                                                   │
│         │                                                   │
└──────────────────────────────────────────────────────────────┘
```

Some content-heavy screens add an inspector or detail column:

```text
┌──────────────────────────────────────────────────────────────┐
│ Sidebar │ Main workspace                     │ Detail pane   │
└──────────────────────────────────────────────────────────────┘
```

Issues uses a master-detail split:

```text
┌──────────────────────────────────────────────────────────────┐
│ Sidebar │ Issue queue              │ Issue detail            │
└──────────────────────────────────────────────────────────────┘
```

## 5.2 Sidebar width

Target:

- 238 to 260 px

Recommended canonical width:

- **248 px**

Sidebar should remain visually stable across all screens.

## 5.3 Main content padding

Recommended:

- 28 to 32 px horizontal
- 24 to 28 px top
- 20 to 28 px bottom

Data tables may extend closer to the edges after the page header.

## 5.4 Optional right inspector

Typical width:

- 280 to 320 px

Recommended:

- **300 px**

Use it only when persistent contextual detail is useful.

Do not add an inspector just to fill space.

## 5.5 Master-detail split

For Issues:

- issue queue: about 360 to 420 px
- issue detail: remaining width

The queue should feel like a source list, not another dashboard.

---

# 6. Spacing Scale

Use a consistent 4 px base scale.

Preferred tokens:

| Token | Size | Use |
|---|---:|---|
| `space-1` | 4 px | icon/text micro-gap |
| `space-2` | 8 px | inline element gap |
| `space-3` | 12 px | compact group spacing |
| `space-4` | 16 px | default control spacing |
| `space-5` | 20 px | section internals |
| `space-6` | 24 px | card/section padding |
| `space-8` | 32 px | major section spacing |
| `space-10` | 40 px | large page separation |
| `space-12` | 48 px | exceptional large spacing |

Rules:

- keep controls tightly grouped
- separate major concepts with whitespace
- do not use random spacing values
- table rows should be compact
- page headers can breathe more than data regions

---

# 7. Typography

## 7.1 Font family

Primary:

```css
font-family:
  system-ui,
  -apple-system,
  BlinkMacSystemFont,
  "Segoe UI",
  Roboto,
  Ubuntu,
  Cantarell,
  "Noto Sans",
  "Helvetica Neue",
  Arial,
  sans-serif;
```

Use the host operating system's UI font whenever practical.

Expected results are approximately:

- macOS: SF Pro / the current Apple system UI family
- Windows: Segoe UI / the current Windows system UI family
- Linux: the desktop environment's configured UI font, commonly Ubuntu, Cantarell, Noto Sans, or another system sans

Typography should remain visually consistent through **size, weight, line height, and hierarchy**, not by forcing the exact same font file onto every platform.

Do not bundle or require a proprietary platform font merely to make another operating system resemble macOS.

Do not use a decorative display font.

## 7.2 Font roles

### Page title

- 32 to 36 px
- weight 700 to 800
- tight line height
- dark neutral
- used once per page

Example:

`Local Library`

### Collection/detail title

- 30 to 36 px
- weight 700 to 800

Example:

`Short n' Sweet`

### Section title

- 17 to 20 px
- weight 650 to 700

### Card heading

- 15 to 17 px
- weight 650 to 700

### Primary body

- 14 to 15 px
- weight 400 to 500

### Secondary body

- 13 to 14 px
- weight 400 to 500
- muted color

### Table text

- 13 to 14 px
- title cells may use 500 to 600 weight
- metadata uses 12 to 13 px

### Microcopy

- 11 to 12 px
- only for secondary timestamps, counts, paths, helper text

## 7.3 Typography rules

Use semibold selectively.

Good uses:

- page title
- track title
- album title
- settings section name
- primary number
- selected navigation label

Avoid bolding entire rows or large blocks of metadata.

---

# 8. Color System

The application should be mostly neutral.

## 8.1 Core neutrals

Recommended tokens:

```text
--bg-app:              #F8F9FB
--bg-content:          #FFFFFF
--bg-sidebar:          #F3F4F6
--bg-subtle:           #F7F8FA
--bg-hover:            #F2F5FA
--bg-selected:         #E8F0FF

--text-primary:        #121826
--text-secondary:      #667085
--text-tertiary:       #98A2B3
--text-disabled:       #B8C0CC

--border-default:      #E3E7ED
--border-subtle:       #EDF0F4
--divider:             #E8EBF0
```

These are guidance values. Small implementation adjustments are acceptable.

## 8.2 Accent blue

Use one consistent application blue.

Recommended:

```text
--blue-600: #1769FF
--blue-500: #2F7BFF
--blue-100: #E8F0FF
--blue-050: #F3F7FF
```

Usage:

- primary buttons
- selected navigation
- selected segmented control
- focused controls
- active tab underline
- active table/list selection

Do not use blue for generic decoration.

## 8.3 Semantic green

```text
--green-600: #169B62
--green-100: #E7F7EF
```

Usage:

- Local
- Tracked
- Connected
- Present
- Successful sync
- Available locally

Keep green mostly to:

- small icon
- small dot
- compact badge
- thin progress bar

## 8.4 Semantic orange

```text
--orange-600: #E97800
--orange-100: #FFF0DF
```

Usage:

- Needs Review
- attention required
- warning

Do not use large orange panels unless the state is genuinely blocking.

## 8.5 Semantic red

```text
--red-600: #E5484D
--red-100: #FDEBEC
```

Usage:

- Failed
- Invalid
- destructive action
- hard error

Red should be scarce.

## 8.6 Neutral state

```text
--gray-state-bg: #F0F2F5
--gray-state-fg: #667085
```

Usage:

- Not downloaded
- unavailable
- excluded
- inactive
- secondary status

---

# 9. Color Restraint Rules

This section is critical.

Do:

- keep 85 to 90 percent of the screen neutral
- use one primary blue
- use semantic colors only where state needs recognition
- prefer colored icons/dots over colored containers
- use light badge fills

Do not:

- give each category its own strong color
- use multiple bright colored cards in one row
- use rainbow dashboard statistics
- use saturated status cards
- use large gradients
- use colored backgrounds behind normal content
- add colored charts unless they communicate something essential

The approved Issues direction should be quieter than a typical admin dashboard.

---

# 10. Borders, Radius, and Shadows

## 10.1 Corner radius

Use a small set:

| Element | Radius |
|---|---:|
| window | 16 px |
| large panel | 10 to 12 px |
| card | 8 to 10 px |
| button | 7 to 9 px |
| input | 7 to 9 px |
| badge/pill | 999 px |
| artwork | 6 to 8 px |

Avoid over-rounding.

Not every block needs a card.

## 10.2 Borders

Default:

- 1 px
- very light gray

Borders should separate structure without looking boxed-in.

## 10.3 Shadows

Use sparingly.

Main window can have:

- wide
- soft
- low-opacity shadow

Internal cards generally should not have visible shadows.

Prefer borders and contrast over elevation.

---

# 11. Sidebar

## 11.1 Structure

The sidebar contains:

1. app identity
2. Library section
3. Local
4. Spotify parent
5. Spotify subsections
6. contextual destinations such as Issues when shown
7. Settings anchored near bottom

## 11.2 Brand area

Top-left:

- app icon around 42 to 46 px
- `Refrain`
- optional short tagline
- compact vertical spacing

The brand area must not dominate the screen.

## 11.3 Navigation row

Recommended:

- height: 36 to 40 px
- left padding: 12 px
- icon: 16 to 18 px
- label: 13 to 14 px
- count aligned right
- selected background: pale blue
- selected text/icon: application blue

Selected state should be obvious but quiet.

## 11.4 Nested Spotify items

Indent:

- 22 to 28 px from parent

Use familiar icons:

- heart for Liked Songs
- disc/circle for Albums
- playlist/music-list icon for Playlists

## 11.5 Bottom Settings item

Anchor near the bottom when space permits.

This gives the sidebar a desktop-app feel.

---

# 12. Page Headers

## 12.1 Standard header

Typical structure:

```text
Page title          Optional contextual label     Primary actions
Description
```

Example:

```text
Local Library       Your library                  Need Attention   Sync Local
These are tracks that physically exist on your disk...
```

## 12.2 Spotify header

Spotify screens may include:

- Spotify icon
- workspace title
- subsection name
- account identity
- last refresh
- Refresh Spotify button

Do not duplicate all of this if it makes the header heavy.

## 12.3 Header action hierarchy

Primary action:

- filled blue button

Secondary action:

- white / neutral bordered button

Attention action:

- light orange treatment
- not a giant warning banner

## 12.4 Summary metrics

For Local Library, compact inline metrics are preferred.

Use cards only when the screen benefits from them.

Avoid turning every page into a dashboard.

---

# 13. Buttons

## 13.1 Primary button

- blue fill
- white text
- medium weight
- 34 to 38 px height
- 10 to 14 px horizontal padding
- icon optional
- one dominant primary action per region

Examples:

- Sync Local
- Refresh Spotify
- Confirm Match

## 13.2 Secondary button

- white or transparent
- thin gray border
- dark text
- no heavy shadow

## 13.3 Destructive button

- white or pale red background
- red text
- only strong red fill for severe confirmations

## 13.4 Icon button

- 30 to 34 px square
- subtle hover fill
- no unnecessary border unless grouped with controls

## 13.5 Split button

Use when one primary action has closely related variants.

Example:

`Track Album ▾`

Do not use split buttons casually.

---

# 14. Search

Search is one of the most important persistent controls.

## 14.1 Appearance

- height: 36 to 40 px
- light border
- white background
- search icon on left
- muted placeholder
- radius 8 px

## 14.2 Placement

For dense list screens:

- put search above the list/table
- give search the most horizontal space in the filter row

For grid screens:

- search may appear in top toolbar

## 14.3 Placeholder style

Describe searchable data directly.

Examples:

- `Search title, artist, album, path, or collection`
- `Search albums, artists, or genres...`
- `Search in this playlist...`

---

# 15. Filters

## 15.1 Basic filters

Basic state filters may remain visible.

Examples:

- All
- On Spotify
- Local Only

or:

- All
- Tracked
- Not Downloaded
- Need Attention

Use a segmented control.

## 15.2 Advanced filters

Secondary filters must be hidden behind a single `Filters` button.

This is an intentional part of the Refrain design.

Examples to hide:

- Artist
- Album
- Year
- Format
- Collection
- Path contains
- Match status
- Acquisition status
- Explicit state
- Duration

Do not permanently expose a long row of dropdowns.

## 15.3 Expanded filter treatment

When expanded:

- use a popover, anchored panel, or compact collapsible row
- group filters logically
- avoid a full-page filter sidebar
- keep the music content visible
- show active filter count if useful
- provide `Clear filters`

---

# 16. Segmented Controls

Use for mutually exclusive, common filters.

Appearance:

- light gray outer background
- active segment: blue fill or white selected segment depending context
- active text with strong contrast
- compact height

Avoid web-style tab pills with excessive gaps.

---

# 17. Tables and Lists

Tables are central to Refrain.

## 17.1 Row height

Recommended:

- 42 to 48 px for simple rows
- 52 to 58 px when artwork and two-line metadata are present

## 17.2 Dividers

Use subtle horizontal dividers.

Avoid vertical grid lines unless absolutely necessary.

## 17.3 Track title cell

Typical composition:

```text
[artwork]  Track Title   [E]
           /path/or-secondary-info
```

or:

```text
[artwork]  Track Title
           Artist
```

Primary line:

- 13 to 14 px
- medium / semibold

Secondary:

- 11.5 to 12.5 px
- muted

## 17.4 Artwork

Typical table artwork:

- 34 to 42 px square
- 5 to 7 px radius

Grid artwork:

- square
- dominant element of album/playlist card

## 17.5 Column alignment

Text metadata:

- left aligned

Numeric/time values:

- may be right aligned or tabular aligned

Status:

- left aligned unless space is constrained

## 17.6 Overflow menus

Use a compact `•••` at far right.

Do not expose secondary actions as multiple buttons in each row.

## 17.7 Selected row

Use:

- pale blue background
- optional blue outline for active detail selection

Avoid strong saturated selection fills.

---

# 18. Status Presentation

## 18.1 Preferred forms

Use, in order:

1. icon + text
2. small dot + text
3. compact pill
4. very small progress indicator

Avoid large banners for normal statuses.

## 18.2 Common states

### Local / Present / Tracked

- green dot or check
- neutral text or green label
- light green pill only when useful

### Local Only

- orange dot
- neutral text

### Not Downloaded / Missing

- gray or muted cloud icon when informational
- red only when genuinely an error

### Needs Review

- orange warning icon
- orange text or light orange pill

### Failed / Invalid

- red icon
- red or dark text
- small light-red background if needed

### Unavailable

- gray
- low emphasis

---

# 19. Progress Bars

Use thin progress bars only when they help explain proportion.

Recommended:

- 3 to 5 px tall
- rounded
- muted track
- semantic foreground

Do not use progress bars underneath every metric.

---

# 20. Cards and Panels

## 20.1 General rule

Cards should group related information, not decorate empty space.

Good uses:

- settings group
- album metadata inspector
- playlist information
- match candidate
- sync summary

Bad uses:

- wrapping every metric in a large card
- wrapping each table row in a card
- turning a list into a grid of unrelated rectangles

## 20.2 Card padding

Recommended:

- 16 to 20 px

## 20.3 Card background

Usually:

- white

Border:

- light neutral

Shadow:

- none

---

# 21. Album and Playlist Grids

## 21.1 Grid structure

Use a content-first media grid.

Typical:

- 4 to 5 columns depending available width
- square artwork
- text directly beneath artwork
- minimal surrounding chrome

## 21.2 Card content

Album:

- cover
- album name
- artist
- year
- compact tracking/local status

Playlist:

- artwork
- playlist name
- track count
- compact status

## 21.3 Card hover/selection

Hover:

- subtle background or artwork emphasis

Selection:

- restrained blue outline or pale selection state

Do not put each media item inside a heavy card.

## 21.4 Right inspector

Grid screens may open a right-hand inspector for the selected album or playlist.

This is acceptable because it preserves browsing context.

---

# 22. Detail Pages

Album and Playlist detail screens share a common structure.

## 22.1 Top detail header

Use:

- back/breadcrumb navigation
- large artwork
- large title
- artist or collection identity
- compact metadata line
- primary tracking/sync control
- small summary metrics

## 22.2 Artwork

Target:

- 180 to 210 px square

It should be visually strong but not overpower the screen.

## 22.3 Metadata line

Example:

`2024 • 12 tracks • 36 min • Pop`

Keep it quiet and secondary.

## 22.4 Track area

The track list should begin relatively high on screen.

Do not create a huge hero area that pushes actual content below the fold.

---

# 23. Right-Side Inspectors

Use inspectors for persistent context.

Common sections:

- Information
- Cover
- Collections
- Quick Actions

Rules:

- narrow
- vertically stacked
- quiet
- low visual weight
- no unnecessary color
- no oversized section cards

Use a light divider or small card separation.

---

# 24. Settings

Settings should feel like a native preference surface.

## 24.1 Layout

Preferred shell:

```text
Sidebar | Settings content | Optional contextual utility column
```

Within Settings, a secondary settings category navigation may be used.

## 24.2 Settings rows

A settings group should look like:

```text
Section Title
Description

Setting name                  [control]
Short explanation
---------------------------------------
Setting name                  [control]
Short explanation
```

## 24.3 Controls

Use:

- native-feeling or platform-neutral switches
- dropdowns
- path fields
- compact buttons
- rows with chevrons when they open deeper configuration

## 24.4 Avoid

- giant preference cards
- colored category tiles
- excessive dashboard statistics
- large illustrations
- mobile-style settings

## 24.5 Current product areas

Keep the following concepts discoverable:

- Spotify connection
- Local Library
- Synchronization
- Acquisition
- runtime/application information

If additional product areas are introduced, they must use the same visual structure.

---

# 25. Issues

Issues needs special handling because it is decision-heavy.

## 25.1 Philosophy

Issues is not a dashboard.

It is an attention workspace.

The user should immediately understand:

- what needs attention
- which issue is selected
- what decision must be made
- what the consequences are

## 25.2 Layout

Use master-detail:

```text
Issue queue | Detail pane
```

The queue remains visible while resolving issues.

## 25.3 Issue queue

Each queue row:

- small artwork when relevant
- issue title
- artist / secondary info
- restrained status badge
- chevron
- selected state

Avoid excessive category colors.

## 25.4 Issue summary

If issue counts are shown, keep them compact.

Do not create four or five large brightly colored metric cards.

Preferred:

- one compact summary strip
- neutral numbers
- semantic color only on icon or critical count

## 25.5 Ambiguous Match Review

This screen must prioritize comparison.

Structure:

1. source track
2. candidate local matches
3. match evidence
4. linked local files
5. decision actions

Candidate cards can use borders because they represent discrete decisions.

## 25.6 Candidate card

Show:

- artwork
- title
- artist
- version
- duration
- format
- confidence percentage
- ISRC
- warnings
- local file path
- file state
- preferred file marker
- Confirm Match
- Reject

Do not use multiple large colored badges.

Use neutral chips for evidence such as:

- Exact match
- Same album
- Different version
- Album differs

Reserve orange/red for actual risk.

## 25.7 Rejected candidate

Use:

- muted card
- neutral/red rejection indicator
- `Clear Rejection`

Do not saturate the entire card red.

---

# 26. Local Library Screen Template

Recommended structure:

```text
[Sidebar]

Local Library    Your library                       Need Attention  Sync Local
Description

12,482 on disk | 12,601 indexed | Last sync | 12,440 matched | 18 need review

Search................................  [All | On Spotify | Local Only] [Filters]

Track table
```

Characteristics:

- inline summary metrics
- no oversized summary cards
- dense table
- path can appear as secondary row text
- collection memberships as compact neutral chips
- `Filters` collapses advanced controls

---

# 27. Spotify Liked Songs Template

Recommended structure:

```text
Spotify Library    Liked Songs             account     Refresh Spotify
Description

compact summary
Search........................ [basic state filter] [Filters]

Track table
```

Keep:

- tracking state visible
- local availability visible
- collection context visible
- row controls compact

If per-track tracking controls are needed, prefer one compact control or overflow pattern.

---

# 28. Albums Grid Template

Recommended structure:

```text
Spotify Library
Albums
Description                   Search       account    Refresh Spotify

compact summary

[All | Tracked | Not Downloaded | Need Attention]   Sort   View

Album grid                              Inspector
```

Rules:

- artwork drives the grid
- text treatment stays restrained
- inspector should not overpower the media grid
- keep grid density high enough to feel like a desktop library browser

---

# 29. Album Detail Template

Recommended:

```text
Breadcrumb / Back

Artwork | Album title
        | Artist
        | metadata
        | tracking control
        | compact status summary

Tabs: Tracks / About / Collections

Track table                              Inspector
```

Avoid unnecessary top-level cards.

---

# 30. Playlists Grid Template

Same system as Albums grid.

Playlist-specific differences:

- track count
- owner where relevant
- tracking state
- unavailable playlists can live in a collapsed secondary area

Do not mix unavailable playlists into the primary actionable grid.

---

# 31. Playlist Detail Template

Same shell as Album detail.

Important differences:

- track order matters
- duplicate occurrences must remain visible
- playlist-specific metadata may appear in inspector
- source ordering should never be hidden

---

# 32. Empty States

Empty states should remain calm.

Recommended structure:

```text
[small neutral icon]

No local music indexed yet
Choose a library folder in Settings, then run Local Sync.

[Open Settings]
```

Rules:

- no giant illustration
- no marketing copy
- no decorative gradients
- one clear next action

---

# 33. Loading States

Use skeletons that preserve final structure.

Examples:

- table row skeletons
- album art placeholders
- text line placeholders
- inspector placeholders

Avoid:

- full-page spinners
- large animated loaders
- skeletons with excessive shimmer

---

# 34. Error States

Use severity proportional to impact.

## Inline error

For localized failures:

- small red icon
- short explanation
- Retry if appropriate

## Page-level backend failure

Use a restrained error panel.

Do not flood the entire screen red.

---

# 35. Busy and Disabled States

When a user action is running:

- disable conflicting controls
- retain layout
- change label where useful
- show small spinner/progress state
- preserve cancel action for long-running syncs

Examples:

- `Syncing...`
- `Refreshing...`
- `Saving...`

Do not remove controls and cause layout shifts.

---

# 36. Sync Status

Current synchronization should be visible but compact.

Preferred pattern:

- small persistent status block in sidebar, toolbar, or top-right
- sync type
- current phase
- progress
- Cancel when active

Completed sync should collapse back into a quiet summary.

Do not permanently occupy a large part of the UI with sync status.

---

# 37. Iconography

Use a consistent outlined icon set.

Preferred visual characteristics:

- 1.5 to 2 px stroke
- simple
- geometric
- recognizable at 16 to 20 px

Suitable families:

- SF Symbols
- Lucide-style icons when platform-neutral implementation is required

Do not mix filled cartoon icons with thin outline icons.

Spotify's own brand icon may remain filled.

---

# 38. Artwork

Music artwork is the main source of visual richness.

This is important because the rest of the app should stay neutral.

Rules:

- let cover art carry color
- avoid adding competing decorative colors
- use consistent radius
- preserve aspect ratio
- use neutral fallback artwork
- never stretch images

---

# 39. Microinteractions

Keep transitions fast and restrained.

Suggested:

- hover: 100 to 150 ms
- selection: 120 to 180 ms
- panel expand/collapse: 160 to 220 ms
- no bouncy easing

Use motion to clarify state change, not decorate.

---

# 40. Focus and Keyboard Navigation

The app should support desktop workflows.

Design visible but restrained focus states:

- blue focus ring
- 2 px maximum
- only when keyboard navigation is active where platform permits

Important keyboard-accessible targets:

- sidebar navigation
- search
- filters
- rows
- overflow menus
- main actions
- issue decisions

---

# 40.1 Platform-Specific Interaction Details

## Keyboard shortcuts

The command should remain conceptually identical while modifier notation adapts to the operating system.

Examples:

| Action | macOS | Windows / Linux |
|---|---|---|
| Search | `⌘F` | `Ctrl+F` |
| Select all | `⌘A` | `Ctrl+A` |
| Refresh where appropriate | `⌘R` only if it does not conflict with platform conventions | `Ctrl+R` or platform-appropriate equivalent |
| Close window | `⌘W` | `Alt+F4` / desktop convention |

Do not hard-code modifier glyphs into reusable UI copy.

Where possible, obtain shortcut labels from the application's platform layer.

## File paths

Path examples and truncation behavior must respect the operating system.

Examples:

```text
macOS:   /Users/angelo/Music/...
Windows: C:\Users\angelo\Music\...
Linux:   /home/angelo/Music/...
```

The visual treatment is the same:

- secondary text
- selectable when useful
- full value available through tooltip, inspector, or copy action
- truncate rather than wrap aggressively inside dense tables

## Reveal-in-file-manager actions

Use platform-appropriate terminology or a neutral label such as:

- `Show in Folder`
- `Reveal in File Manager`

If platform-specific wording is used, adapt it:

- macOS: `Reveal in Finder`
- Windows: `Show in File Explorer`
- Linux: `Show in File Manager`

## Background-running terminology

Prefer neutral product copy where possible:

- `Keep Refrain running in the background`

Avoid making the core setting depend on the phrase `system tray`.

Platform-specific explanatory text may refer to:

- macOS menu bar, if Refrain actually exposes a menu-bar item
- Windows notification area
- Linux status notifier/system tray when supported by the desktop environment

## Native dialogs

External OS surfaces should remain native:

- folder picker
- save/export dialog
- file chooser
- confirmation dialog when delegated to the OS
- notifications
- permission prompts

Do not redraw these as fake Refrain screens unless Refrain intentionally replaces the operating-system flow.

---

# 41. Accessibility

Minimum requirements:

- text contrast should meet WCAG AA where practical
- never rely on color alone
- status must include text or icon
- focus indicators must be visible
- click targets should generally be at least 28 to 32 px in desktop UI
- destructive actions need clear labeling
- important errors should not be communicated only through tint

---

# 42. Data Formatting

Be consistent.

## Counts

Use commas:

`12,482`

## Duration

Use:

`3:45`

For aggregate duration:

`36 min`
`7 hr 26 min`

## Relative time

Use:

`9 min ago`
`2 hours ago`

Use absolute dates in detailed inspectors when necessary.

## Paths

Use monospaced or regular system text only if readability benefits.

Paths should:

- truncate in middle or tail depending context
- show full path on hover/tooltip
- never dominate the row

---

# 43. Chips and Badges

Badges should be compact.

Recommended:

- 20 to 24 px height
- 11 to 12 px text
- pill radius
- low-saturation fill

Examples:

- Tracked
- Local
- Need Attention
- Missing
- Present
- Preferred

Do not stack many badges on one line.

For multiple Spotify collections, show:

`Liked Songs   Chill Mix   +1`

instead of rendering every collection.

---

# 44. Selection and Hover

## Hover

Very subtle neutral tint.

## Selected navigation

Pale blue fill.

## Selected row/card

Pale blue fill or thin blue border.

## Active input

Blue focus border/ring.

Avoid strong dark selection backgrounds.

---

# 45. Information Density Targets

The interface should feel efficient.

Aim for:

- 10 to 14 visible track rows at 900 to 950 px window height
- 3 visible rows of album/playlist artwork in grid views
- major actions visible without scrolling
- detail inspector visible alongside primary content on wide windows

Avoid oversized whitespace that reduces useful content excessively.

---

# 46. Responsive Desktop Behavior

Refrain is desktop-first.

At narrower widths:

1. reduce grid columns
2. collapse right inspector into popover/drawer
3. preserve sidebar as long as practical
4. truncate secondary metadata
5. do not switch to a mobile-style stacked page unless absolutely necessary

---

# 47. Visual Anti-Patterns

The following should be treated as design failures.

## Too much color

Bad:

- four brightly tinted metric cards
- every status with unique saturated fill
- colored section backgrounds
- gradients for decoration

Correct:

- neutral interface
- cover artwork provides visual color
- semantic color limited to status indicators

## Too many cards

Bad:

- every subsection in its own rounded container
- every table row as a card
- dashboard-like block grid

Correct:

- cards only for meaningful grouping
- tables stay tables
- lists stay lists

## Website-like header

Bad:

- giant title
- large paragraph
- enormous call-to-action

Correct:

- compact page header
- actual content begins quickly

## Generic admin dashboard

Bad:

- KPI cards everywhere
- colored charts
- generic sidebar with dozens of modules
- analytics-style layout

Correct:

- music library first
- clear source/content hierarchy

## Excessive rounded corners

Bad:

- pill-shaped everything
- 16 px radius on every control

Correct:

- 7 to 10 px controls
- pills only for badges

## Too many visible filters

Bad:

- search + six dropdowns + chips + sort + view + toggles all at once

Correct:

- search
- basic state filter
- Filters button
- sort/view only when relevant

## Too much helper text

Bad:

- every row includes a paragraph

Correct:

- explain unfamiliar concepts once
- keep common controls self-explanatory

---

# 48. AI Screen Generation Instructions

When generating a new Refrain screen, follow this sequence.

## Step 1: Identify the screen family

Choose:

- library list
- media grid
- collection detail
- settings
- issue resolution
- progress/status
- empty/error state

Do not invent a new shell unless the workflow genuinely requires one.

## Step 2: Reuse the global shell

Always preserve:

- Refrain sidebar
- typography
- neutral palette
- blue primary accent
- spacing scale
- corner radii
- control styling

## Step 3: Decide the one primary action

Each region should have one obvious primary action.

Examples:

- Sync Local
- Refresh Spotify
- Track Album
- Sync Playlist
- Confirm Match

Everything else should be secondary.

## Step 4: Keep color restrained

Before adding color, ask:

"Does this color encode state or selection?"

If not, remove it.

## Step 5: Check density

Make sure the screen still feels like a desktop productivity tool.

## Step 6: Check for website drift

Remove:

- marketing layouts
- large hero areas
- decorative gradients
- excessive cards
- oversized buttons

## Step 7: Check against approved screens

A new screen should look believable if inserted directly between the existing Local, Spotify, Settings, and Issues screens.

---

# 49. Canonical Design Tokens

Use these as the default starting point.

```css
:root {
  /* Backgrounds */
  --bg-app: #F8F9FB;
  --bg-content: #FFFFFF;
  --bg-sidebar: #F3F4F6;
  --bg-subtle: #F7F8FA;
  --bg-hover: #F2F5FA;
  --bg-selected: #E8F0FF;

  /* Text */
  --text-primary: #121826;
  --text-secondary: #667085;
  --text-tertiary: #98A2B3;
  --text-disabled: #B8C0CC;

  /* Borders */
  --border-default: #E3E7ED;
  --border-subtle: #EDF0F4;
  --divider: #E8EBF0;

  /* Primary */
  --blue-600: #1769FF;
  --blue-500: #2F7BFF;
  --blue-100: #E8F0FF;
  --blue-050: #F3F7FF;

  /* Success */
  --green-600: #169B62;
  --green-100: #E7F7EF;

  /* Warning */
  --orange-600: #E97800;
  --orange-100: #FFF0DF;

  /* Error */
  --red-600: #E5484D;
  --red-100: #FDEBEC;

  /* Neutral state */
  --gray-state-bg: #F0F2F5;
  --gray-state-fg: #667085;

  /* Radius */
  --radius-control: 8px;
  --radius-card: 10px;
  --radius-panel: 12px;
  --radius-window: 16px;

  /* Spacing */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
}
```

---

# 50. Canonical Component Dimensions

Suggested starting values:

```text
Sidebar width                  248 px
Right inspector                300 px
Top page padding               26 px
Main horizontal padding        28 px
Search field height            38 px
Standard button height         36 px
Compact button height          32 px
Sidebar row height             38 px
Table row height               48 to 54 px
Settings row min height        58 px
Small artwork                  38 px
Medium artwork                 64 to 80 px
Detail artwork                 190 to 210 px
Grid gap                       18 to 22 px
Card radius                    10 px
Input radius                   8 px
```

---

# 51. Final Quality Checklist

Before approving a new screen, verify all of the following.

## Shell

- [ ] Same sidebar width and style
- [ ] Same Refrain brand treatment
- [ ] Same Refrain content shell with platform-appropriate window chrome
- [ ] Same main content padding

## Typography

- [ ] Host system UI font with the canonical Refrain type scale
- [ ] Strong page title
- [ ] Secondary copy is muted
- [ ] No excessive bold text

## Color

- [ ] Mostly neutral
- [ ] Blue is the only primary accent
- [ ] Green/orange/red are semantic
- [ ] No decorative gradients
- [ ] No excessive colored cards

## Layout

- [ ] Content appears quickly
- [ ] Desktop density is preserved
- [ ] No unnecessary cards
- [ ] Optional inspector only if useful

## Controls

- [ ] One clear primary action
- [ ] Search is prominent where needed
- [ ] Only basic filters are always visible
- [ ] Advanced filters are collapsible
- [ ] Overflow menus contain secondary actions

## Data

- [ ] Track metadata is readable
- [ ] Artwork remains visually dominant where appropriate
- [ ] File paths remain available where relevant
- [ ] Status is not represented by color alone

## Platform adaptation

- [ ] Window controls match the target operating system
- [ ] System UI font is used or respected
- [ ] Shortcut modifiers match the target operating system
- [ ] Path examples use the correct path syntax
- [ ] Native file dialogs are not imitated with fake in-app screens
- [ ] Product layout remains recognizably Refrain across macOS, Windows, and Linux

## Feel

- [ ] Looks like a polished native desktop app on the target OS
- [ ] Does not look like a website
- [ ] Does not look like a generic admin dashboard
- [ ] Does not look overdesigned
- [ ] Does not feel overstimulating
- [ ] Could sit beside the approved Refrain screens without visual drift

---

# 52. One-Sentence Design Rule

**Refrain should look like one refined cross-platform native music-management utility: consistent product hierarchy and visual language across macOS, Windows, and Linux, with platform-specific chrome and conventions adapting naturally to the host OS.**
