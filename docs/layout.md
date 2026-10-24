# Layout and accessibility

**Layout** shows the views on the screen: a screenshot with a box around each view, the tree of views, and the
properties of the one you pick. The same tree feeds an **accessibility audit** that looks for what a screen reader or a
thumb would trip over.

## The Layout view

Press **Capture**. The screenshot and the view tree are read together, so they match.

- Move over the screenshot to outline the view under the pointer; click to select it. The tree opens down to it.
- **Outline every view** draws all the boxes at once.
- Search the tree by text, label, id or class.
- The properties show the class, text, label, id, position, size (in dp on Android) and flags: clickable, focusable,
  editable, scrollable, disabled, checked.

### Where the tree comes from

| | Source |
|---|---|
| Android | `uiautomator dump`, which every image has. It is the accessibility tree TalkBack reads. Nothing is installed. |
| iOS Simulator | The macOS Accessibility API on the Simulator window, which carries the iOS accessibility tree (what VoiceOver walks), with labels and identifiers. |

iOS needs two things: the Simulator window has to be open on that device, and the program that runs EmuLabZip
(the app, or your terminal for the command line) must be allowed in **System Settings, Privacy & Security,
Accessibility**. EmuLabZip says so when it is not. Positions are converted to points, so they match what the app
declares. Physical devices are not supported.

`uiautomator` cannot read screens that forbid it (secure windows, some video players); EmuLabZip reports that instead of
showing an empty tree.

## The audit

The **Accessibility** tab lists findings, most serious first. Click one to see the view and what to do about it.

| Rule | Looks for | WCAG |
|------|-----------|------|
| `missing-label` | A view that can be tapped or typed into and has no name: no text, no label, nothing inside it that has one | 1.1.1, 4.1.2 |
| `unlabeled-focusable` | A view that takes focus and says nothing | 4.1.2 |
| `touch-target` | A control smaller than 48 dp (Android) or 44 pt (iOS) in either direction | 2.5.8 |
| `contrast` | Text with a contrast under 4.5:1 (3:1 for large text), measured from the screenshot's pixels inside the view | 1.4.3 |
| `duplicate-label` | Neighbouring controls announced the same way ("Delete", "Delete", "Delete") | 2.4.6 |

Contrast is measured by taking the most common colour in the view as the background and the colour furthest from it as
the text. A view over a photo or a gradient is not judged, because there is no single background. Disabled controls are
not asked to be big or named.

This is a first pass from the layout alone. It does not replace trying the app with TalkBack or VoiceOver, and it cannot
know that a label is *wrong*.

## Command line and assistants

```bash
emulabzip layout                       # the tree as text
emulabzip layout --json --png screen.png
emulabzip a11y                         # the findings
emulabzip a11y --json
emulabzip a11y --fail-on warning       # exit 1 when something at least this serious is found (note, warning, error)
```

`a11y --fail-on` makes a CI step fail when a screen regresses. The [MCP server](mcp.md) offers `get_layout` and
`audit_accessibility`, so an assistant can look at a screen and fix what it finds.
