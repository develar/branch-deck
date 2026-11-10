## ADDED Requirements

### Requirement: Hover and click: The pointer back over a trigger opens its popup

The pointer back over the trigger of a popup that closed otherwise than by its hover (its view hid, a press of an item)
MUST open the popup after the delay of its hover, whether the closed popup still runs its exit or went meanwhile: a
tooltip, a hover card and a submenu alike.

#### Scenario: A tooltip and a hover card that the hidden view closed
- **WHEN** a tooltip or a hover card (the description of a push) was open as its view hid, and the pointer comes back over its trigger as the view shows, before the closed popup went at the end of its exit
- **THEN** the popup opens after the delay of its hover
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the tooltip and the hover card whose trigger the pointer came back to while the popup the hidden view closed still went`)
  [@test] tests/e2e/landing/statistics.spec.ts (`should close the popups of the branches as the view hides, open none by itself as it shows, and leave the focus on no hidden control`)

#### Scenario: A submenu that a press of its item closed
- **WHEN** a press of an item of a submenu (Copy Branch Name) closed the menus, and the pointer comes back over the item of the submenu (Copy) in the row menu opened again, before the closed submenu went
- **THEN** the submenu opens after the delay of its hover
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the submenu of the row menu whose item the pointer came back to while the submenu a press closed still went`)
